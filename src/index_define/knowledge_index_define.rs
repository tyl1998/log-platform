use serde_json::json;

/// 知识库索引默认名称
pub const DEFAULT_KNOWLEDGE_INDEX: &str = "knowledge_segments_v1";

/// 获取知识库索引配置
/// 基于QuickWit索引规范创建的知识库索引配置
pub fn get_knowledge_index_config(index_id: &str) -> serde_json::Value {
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
                    "name": "id",
                    "type": "text",
                    "tokenizer": "raw",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "raw_id",
                    "type": "u64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "kb_id",
                    "type": "u64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "doc_id",
                    "type": "u64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "tenant_id",
                    "type": "i64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "space_id",
                    "type": "i64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "raw_txt",
                    "type": "text",
                    "tokenizer": "chinese_compatible",
                    "record": "position",
                    "stored": true,
                    "indexed": true,
                    "fieldnorms": true
                },
                {
                    "name": "sort_index",
                    "type": "i64",
                    "stored": true,
                    "indexed": true,
                    "fast": true
                },
                {
                    "name": "created",
                    "type": "datetime",
                    "input_formats": ["rfc3339"],
                    "stored": true,
                    "indexed": true,
                    "fast": true,
                    "fast_precision": "seconds"
                }
            ],
            "tag_fields": ["tenant_id","kb_id", "doc_id"],
            "timestamp_field": "created",
            "store_source": true,
            "partition_key": "tenant_id"
        },
        "search_settings": {
            "default_search_fields": ["raw_txt"]
        },
        "indexing_settings": {
            "commit_timeout_secs": 10,
            "split_num_docs_target": 1000000,
            "merge_policy": {
                "type": "stable_log",
                "min_level_num_docs": 100000,
                "merge_factor": 10,
                "max_merge_factor": 12,
                "maturation_period": "1m"
            }
        }
    })
}
