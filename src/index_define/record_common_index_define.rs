use serde_json::json;

/// 获取通用记录日志的索引定义
pub fn get_record_common_log_index_config(index_name: &str) -> serde_json::Value {
    json!({
        "version": "0.8",
        "index_id": index_name,
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
                    "name": "id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true
                },
                {
                    "name": "timestamp",
                    "type": "datetime",
                    "fast": true,
                    "stored": true
                },
                {
                    "name": "level",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true
                },
                {
                    "name": "message",
                    "type": "text",
                    "tokenizer": "default",
                    "stored": true
                },
                {
                    "name": "service",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "trace_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "span_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "parent_span_id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "operation_name",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "host",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true
                },
                {
                    "name": "ip",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true
                },
                {
                    "name": "duration_ms",
                    "type": "i64",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "error",
                    "type": "bool",
                    "stored": true,
                    "fast": true
                },
                {
                    "name": "tags",
                    "type": "json",
                    "stored": true
                },
                {
                    "name": "metadata",
                    "type": "json",
                    "stored": true
                }
            ],
            "tag_fields": ["service", "trace_id"],
            "timestamp_field": "timestamp"
        }
    })
}
