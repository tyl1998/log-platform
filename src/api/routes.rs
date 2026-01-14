use crate::{
    api::docs::ApiDoc,
    api::record_common_logs,
    middlewares,
    models::{
        AppStates, DeleteParams, HttpResult, KnowledgeSearchParams, PushRequest, SegmentIdsParams,
        StatsParams, UpdateRequest,
    },
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Json, State},
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;

use super::agent_logs::{
    agent_batch_ingest_logs, agent_create_index, agent_create_v2_index, agent_delete_logs,
    agent_ingest_log, agent_migrate_data, agent_migration_status, agent_query_detail_log,
    agent_reset_migration, agent_search_logs,
};
use super::knowledge_logs::{
    knowledge_clear_all_segments, knowledge_create_index, knowledge_delete_segments,
    knowledge_delete_segments_async, knowledge_get_delete_task_simple_status,
    knowledge_get_delete_task_status, knowledge_get_delete_tasks, knowledge_get_stats,
    knowledge_push_segments, knowledge_query_segment_ids, knowledge_search_logs,
    knowledge_update_segment,
};

/// 健康检查接口
#[utoipa::path(
    get,
    path = "/health",
    tag = "健康检查",
    responses(
        (status = 200, description = "系统健康")
    )
)]
pub async fn health_check() -> impl axum::response::IntoResponse {
    HttpResult::<()>::success()
}

/// 创建应用路由
pub fn init_routes(app_states: Arc<AppStates>) -> Router {
    // 创建 CORS 层
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 创建基础路由
    let router = Router::new()
        // 基础路由
        .route("/health", get(health_check))
        .route("/ready", get(health_check))
        // 知识库路由（已添加 utoipa 注解）
        .route(
            "/api/knowledge/createIndex",
            get(|State(app_states): State<Arc<AppStates>>| knowledge_create_index(app_states)),
        )
        .route(
            "/api/knowledge/search",
            post(
                |State(app_states): State<Arc<AppStates>>,
                 Json(params): Json<KnowledgeSearchParams>| {
                    knowledge_search_logs(app_states, params)
                },
            ),
        )
        .route(
            "/api/knowledge/push",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(request): Json<PushRequest>| {
                    knowledge_push_segments(app_states, request)
                },
            ),
        )
        .route(
            "/api/knowledge/delete",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(params): Json<DeleteParams>| {
                    knowledge_delete_segments(app_states, params)
                },
            ),
        )
        .route(
            "/api/knowledge/delete-async",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(params): Json<DeleteParams>| {
                    knowledge_delete_segments_async(app_states, params)
                },
            ),
        )
        .route(
            "/api/knowledge/clear",
            post(|State(app_states): State<Arc<AppStates>>| {
                knowledge_clear_all_segments(app_states)
            }),
        )
        .route(
            "/api/knowledge/stats",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(params): Json<StatsParams>| {
                    knowledge_get_stats(app_states, params)
                },
            ),
        )
        .route(
            "/api/knowledge/update",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(request): Json<UpdateRequest>| {
                    knowledge_update_segment(app_states, request)
                },
            ),
        )
        .route(
            "/api/knowledge/segment-ids",
            post(
                |State(app_states): State<Arc<AppStates>>, Json(params): Json<SegmentIdsParams>| {
                    knowledge_query_segment_ids(app_states, params)
                },
            ),
        )
        // 删除任务管理路由
        .route(
            "/api/knowledge/delete-tasks",
            get(|State(app_states): State<Arc<AppStates>>| knowledge_get_delete_tasks(app_states)),
        )
        .route(
            "/api/knowledge/delete-tasks/{task_id}",
            get(
                |axum::extract::Path(task_id): axum::extract::Path<String>,
                 State(app_states): State<Arc<AppStates>>| {
                    knowledge_get_delete_task_status(task_id, app_states)
                },
            ),
        )
        .route(
            "/api/knowledge/delete-tasks/{task_id}/status",
            get(
                |axum::extract::Path(task_id): axum::extract::Path<String>,
                 State(app_states): State<Arc<AppStates>>| {
                    knowledge_get_delete_task_simple_status(task_id, app_states)
                },
            ),
        )
        // 其他路由
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
        .route("/api/agent/log/createV2Index", get(agent_create_v2_index))
        .route("/api/agent/log/migrateData", get(agent_migrate_data))
        .route(
            "/api/agent/log/migrationStatus",
            get(agent_migration_status),
        )
        .route(
            "/api/agent/log/migrationReset",
            delete(agent_reset_migration),
        )
        .route(
            "/api/agent/log/delete/{index_name}",
            delete(agent_delete_logs),
        )
        .layer(cors)
        // change the default limit 20MB
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(app_states);

    // 添加 Swagger UI 路由（独立添加）
    let router = router.merge(
        utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", ApiDoc::openapi()),
    );

    // 应用中间件
    middlewares::apply_middlewares(router)
}

/// 生成 OpenAPI 文档 JSON
pub fn generate_openapi_json() -> String {
    serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap()
}
