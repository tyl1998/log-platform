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
pub async fn health_check() -> impl IntoResponse {
    HttpResult::<()>::success()
}
