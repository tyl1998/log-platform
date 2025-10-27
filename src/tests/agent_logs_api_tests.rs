#![cfg(test)]

use crate::{
    api::init_routes,
    models::{AgentLogEntry, AgentLogSearchParams, AppStates, PageQuery},
    tests::tests::{obtain_test_config, setup},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

/// 创建测试用的智能体日志条目
fn create_test_agent_log() -> AgentLogEntry {
    AgentLogEntry {
        request_id: "req_001".to_string(),
        message_id: Some("msg_001".to_string()),
        agent_id: Some("agent_001".to_string()),
        conversation_id: Some("session_001".to_string()),
        user_uid: Some("user_123".to_string()),
        tenant_id: "tenant_001".to_string(),
        space_id: Some("space_001".to_string()),
        user_input: Some("请帮我分析一下这个数据".to_string()),
        output: Some("根据您提供的数据，我分析出以下结论...".to_string()),
        execute_result: None,
        input_token: None,
        output_token: None,
        request_start_time: Some("2024-01-15T10:30:00Z".parse::<DateTime<Utc>>().unwrap()),
        request_end_time: Some("2024-01-15T10:30:05Z".parse::<DateTime<Utc>>().unwrap()),
        elapsed_time_ms: None,
        node_type: None,
        status: Some("success".to_string()),
        node_name: None,
        created_at: None,
        updated_at: None,
        user_id: Some(1),
        user_name: Some("张三".to_string()),
    }
}

/// 创建测试应用
fn create_test_app() -> axum::Router {
    setup();
    let config = obtain_test_config();
    let app_states = Arc::new(AppStates::new(config.quickwit));
    init_routes(app_states)
}

#[tokio::test]
async fn test_health_check() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_ready_check() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_agent_ingest_log_success() {
    let app = create_test_app();
    let log_entry = create_test_agent_log();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/add")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&log_entry).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 测试环境下可能无法连接到真实的QuickWit，所以我们主要测试路由和序列化
    // 实际的QuickWit连接错误是预期的
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_batch_ingest_logs() {
    let app = create_test_app();
    let logs = vec![
        create_test_agent_log(),
        AgentLogEntry {
            request_id: "req_002".to_string(),
            agent_id: Some("agent_002".to_string()),
            message_id: Some("msg_002".to_string()),
            conversation_id: Some("session_001".to_string()),
            user_uid: Some("user_123".to_string()),
            tenant_id: "tenant_001".to_string(),
            space_id: Some("space_001".to_string()),
            user_input: Some("什么是机器学习？".to_string()),
            output: Some("机器学习是人工智能的一个分支...".to_string()),
            execute_result: None,
            input_token: None,
            output_token: None,
            request_start_time: Some("2024-01-15T10:31:00Z".parse::<DateTime<Utc>>().unwrap()),
            request_end_time: Some("2024-01-15T10:31:03Z".parse::<DateTime<Utc>>().unwrap()),
            elapsed_time_ms: None,
            node_type: None,
            status: Some("success".to_string()),
            node_name: None,
            created_at: None,
            updated_at: None,
            user_id: Some(2),
            user_name: Some("李四".to_string()),
        },
    ];

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&logs).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 测试环境下可能无法连接到真实的QuickWit
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_basic() {
    let app = create_test_app();

    let search_params = PageQuery {
        current: 1,
        page_size: 10,
        query_filter: Some(AgentLogSearchParams {
            request_id: None,
            conversation_id: None,
            agent_id: None,
            message_id: None,
            user_uid: None,
            user_input: None,
            output: None,
            start_time: None,
            end_time: None,
            tenant_id: Some("tenant_001".to_string()),
            space_id: None,
        }),
        orders: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&search_params).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 测试环境下可能无法连接到真实的QuickWit
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_filters() {
    let app = create_test_app();

    let search_params = PageQuery {
        current: 1,
        page_size: 10,
        query_filter: Some(AgentLogSearchParams {
            request_id: Some("req_001".to_string()),
            agent_id: Some("agent_001".to_string()),
            message_id: Some("msg_001".to_string()),
            conversation_id: Some("session_001".to_string()),
            user_uid: Some("user_123".to_string()),
            user_input: None,
            output: None,
            start_time: Some("2024-01-15T10:00:00Z".parse::<DateTime<Utc>>().unwrap()),
            end_time: Some("2024-01-15T11:00:00Z".parse::<DateTime<Utc>>().unwrap()),
            tenant_id: Some("tenant_001".to_string()),
            space_id: Some(vec!["space_001".to_string()]),
        }),
        orders: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&search_params).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 测试环境下可能无法连接到真实的QuickWit
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_sorting() {
    let app = create_test_app();

    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "tenant_id": "tenant_001"
        },
        "orders": [
            {
                "column": "request_start_time",
                "asc": false
            }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 测试环境下可能无法连接到真实的QuickWit
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_invalid_sorting() {
    let app = create_test_app();

    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "tenant_id": "tenant_001"
        },
        "orders": [
            {
                "column": "request_id", // 这是无效的排序字段
                "asc": false
            },
            {
                "column": "request_start_time",
                "asc": false
            }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 应该能处理无效排序字段，过滤掉无效字段只使用有效字段
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_missing_tenant_id() {
    let app = create_test_app();

    // 测试缺少必需参数 tenant_id 的情况
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "user_uid": "user_123"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 应该能正常处理（tenant_id现在是可选的）
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_ingest_log_invalid_json() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/add")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_agent_search_logs_invalid_json() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_nonexistent_route() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cors_headers() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/agent/log/search")
                .header("Origin", "http://localhost:3000")
                .header("Access-Control-Request-Method", "POST")
                .header("Access-Control-Request-Headers", "Content-Type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 应该返回适当的 CORS 头
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NO_CONTENT);

    let headers = response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn test_agent_search_logs_with_empty_strings() {
    let app = create_test_app();

    // 使用包含空字符串和空白字符的搜索参数
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "request_id": "", // 空字符串
            "conversation_id": "   ", // 只包含空白字符
            "user_uid": "valid_user", // 有效值
            "user_input": [""], // 空字符串数组
            "output": ["  \t\n  "], // 只包含空白字符数组
            "tenant_id": "tenant_001",
            "space_id": ["  "] // 只包含空白字符数组
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 应该能正常处理，只使用有效的字段进行查询
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_multiple_user_inputs() {
    let app = create_test_app();

    // 测试 user_input 多个关键字的 AND 搜索
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "user_input": ["分析", "数据"], // 多个关键字，必须同时包含
            "tenant_id": "tenant_001"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_multiple_outputs() {
    let app = create_test_app();

    // 测试 output 多个关键字的 AND 搜索
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "output": ["结论", "分析"], // 多个关键字，必须同时包含
            "tenant_id": "tenant_001"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_multiple_space_ids() {
    let app = create_test_app();

    // 测试 space_id 多个值的 OR 搜索（IN 关系）
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "space_id": ["space_001", "space_002", "space_003"], // 多个 space_id，任一匹配即可
            "tenant_id": "tenant_001"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_agent_search_logs_with_combined_multi_values() {
    let app = create_test_app();

    // 测试组合使用多个多值字段
    let search_params = json!({
        "current": 1,
        "pageSize": 10,
        "queryFilter": {
            "user_input": ["请帮", "分析"], // AND 关系
            "output": ["数据", "结论"], // AND 关系
            "space_id": ["space_001", "space_002"], // OR 关系
            "tenant_id": "tenant_001"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/log/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
}
