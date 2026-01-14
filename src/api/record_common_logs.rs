use crate::{
    models::{AppStates, HttpResult, LogEntry, LogQuery},
    my_error::AppError,
    services::RecordCommonLogQuickwitService,
};
use axum::{Json, extract::Query, extract::State, response::IntoResponse};
use chrono::{DateTime, Utc};
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 处理单条日志写入
#[utoipa::path(
    post,
    path = "/api/logs/ingest",
    tag = "通用日志",
    request_body = LogEntry,
    responses(
        (status = 200, description = "日志写入成功"),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "日志写入失败")
    )
)]
pub async fn ingest_log(
    State(service): State<Arc<AppStates>>,
    Json(log): Json<LogEntry>,
) -> Result<impl IntoResponse, AppError> {
    info!("处理单条日志写入: {:?}", log);
    let service = RecordCommonLogQuickwitService::new(service.clone());
    service.ingest_log(&log).await?;
    Ok(HttpResult::<()>::success())
}

/// 处理批量日志写入
#[utoipa::path(
    post,
    path = "/api/logs/batchIngest",
    tag = "通用日志",
    request_body = Vec<LogEntry>,
    responses(
        (status = 200, description = "批量日志写入成功"),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "批量日志写入失败")
    )
)]
pub async fn batch_ingest_logs(
    State(service): State<Arc<AppStates>>,
    Json(logs): Json<Vec<LogEntry>>,
) -> Result<impl IntoResponse, AppError> {
    if logs.is_empty() {
        return Err(AppError::BadRequest("日志列表为空".to_string()));
    }

    let service = RecordCommonLogQuickwitService::new(service.clone());
    service.batch_ingest_logs(&logs).await?;
    Ok(HttpResult::<()>::success())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

/// 处理日志搜索
#[utoipa::path(
    get,
    path = "/api/logs/search",
    tag = "通用日志",
    params(
        ("query" = Option<String>, Query, description = "搜索关键词"),
        ("start_time" = Option<String>, Query, description = "开始时间"),
        ("end_time" = Option<String>, Query, description = "结束时间"),
        ("offset" = Option<i64>, Query, description = "偏移量"),
        ("limit" = Option<i64>, Query, description = "返回数量")
    ),
    responses(
        (status = 200, description = "搜索成功"),
        (status = 500, description = "搜索失败")
    )
)]
pub async fn search_logs(
    State(service): State<Arc<AppStates>>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let query = LogQuery {
        query: params.query.unwrap_or_else(|| "*".to_string()),
        start_timestamp: params.start_time.map(|t| t.timestamp()),
        end_timestamp: params.end_time.map(|t| t.timestamp()),
        start_offset: params.offset,
        max_hits: params.limit,
    };

    let service = RecordCommonLogQuickwitService::new(service.clone());
    let result = service.search_logs(&query).await?;
    Ok(HttpResult::success_with_data(result))
}

/// 健康检查
#[utoipa::path(
    get,
    path = "/health",
    tag = "健康检查",
    responses(
        (status = 200, description = "服务健康")
    )
)]
pub async fn health_check() -> impl IntoResponse {
    HttpResult::<()>::success()
}
