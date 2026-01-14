// 导出智能体日志索引配置
pub use agent_index_define::get_agent_index_config;

// 导出知识库索引配置
pub use knowledge_index_define::get_knowledge_index_config;

// 导出通用日志索引配置
pub use record_common_index_define::get_record_common_log_index_config;

// 索引名称常量
pub const DEFAULT_AGENT_LOG_INDEX: &str = "agent_logs_v2";
pub const DEFAULT_AGENT_LOG_INDEX_V1: &str = "agent_logs";
pub const DEFAULT_KNOWLEDGE_INDEX: &str = "knowledge_segments_v1";
pub const DEFAULT_RECORD_COMMON_LOG_INDEX: &str = "record_common_logs";

// 模块声明
pub mod agent_index_define;
pub mod knowledge_index_define;
pub mod record_common_index_define;
