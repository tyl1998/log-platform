use serde_json::json;

///这个是agent的 AgentLogEntry 结构体的 索引定义
///
/// 基于QuickWit索引规范创建的智能体日志索引配置
pub fn get_agent_index_config(index_id: &str) -> serde_json::Value {
    json!({
        "version": "0.7",
        "index_id": index_id,
        "doc_mapping": {
            "mode": "dynamic",
            "dynamic_mapping": {
                "indexed": true,
                "stored": true,
                "tokenizer": "default",
                "record": "basic"
            },
            "field_mappings": [
                {
                    "name": "request_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "message_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "conversation_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "agent_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "user_uid",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "tenant_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "space_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "user_input",
                    "type": "text",
                    "tokenizer": "chinese_compatible",
                    "record": "position",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "output",
                    "type": "text",
                    "tokenizer": "chinese_compatible",
                    "record": "position",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "execute_result",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "input_token",
                    "type": "i64",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "output_token",
                    "type": "i64",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "request_start_time",
                    "type": "datetime",
                    "fast": true,
                    "stored": true
                },
                {
                    "name": "request_end_time",
                    "type": "datetime",
                    "fast": true,
                    "stored": true
                },
                {
                    "name": "elapsed_time_ms",
                    "type": "i64",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "node_type",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "status",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "node_name",
                    "type": "text",
                    "tokenizer": "chinese_compatible",
                    "record": "position",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "created_at",
                    "type": "datetime",
                    "fast": true,
                    "stored": true
                },
                {
                    "name": "updated_at",
                    "type": "datetime",
                    "stored": true
                },
                {
                    "name": "user_id",
                    "type": "i64",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "user_name",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true
                },
                {
                    "name": "biz_type",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                }
            ],
            "tag_fields": ["tenant_id", "user_uid", "biz_type"],
            "timestamp_field": "request_start_time",
            "store_source": false
        },
        "search_settings": {
            "default_search_fields": ["user_input", "output"]
        },
        "retention": {
            "period": "90 days",
            "schedule": "daily"
        },
        "indexing_settings": {
            "commit_timeout_secs": 3
        }
    })
}
