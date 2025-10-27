mod mark_log_span;
mod request_id;
mod request_logger;
mod server_time;

use axum::Router;
use axum::middleware::from_fn;
use mark_log_span::MyDefaultMakeSpan;
use request_id::set_request_id;
use request_logger::log_request;
use server_time::ServerTimeLayer;
use tower::ServiceBuilder;
use tower_http::LatencyUnit;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

// pub use auth::{extract_user, verify_token};

// pub trait TokenVerify {
//     type Error: fmt::Debug;
//     fn verify(&self, token: &str) -> Result<User, Self::Error>;
// }

const REQUEST_ID_HEADER: &str = "x-request-id";
const SERVER_TIME_HEADER: &str = "x-server-time";

/// 将中间件应用到路由
pub fn apply_middlewares<S>(app: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    app.layer(
        ServiceBuilder::new()
            .layer(from_fn(set_request_id))
            .layer(from_fn(log_request))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(MyDefaultMakeSpan::new().include_headers(false))
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(
                        DefaultOnResponse::new()
                            .level(Level::INFO)
                            .latency_unit(LatencyUnit::Micros),
                    ),
            )
            .layer(ServerTimeLayer),
    )
}
