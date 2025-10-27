use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub service: Option<String>,

    // 调用链相关字段 - 增强与Jaeger集成
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>, // 新增：父span ID
    pub operation_name: Option<String>, // 新增：操作名称

    // 系统环境信息
    pub host: Option<String>,        // 新增：主机名
    pub ip: Option<String>,          // 新增：IP地址
    pub app_version: Option<String>, // 新增：应用版本

    // 性能指标
    pub duration_ms: Option<i64>, // 新增：操作耗时
    pub status_code: Option<i32>, // 新增：状态码
    pub error: Option<bool>,      // 新增：是否有错误

    // 自定义标签和上下文
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<serde_json::Value>, // 新增：标签，用于过滤和查询
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

impl Default for LogEntry {
    fn default() -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            timestamp: Utc::now(),
            level: LogLevel::default(),
            message: String::new(),
            service: None,
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            operation_name: None,
            host: None,
            ip: None,
            app_version: None,
            duration_ms: None,
            status_code: None,
            error: None,
            metadata: None,
            tags: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogQuery {
    pub query: String,
    pub start_timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub start_offset: Option<i64>,
    pub max_hits: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogSearchResult {
    pub hits: Vec<LogEntry>,
    pub total_hits: i64,
    pub elapsed_time_ms: i64,
}

// 新增：针对Jaeger的查询参数
#[derive(Debug, Serialize, Deserialize)]
pub struct TraceQuery {
    pub trace_id: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

// 新增：服务依赖关系
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceDependency {
    pub parent: String,
    pub child: String,
    pub calls_count: i64,
}

// 新增：服务统计信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStats {
    pub service_name: String,
    pub operation_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: f64,
}
