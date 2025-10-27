use crate::{api::record_common_logs, middlewares, models::AppStates};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::agent_logs::{
    agent_batch_ingest_logs, agent_create_index, agent_delete_logs, agent_ingest_log,
    agent_query_detail_log, agent_search_logs,
};

// 显式指定返回类型包含状态
pub fn init_routes(app_states: Arc<AppStates>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 基础路由
    let app = Router::new()
        .route("/health", get(record_common_logs::health_check))
        .route("/ready", get(record_common_logs::health_check))
        .route("/api/logs", post(record_common_logs::ingest_log))
        .route(
            "/api/logs/batch",
            post(record_common_logs::batch_ingest_logs),
        )
        .route("/api/logs/search", get(record_common_logs::search_logs))
        // agent智能体,日志相关操作
        .route("/api/agent/log/add", post(agent_ingest_log))
        .route("/api/agent/log/batch", post(agent_batch_ingest_logs))
        .route("/api/agent/log/search", post(agent_search_logs))
        .route("/api/agent/log/detail", post(agent_query_detail_log))
        .route("/api/agent/log/createIndex", get(agent_create_index))
        .route(
            "/api/agent/log/delete/{index_name}",
            delete(agent_delete_logs),
        )
        .layer(cors)
        // change the default limit 20MB
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(app_states);

    // 应用中间件
    middlewares::apply_middlewares(app)
}
