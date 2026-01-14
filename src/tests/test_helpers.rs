#![cfg(test)]

use crate::{
    api::init_routes,
    models::{AgentLogEntry, AppStates},
    tests::tests::{obtain_test_config, setup},
};
use axum::{body::Body, http::Request};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// 测试辅助工具
pub struct TestHelper;

impl TestHelper {
    /// 创建测试应用
    pub fn create_app() -> axum::Router {
        setup();
        let config = obtain_test_config();
        let app_states = Arc::new(AppStates::new(config.quickwit));
        init_routes(app_states)
    }

    /// 创建随机的智能体日志条目
    pub fn create_random_agent_log() -> AgentLogEntry {
        let uuid = Uuid::new_v4().simple().to_string();
        AgentLogEntry {
            request_id: format!("req_{}", &uuid[..8]),
            agent_id: Some("agent_001".to_string()),
            message_id: Some(format!("msg_{}", &uuid[8..16])),
            conversation_id: Some(format!("session_{}", &uuid[16..24])),
            user_uid: Some(format!("user_{}", &uuid[24..32])),
            tenant_id: "test_tenant".to_string(),
            space_id: Some(format!("space_{}", &uuid[24..32])),
            user_input: Some("测试用户输入".to_string()),
            output: Some("测试系统输出".to_string()),
            execute_result: None,
            input_token: Some(10),
            output_token: Some(20),
            request_start_time: Some(Utc::now()),
            request_end_time: Some(Utc::now()),
            elapsed_time_ms: Some(100),
            node_type: Some("llm".to_string()),
            status: Some("success".to_string()),
            node_name: Some("test_node".to_string()),
            created_at: None,
            updated_at: None,
            user_id: Some(1),
            user_name: Some("张三".to_string()),
            biz_type: Some("agent".to_string()),
        }
    }

    /// 创建批量测试日志数据
    pub fn create_batch_agent_logs(count: usize, tenant_id: &str) -> Vec<AgentLogEntry> {
        (0..count)
            .map(|i| {
                let mut log = Self::create_random_agent_log();
                log.tenant_id = tenant_id.to_string();
                log.request_id = format!("batch_req_{:03}", i);
                log
            })
            .collect()
    }

    /// 创建POST请求
    pub fn create_post_request(uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// 创建GET请求
    pub fn create_get_request(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    /// 创建带CORS头的OPTIONS请求
    pub fn create_cors_preflight_request(uri: &str) -> Request<Body> {
        Request::builder()
            .method("OPTIONS")
            .uri(uri)
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "Content-Type")
            .body(Body::empty())
            .unwrap()
    }

    /// 验证响应是否包含CORS头
    pub fn verify_cors_headers(headers: &axum::http::HeaderMap) -> bool {
        headers.contains_key("access-control-allow-origin")
    }

    /// 创建无效的JSON请求体
    pub fn create_invalid_json_body() -> String {
        "{ invalid json".to_string()
    }

    /// 创建空的JSON对象
    pub fn create_empty_json_body() -> String {
        "{}".to_string()
    }

    /// 创建包含排序的搜索请求JSON
    pub fn create_search_with_sorting_json(
        tenant_id: &str,
        sort_field: &str,
        ascending: bool,
    ) -> String {
        json!({
            "current": 1,
            "pageSize": 10,
            "queryFilter": {
                "tenant_id": tenant_id
            },
            "orders": [
                {
                    "column": sort_field,
                    "asc": ascending
                }
            ]
        })
        .to_string()
    }

    /// 创建多字段排序的搜索请求JSON
    pub fn create_search_with_multi_sorting_json(tenant_id: &str) -> String {
        json!({
            "current": 1,
            "pageSize": 10,
            "queryFilter": {
                "tenant_id": tenant_id
            },
            "orders": [
                {
                    "column": "request_start_time",
                    "asc": false
                },
                {
                    "column": "created_at",
                    "asc": true
                }
            ]
        })
        .to_string()
    }

    /// 创建包含无效排序字段的搜索请求JSON
    pub fn create_search_with_invalid_sorting_json(tenant_id: &str) -> String {
        json!({
            "current": 1,
            "pageSize": 10,
            "queryFilter": {
                "tenant_id": tenant_id
            },
            "orders": [
                {
                    "column": "request_id", // 无效排序字段
                    "asc": false
                },
                {
                    "column": "conversation_id", // 无效排序字段
                    "asc": true
                }
            ]
        })
        .to_string()
    }

    /// 获取测试用的租户ID列表
    pub fn get_test_tenant_ids() -> Vec<&'static str> {
        vec!["tenant_001", "tenant_002", "tenant_test", "test_tenant"]
    }

    /// 获取有效的排序字段列表
    pub fn get_valid_sort_fields() -> Vec<&'static str> {
        vec![
            "input_token",
            "output_token",
            "request_start_time",
            "request_end_time",
            "elapsed_time_ms",
            "created_at",
        ]
    }

    /// 获取无效的排序字段列表
    pub fn get_invalid_sort_fields() -> Vec<&'static str> {
        vec![
            "request_id",
            "conversation_id",
            "user_uid",
            "tenant_id",
            "space_id",
            "user_input",
            "output",
            "node_type",
            "status",
            "node_name",
        ]
    }

    /// 创建大批量测试数据
    pub fn create_large_batch_logs(count: usize) -> Vec<AgentLogEntry> {
        Self::create_batch_agent_logs(count, "bulk_test_tenant")
    }

    /// 创建性能测试用的搜索参数
    pub fn create_performance_search_params(page_size: i64) -> String {
        json!({
            "current": 1,
            "pageSize": page_size,
            "queryFilter": {
                "tenant_id": "performance_test_tenant"
            }
        })
        .to_string()
    }
}
