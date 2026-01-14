#![cfg(test)]

use crate::{
    api::init_routes,
    models::{AppStates, KnowledgeRawSegment},
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

/// 创建测试用的知识库分段数据
fn create_test_knowledge_segment() -> KnowledgeRawSegment {
    KnowledgeRawSegment {
        id: Some("seg_001".to_string()),
        raw_id: 1001,
        kb_id: 1,
        doc_id: 101,
        raw_txt: "这是一个测试文档，用于验证知识库全文检索功能。包含中文内容和中国语料库数据。"
            .to_string(),
        sort_index: Some(1),
        tenant_id: 1001,
        space_id: 1001,
        created: Some(Utc::now()),
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
async fn test_knowledge_push_segments_success() {
    let app = create_test_app();
    let segment = create_test_knowledge_segment();

    let request_body = json!( {
        "segments": [segment]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/push")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_push_segments_empty() {
    let app = create_test_app();

    let request_body = json!( {
        "segments": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/push")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_search_success() {
    let app = create_test_app();

    let search_params = json!( {
        "query": "测试",
        "tenant_id": 1001,
        "limit": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_search_empty_query() {
    let app = create_test_app();

    let search_params = json!( {
        "query": "",
        "tenant_id": 1001,
        "limit": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/search")
                .header("content-type", "application/json")
                .body(Body::from(search_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_get_stats() {
    let app = create_test_app();

    let stats_params = json!( {
        "tenant_id": 1001
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/stats")
                .header("content-type", "application/json")
                .body(Body::from(stats_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_delete_success() {
    let app = create_test_app();

    let delete_params = json!( {
        "kb_id": [1],
        "tenant_id": 1001
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/delete")
                .header("content-type", "application/json")
                .body(Body::from(delete_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_update_segment() {
    let app = create_test_app();

    let update_params = json!( {
        "raw_id": "1001",
        "tenant_id": 1001,
        "raw_txt": "更新后的文档内容"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/update")
                .header("content-type", "application/json")
                .body(Body::from(update_params.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_knowledge_clear_all() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/knowledge/clear")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
