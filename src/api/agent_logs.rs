use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use log::error;
use std::sync::Arc;

use crate::{
    models::{
        AgentLogEntry, AgentLogSearchParams, AgentLogSearchResult, AppStates, HttpResult, PageQuery,
    },
    my_error::AppError,
    services::AgentLogQuickwitService,
};

/// agent智能体,新增日志
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

/// agent智能体,新增日志
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

/// agent智能体,批量新增日志
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
