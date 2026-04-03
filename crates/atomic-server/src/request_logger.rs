//! Request logging middleware — emits a structured tracing event for every HTTP request.
//!
//! Logs the HTTP method, path, response status code, elapsed time in milliseconds,
//! and peer IP address so operators can monitor traffic without an external access-log agent.

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};
use std::time::Instant;

/// Middleware that emits a structured `tracing` event after every HTTP request.
///
/// Fields logged per request:
/// - `method`      — HTTP verb (GET, POST, …)
/// - `path`        — request path (no query string)
/// - `status`      — response HTTP status code
/// - `elapsed_ms`  — wall-clock milliseconds from start of request to response
/// - `peer`        — client IP address (or `"-"` when unavailable)
pub struct RequestLogger;

impl<S, B> Transform<S, ServiceRequest> for RequestLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestLoggerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestLoggerMiddleware { service })
    }
}

pub struct RequestLoggerMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start = Instant::now();
        let method = req.method().to_string();
        let path = req.path().to_string();
        let peer = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "-".to_string());

        let fut = self.service.call(req);
        Box::pin(async move {
            let result = fut.await;
            let elapsed_ms = start.elapsed().as_millis();
            match &result {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    tracing::info!(
                        method = %method,
                        path = %path,
                        status = status,
                        elapsed_ms = elapsed_ms,
                        peer = %peer,
                        "request"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        method = %method,
                        path = %path,
                        elapsed_ms = elapsed_ms,
                        peer = %peer,
                        error = %e,
                        "request error"
                    );
                }
            }
            result
        })
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

    async fn not_found_handler() -> HttpResponse {
        HttpResponse::NotFound().finish()
    }

    #[actix_web::test]
    async fn test_request_logger_passes_through_200() {
        let app = actix_test::init_service(
            App::new()
                .wrap(RequestLogger)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_request_logger_passes_through_404() {
        let app = actix_test::init_service(
            App::new()
                .wrap(RequestLogger)
                .route("/ping", web::get().to(not_found_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_request_logger_does_not_alter_response_body() {
        let app = actix_test::init_service(
            App::new()
                .wrap(RequestLogger)
                .route("/data", web::get().to(ok_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/data").to_request();
        let body = actix_test::call_and_read_body(&app, req).await;
        assert_eq!(body, r#"{"ok":true}"#);
    }
}
