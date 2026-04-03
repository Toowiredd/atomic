//! Rate limiting middleware — caps requests per peer IP address.
//!
//! Uses a simple fixed-window counter: at most `max_requests` requests are allowed
//! from a single IP within any `window` duration.  When a client exceeds the limit
//! the middleware returns `429 Too Many Requests` and they must wait for the current
//! window to expire.
//!
//! The internal map is pruned periodically (every `PRUNE_EVERY` requests globally)
//! to prevent unbounded memory growth from idle entries.

use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use actix_web::HttpResponse;
use futures::future::{ok, LocalBoxFuture, Ready};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

type RateMap = Arc<Mutex<HashMap<String, (u64, Instant)>>>;

/// How often (in total requests across all IPs) to scan and prune stale map entries.
const PRUNE_EVERY: u64 = 1_000;

/// Middleware factory.  Cloning is cheap — the `Arc<Mutex<…>>` is shared across
/// all worker threads so counters are global, not per-thread.
#[derive(Clone)]
pub struct RateLimiter {
    state: RateMap,
    max_requests: u64,
    window: Duration,
    /// Global request counter used to decide when to prune stale entries.
    global_count: Arc<AtomicU64>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// * `max_requests` — maximum number of requests allowed per `window_secs`
    /// * `window_secs`  — length of the counting window in seconds
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_secs),
            global_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimiterMiddleware {
            service,
            state: self.state.clone(),
            max_requests: self.max_requests,
            window: self.window,
            global_count: self.global_count.clone(),
        })
    }
}

pub struct RateLimiterMiddleware<S> {
    service: S,
    state: RateMap,
    max_requests: u64,
    window: Duration,
    global_count: Arc<AtomicU64>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let key = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let allowed = {
            let now = Instant::now();
            let mut map = self.state.lock().unwrap_or_else(|e| e.into_inner());

            let entry = map.entry(key.clone()).or_insert((0, now));

            // Reset counter when the current window has expired.
            if now.duration_since(entry.1) >= self.window {
                *entry = (0, now);
            }

            // Increment then check — correct for all values of max_requests including 0.
            entry.0 += 1;
            let under = entry.0 <= self.max_requests;

            // Prune stale entries on every PRUNE_EVERY-th request (global, not per-IP)
            // to bound memory without creating unpredictable per-IP latency spikes.
            let n = self.global_count.fetch_add(1, Ordering::Relaxed);
            if n % PRUNE_EVERY == 0 {
                map.retain(|_, v| now.duration_since(v.1) < self.window);
            }

            under
        };

        if !allowed {
            tracing::warn!(peer = %key, limit = self.max_requests, "rate limit exceeded");
            return Box::pin(async move {
                Ok(req
                    .into_response(
                        HttpResponse::TooManyRequests()
                            .json(serde_json::json!({"error": "rate limit exceeded"})),
                    )
                    .map_into_right_body())
            });
        }

        let fut = self.service.call(req);
        Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test as actix_test;
    use actix_web::{web, App, HttpResponse};

    async fn ok_handler() -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
    }

    #[actix_web::test]
    async fn test_requests_within_limit_succeed() {
        let limiter = RateLimiter::new(3, 60);
        let app = actix_test::init_service(
            App::new()
                .wrap(limiter)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        for _ in 0..3 {
            let req = actix_test::TestRequest::get().uri("/ping").to_request();
            let resp = actix_test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
        }
    }

    #[actix_web::test]
    async fn test_request_over_limit_returns_429() {
        let limiter = RateLimiter::new(2, 60);
        let app = actix_test::init_service(
            App::new()
                .wrap(limiter)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        // First two should succeed.
        for _ in 0..2 {
            let req = actix_test::TestRequest::get().uri("/ping").to_request();
            let resp = actix_test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
        }

        // Third request exceeds the limit.
        let req = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429);
    }

    #[actix_web::test]
    async fn test_rate_limit_response_body_contains_error_field() {
        let limiter = RateLimiter::new(0, 60);
        let app = actix_test::init_service(
            App::new()
                .wrap(limiter)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429);

        let body = actix_test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "rate limit exceeded");
    }

    #[actix_web::test]
    async fn test_window_expiry_resets_counter() {
        // Zero-second window — every request opens a new window, so all requests
        // should succeed even with max_requests = 1.
        let limiter = RateLimiter::new(1, 0);
        let app = actix_test::init_service(
            App::new()
                .wrap(limiter)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        for _ in 0..5 {
            let req = actix_test::TestRequest::get().uri("/ping").to_request();
            let resp = actix_test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
        }
    }
}

