use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use log::error;
use std::sync::Arc;

use crate::{
    migration::AgentLogMigrationManager,
    models::{
        AgentLogEntry, AgentLogSearchParams, AgentLogSearchResult, AppStates, HttpResult, PageQuery,
    },
    my_error::AppError,
    services::AgentLogQuickwitService,
};

/// agent智能体,新增日志
#[utoipa::path(
    post,
    path = "/api/agent/ingest",
    tag = "智能体日志",
    request_body = AgentLogEntry,
    responses(
        (status = 200, description = "日志摄取成功"),
        (status = 500, description = "日志摄取失败")
    )
)]
pub async fn agent_ingest_log(
    State(app_states): State<Arc<AppStates>>,
    Json(log): Json<AgentLogEntry>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 调用摄取日志方法（附带索引检查）
    match agent_log_service.ingest_agent_log(&log).await {
        Ok(_) => Ok(HttpResult::<()>::success()),
        Err(e) => {
            error!("摄取智能体日志失败: {}", e);
            let http_result = HttpResult::<()>::error("500", &e.to_string());
            Ok(http_result)
        }
    }
}

/// agent智能体,创建索引
#[utoipa::path(
    get,
    path = "/api/agent/createIndex",
    tag = "智能体日志",
    responses(
        (status = 200, description = "索引创建成功"),
        (status = 500, description = "索引创建失败")
    )
)]
pub async fn agent_create_index(
    State(app_states): State<Arc<AppStates>>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 索引检查或创建
    match agent_log_service.ensure_agent_index_exists().await {
        Ok(_) => Ok(HttpResult::<()>::success()),
        Err(e) => {
            error!("智能体索引检查创建失败: {}", e);
            let http_result = HttpResult::<()>::error("500", &e.to_string());
            Ok(http_result)
        }
    }
}

/// 创建带有新字段配置的新版本索引
#[utoipa::path(
    post,
    path = "/api/agent/createV2Index",
    tag = "智能体日志",
    responses(
        (status = 200, description = "V2索引创建成功"),
        (status = 500, description = "V2索引创建失败")
    )
)]
pub async fn agent_create_v2_index(
    State(app_states): State<Arc<AppStates>>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 创建新版本索引，包含biz_type字段的优化配置
    match agent_log_service.create_named_index("agent_logs_v2").await {
        Ok(_) => {
            let result = HttpResult::<serde_json::Value>::success_with_data(
                serde_json::json!({"message": "新版本索引创建成功", "index_name": "agent_logs_v2"}),
            );
            Ok(result)
        }
        Err(e) => {
            error!("创建新版本索引失败: {}", e);
            let http_result = HttpResult::<serde_json::Value>::error_with_data(
                "500",
                &e.to_string(),
                serde_json::json!({"error": "创建索引失败"}),
            );
            Ok(http_result)
        }
    }
}

/// 迁移数据从旧索引到新索引
#[utoipa::path(
    post,
    path = "/api/agent/migrateData",
    tag = "智能体日志",
    responses(
        (status = 200, description = "数据迁移成功"),
        (status = 500, description = "数据迁移失败")
    )
)]
pub async fn agent_migrate_data(
    State(app_states): State<Arc<AppStates>>,
) -> Result<impl IntoResponse, AppError> {
    let mut migration_manager = AgentLogMigrationManager::new_with_default_storage(app_states)
        .map_err(|e| AppError::InternalServer(format!("创建迁移管理器失败: {}", e)))?;

    // 执行数据迁移
    match migration_manager.migrate().await {
        Ok(_) => {
            let result = HttpResult::<serde_json::Value>::success_with_data(serde_json::json!({
                "message": "数据迁移成功",
                "from_index": "agent_logs",
                "to_index": "agent_logs_v2"
            }));
            Ok(result)
        }
        Err(e) => {
            error!("数据迁移失败: {}", e);
            let http_result = HttpResult::<serde_json::Value>::error_with_data(
                "500",
                &e.to_string(),
                serde_json::json!({"error": "数据迁移失败"}),
            );
            Ok(http_result)
        }
    }
}

/// 查询迁移状态
#[utoipa::path(
    get,
    path = "/api/agent/migrationStatus",
    tag = "智能体日志",
    responses(
        (status = 200, description = "获取迁移状态成功"),
        (status = 404, description = "未找到迁移记录")
    )
)]
pub async fn agent_migration_status(
    State(app_states): State<Arc<AppStates>>,
) -> Result<impl IntoResponse, AppError> {
    let migration_manager = AgentLogMigrationManager::new_with_default_storage(app_states)
        .map_err(|e| AppError::InternalServer(format!("创建迁移管理器失败: {}", e)))?;

    match migration_manager.get_status().await {
        Some(status) => {
            let result = HttpResult::<serde_json::Value>::success_with_data(
                serde_json::to_value(&status).unwrap_or_default(),
            );
            Ok(result)
        }
        None => {
            let result = HttpResult::<serde_json::Value>::error_with_data(
                "404",
                "未找到迁移记录",
                serde_json::json!({"message": "没有进行过数据迁移"}),
            );
            Ok(result)
        }
    }
}

/// 重置迁移状态
#[utoipa::path(
    delete,
    path = "/api/agent/log/migrationReset",
    tag = "智能体日志",
    responses(
        (status = 200, description = "重置迁移状态成功"),
        (status = 500, description = "重置迁移状态失败")
    )
)]
pub async fn agent_reset_migration(
    State(app_states): State<Arc<AppStates>>,
) -> Result<impl IntoResponse, AppError> {
    let migration_manager = AgentLogMigrationManager::new_with_default_storage(app_states)
        .map_err(|e| AppError::InternalServer(format!("创建迁移管理器失败: {}", e)))?;

    match migration_manager.reset_migration().await {
        Ok(_) => {
            let result = HttpResult::<serde_json::Value>::success_with_data(serde_json::json!({
                "message": "迁移状态已重置",
                "note": "下次启动服务或调用迁移接口时将重新执行完整迁移"
            }));
            Ok(result)
        }
        Err(e) => {
            let result = HttpResult::<serde_json::Value>::error_with_data(
                "500",
                "重置迁移状态失败",
                serde_json::json!({
                    "error": e.to_string()
                }),
            );
            Ok(result)
        }
    }
}

/// agent智能体,批量新增日志
#[utoipa::path(
    post,
    path = "/api/agent/batchIngest",
    tag = "智能体日志",
    request_body = Vec<AgentLogEntry>,
    responses(
        (status = 200, description = "批量日志摄取成功"),
        (status = 500, description = "批量日志摄取失败")
    )
)]
pub async fn agent_batch_ingest_logs(
    State(app_states): State<Arc<AppStates>>,
    Json(logs): Json<Vec<AgentLogEntry>>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 调用批量摄取日志方法（附带索引检查）
    match agent_log_service
        .batch_ingest_agent_logs_with_no_index_check(&logs)
        .await
    {
        Ok(_) => Ok(HttpResult::<()>::success()),
        Err(e) => {
            error!("批量摄取智能体日志失败: {}", e);
            let http_result = HttpResult::<()>::error("500", &e.to_string());
            Ok(http_result)
        }
    }
}

/// agent智能体,分页查询日志
#[utoipa::path(
    post,
    path = "/api/agent/search",
    tag = "智能体日志",
    request_body = PageQuery<AgentLogSearchParams>,
    responses(
        (status = 200, description = "搜索成功"),
        (status = 500, description = "搜索失败")
    )
)]
#[axum::debug_handler]
pub async fn agent_search_logs(
    State(app_states): State<Arc<AppStates>>,
    Json(params): Json<PageQuery<AgentLogSearchParams>>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 调用搜索方法
    match agent_log_service.search_agent_logs(params).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => {
            error!("搜索智能体日志失败: {}", e);
            //如果报错 , HttpResult 结构返回,在 HttpResult 的message放入错误信息,code是非"0000"错误码
            let empty_result = AgentLogSearchResult::new(vec![], 0, 0);
            let http_result = HttpResult::error_with_data("500", &e.to_string(), empty_result);
            Ok(http_result)
        }
    }
}

/// agent智能体,查询单个日志详情
#[utoipa::path(
    post,
    path = "/api/agent/queryDetail",
    tag = "智能体日志",
    request_body = PageQuery<AgentLogSearchParams>,
    responses(
        (status = 200, description = "查询成功"),
        (status = 500, description = "查询失败")
    )
)]
#[axum::debug_handler]
pub async fn agent_query_detail_log(
    State(app_states): State<Arc<AppStates>>,
    Json(params): Json<PageQuery<AgentLogSearchParams>>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 调用搜索方法
    match agent_log_service.search_agent_log_detail(params).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => {
            error!("搜索智能体日志失败: {}", e);
            //如果报错 , HttpResult 结构返回,在 HttpResult 的message放入错误信息,code是非"0000"错误码
            let empty_result = AgentLogSearchResult::new(vec![], 0, 0);
            let http_result = HttpResult::error_with_data("500", &e.to_string(), empty_result);
            Ok(http_result)
        }
    }
}

/// 删除索引
#[utoipa::path(
    delete,
    path = "/api/agent/delete/{index_name}",
    tag = "智能体日志",
    params(
        ("index_name" = String, Path, description = "索引名称")
    ),
    responses(
        (status = 200, description = "删除成功"),
        (status = 500, description = "删除失败")
    )
)]
#[axum::debug_handler]
pub async fn agent_delete_logs(
    State(app_states): State<Arc<AppStates>>,
    Path(index_name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // 创建AgentLogQuickwitService
    let agent_log_service = AgentLogQuickwitService::new(app_states);

    // 调用搜索方法
    match agent_log_service.delete_agent_logs(index_name).await {
        Ok(_) => Ok(HttpResult::<()>::success()),
        Err(e) => {
            error!("删除智能体日志失败: {}", e);
            //如果报错 , HttpResult 结构返回,在 HttpResult 的message放入错误信息,code是非"0000"错误码
            let http_result = HttpResult::<()>::error("500", &e.to_string());
            Ok(http_result)
        }
    }
}
