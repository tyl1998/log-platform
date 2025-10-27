use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("内部服务器错误: {0}")]
    InternalServer(String),

    #[error("QuickWit错误: {0}")]
    QuickWit(String),

    #[error("无效请求: {0}")]
    BadRequest(String),

    #[error("未找到: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServer(ref msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::QuickWit(ref msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::BadRequest(ref msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(ref msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = json!({
            "error": self.to_string(),
            "message": error_message,
        });

        (status, axum::Json(body)).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::QuickWit(format!("QuickWit API错误: {}", err))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalServer(format!("内部错误: {}", err))
    }
}
