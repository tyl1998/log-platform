use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

/// 通用HTTP响应结构
#[derive(serde::Serialize)]
pub struct HttpResult<T> {
    // "0000" 表示成功,其他都是失败
    pub code: String,
    // 错误信息
    pub message: String,
    // 返回数据
    pub data: T,
    // 后端服务器生成的tid,用于追踪日志,排查错误
    pub tid: String,
}

impl<T> HttpResult<T> {
    /// 创建成功响应（无数据，仅用于操作类接口）
    pub fn success() -> HttpResult<()> {
        HttpResult {
            code: "0000".to_string(),
            message: "操作成功".to_string(),
            data: (),
            tid: Uuid::new_v4().to_string(),
        }
    }

    /// 创建带数据的成功响应
    pub fn success_with_data(data: T) -> HttpResult<T> {
        HttpResult {
            code: "0000".to_string(),
            message: "操作成功".to_string(),
            data,
            tid: Uuid::new_v4().to_string(),
        }
    }

    /// 创建失败响应
    pub fn error(code: &str, message: &str) -> HttpResult<()> {
        HttpResult {
            code: code.to_string(),
            message: message.to_string(),
            data: (),
            tid: Uuid::new_v4().to_string(),
        }
    }

    /// 创建带数据的失败响应
    pub fn error_with_data(code: &str, message: &str, data: T) -> HttpResult<T> {
        HttpResult {
            code: code.to_string(),
            message: message.to_string(),
            data,
            tid: Uuid::new_v4().to_string(),
        }
    }
}

impl<T: Serialize> IntoResponse for HttpResult<T> {
    fn into_response(self) -> Response {
        // 统一返回200状态码，由前端根据code判断具体结果
        let status_code = StatusCode::OK;

        // 将HttpResult序列化为JSON
        let json_response = Json(self);

        // 构造HTTP响应
        (status_code, json_response).into_response()
    }
}
