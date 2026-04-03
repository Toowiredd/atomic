//! Security headers middleware — injects standard HTTP security headers on every response.
//!
//! Headers added:
//! - `X-Content-Type-Options: nosniff`                    — prevents MIME-type sniffing
//! - `X-Frame-Options: DENY`                              — disallows framing in browsers
//! - `Referrer-Policy: strict-origin-when-cross-origin`   — limits referrer information
//! - `X-Request-Id: <uuid>`                               — unique ID for request correlation

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use futures::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};

/// Middleware that adds standard HTTP security headers to every response and
/// stamps each request with a unique `X-Request-Id` for log correlation.
pub struct SecurityHeaders;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SecurityHeadersMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SecurityHeadersMiddleware { service })
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
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
        // Generate a unique request ID before handing off to inner service.
        let request_id = uuid::Uuid::new_v4().to_string();
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut resp = fut.await?;
            let headers = resp.headers_mut();

            headers.insert(
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            );
            headers.insert(
                HeaderName::from_static("x-frame-options"),
                HeaderValue::from_static("DENY"),
            );
            headers.insert(
                HeaderName::from_static("referrer-policy"),
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            );
            // `request_id` is a UUID string — all ASCII, so from_bytes is infallible.
            if let Ok(val) = HeaderValue::from_bytes(request_id.as_bytes()) {
                headers.insert(HeaderName::from_static("x-request-id"), val);
            }

            Ok(resp)
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

    #[actix_web::test]
    async fn test_security_headers_present() {
        let app = actix_test::init_service(
            App::new()
                .wrap(SecurityHeaders)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let headers = resp.headers();
        assert_eq!(
            headers.get("x-content-type-options").unwrap(),
            "nosniff",
            "should prevent MIME-sniffing"
        );
        assert_eq!(
            headers.get("x-frame-options").unwrap(),
            "DENY",
            "should disallow framing"
        );
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin",
            "should limit referrer leakage"
        );
        assert!(
            headers.get("x-request-id").is_some(),
            "should stamp a unique request ID"
        );
    }

    #[actix_web::test]
    async fn test_request_id_is_unique_per_request() {
        let app = actix_test::init_service(
            App::new()
                .wrap(SecurityHeaders)
                .route("/ping", web::get().to(ok_handler)),
        )
        .await;

        let req1 = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp1 = actix_test::call_service(&app, req1).await;
        let id1 = resp1
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let req2 = actix_test::TestRequest::get().uri("/ping").to_request();
        let resp2 = actix_test::call_service(&app, req2).await;
        let id2 = resp2
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        assert_ne!(id1, id2, "each request should receive a unique ID");
    }

    #[actix_web::test]
    async fn test_security_headers_do_not_alter_body() {
        let app = actix_test::init_service(
            App::new()
                .wrap(SecurityHeaders)
                .route("/data", web::get().to(ok_handler)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/data").to_request();
        let body = actix_test::call_and_read_body(&app, req).await;
        assert_eq!(body, r#"{"ok":true}"#);
    }
}
