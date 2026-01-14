use crate::impl_searchable_params;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

///智能体调试大模型的日志结构定义
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentLogEntry {
    /// 请求ID，唯一标识一次请求（必填）
    pub request_id: String,
    /// 消息ID,（必填）
    pub message_id: Option<String>,
    /// 会话ID（必填）
    pub conversation_id: Option<String>,
    /// 智能体ID
    pub agent_id: Option<String>,
    /// 用户UID,个别场景下,用户没有uid,可以不传
    pub user_uid: Option<String>,
    /// 租户ID，用于租户隔离（必填）
    pub tenant_id: String,
    /// 空间ID，用户可以有多个空间
    pub space_id: Option<String>,
    /// 用户输入的内容
    pub user_input: Option<String>,
    /// 系统输出的内容
    pub output: Option<String>,
    /// 执行结果,json文本,用户自己定义json存储
    pub execute_result: Option<String>,
    /// 输入token数量
    pub input_token: Option<i32>,
    /// 输出token数量
    pub output_token: Option<i32>,
    /// 请求开始时间
    pub request_start_time: Option<DateTime<Utc>>,
    /// 请求结束时间
    pub request_end_time: Option<DateTime<Utc>>,
    /// 耗时(毫秒)
    pub elapsed_time_ms: Option<i64>,
    /// 节点类型
    pub node_type: Option<String>,
    /// 节点状态
    pub status: Option<String>,
    /// 节点名称
    pub node_name: Option<String>,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
    /// 用户ID
    pub user_id: Option<i64>,
    /// 用户名
    pub user_name: Option<String>,
    /// 业务类型，如:agent,mcp等，用于区分不同的业务日志类型
    pub biz_type: Option<String>,
}

/// 日志搜索请求参数
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentLogSearchParams {
    /// 请求ID
    pub request_id: Option<String>,
    /// 消息ID
    pub message_id: Option<String>,
    /// 会话ID
    pub conversation_id: Option<String>,
    /// 智能体ID
    pub agent_id: Option<String>,
    /// 用户UID
    pub user_uid: Option<String>,
    /// 用户输入,需要支持全文检索，支持多个关键字（AND关系）
    pub user_input: Option<Vec<String>>,
    /// 系统输出,需要支持全文检索，支持多个关键字（AND关系）
    pub output: Option<Vec<String>>,
    /// 开始时间
    pub start_time: Option<DateTime<Utc>>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 租户ID，用于租户隔离，确保只查询特定租户的日志
    pub tenant_id: Option<String>,
    /// 空间ID，可选，用于查询特定空间的日志，支持多个ID（OR关系）
    pub space_id: Option<Vec<String>>,
    /// 业务类型，用于查询特定业务类型的日志，支持多个类型（OR关系）
    pub biz_type: Option<Vec<String>>,
}

// 使用新设计的宏自动处理字段
impl_searchable_params!(
    AgentLogSearchParams,
    string_fields: [request_id, message_id, conversation_id, agent_id, user_uid, tenant_id],
    array_fields: [
        space_id => " OR ",
        biz_type => " OR ",
        user_input => " AND ",
        output => " AND "
    ]
);

/// 日志分页结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentLogSearchResult {
    /// 处理耗时(毫秒)
    pub elapsed_time_ms: i64,
    /// 查询数据列表
    pub records: Vec<AgentLogEntry>,
    /// 总数
    pub total: i64,
    /// 每页显示条数，默认10
    #[serde(default = "default_size")]
    pub size: i64,
    /// 当前页
    #[serde(default = "default_current")]
    pub current: i64,
}

/// 默认每页大小
fn default_size() -> i64 {
    10
}

/// 默认当前页
fn default_current() -> i64 {
    1
}

impl AgentLogSearchResult {
    /// 创建新的搜索结果
    pub fn new(records: Vec<AgentLogEntry>, total: i64, elapsed_time_ms: i64) -> Self {
        Self {
            records,
            total,
            elapsed_time_ms,
            size: default_size(),
            current: default_current(),
        }
    }

    /// 设置页码和每页数量
    pub fn with_pagination(mut self, current: i64, size: i64) -> Self {
        self.current = current;
        self.size = size;
        self
    }
}
