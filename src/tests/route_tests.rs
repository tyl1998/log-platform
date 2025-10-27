#![cfg(test)]

use crate::{
    api::init_routes,
    models::AppStates,
    tests::tests::{obtain_test_config, setup},
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt;

/// 创建测试应用
fn create_test_app() -> axum::Router {
    setup();
    let config = obtain_test_config();
    let app_states = Arc::new(AppStates::new(config.quickwit));
    init_routes(app_states)
}

#[tokio::test]
async fn test_all_routes_exist() {
    let app = create_test_app();

    // 测试所有定义的路由是否存在
    let routes = vec![
        ("/health", Method::GET),
        ("/ready", Method::GET),
        ("/api/logs", Method::POST),
        ("/api/logs/batch", Method::POST),
        ("/api/logs/search", Method::GET),
        ("/api/agent/log/add", Method::POST),
        ("/api/agent/log/batch", Method::POST),
        ("/api/agent/log/search", Method::POST),
    ];

    for (path, method) in routes {
        let request = Request::builder()
            .method(method.clone())
            .uri(path)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();

        // 路由应该存在（不是404），但可能因为缺少body或连接问题而返回其他错误
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Route {} {} should exist",
            method,
            path
        );
    }
}

#[tokio::test]
async fn test_method_not_allowed() {
    let app = create_test_app();

    // 测试使用错误的HTTP方法
    let response = app
        .oneshot(
            Request::builder()
                .method("POST") // 应该是GET
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_get_method_on_post_only_routes() {
    let app = create_test_app();

    // 测试在仅支持POST的路由上使用GET方法
    let post_only_routes = vec![
        "/api/logs",
        "/api/logs/batch",
        "/api/agent/log/add",
        "/api/agent/log/batch",
        "/api/agent/log/search",
    ];

    for route in post_only_routes {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(route)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "Route {} should not allow GET method",
            route
        );
    }
}

#[tokio::test]
async fn test_post_method_on_get_only_routes() {
    let app = create_test_app();

    // 测试在仅支持GET的路由上使用POST方法
    let get_only_routes = vec!["/health", "/ready", "/api/logs/search"];

    for route in get_only_routes {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(route)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "Route {} should not allow POST method",
            route
        );
    }
}

#[tokio::test]
async fn test_invalid_routes() {
    let app = create_test_app();

    let invalid_routes = vec![
        "/invalid",
        "/api",
        "/api/invalid",
        "/api/agent",
        "/api/agent/invalid",
        "/api/agent/log",
        "/api/agent/log/invalid",
    ];

    for route in invalid_routes {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(route).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Route {} should return 404",
            route
        );
    }
}

#[tokio::test]
async fn test_cors_preflight_options() {
    let app = create_test_app();

    // 测试CORS预检请求
    let routes_to_test = vec![
        "/api/agent/log/add",
        "/api/agent/log/batch",
        "/api/agent/log/search",
        "/api/logs",
        "/api/logs/batch",
    ];

    for route in routes_to_test {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri(route)
                    .header("Origin", "http://localhost:3000")
                    .header("Access-Control-Request-Method", "POST")
                    .header("Access-Control-Request-Headers", "Content-Type")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // CORS预检请求应该返回成功状态
        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::NO_CONTENT,
            "CORS preflight should succeed for route {}",
            route
        );

        let headers = response.headers();
        assert!(
            headers.contains_key("access-control-allow-origin"),
            "Should have CORS allow-origin header for route {}",
            route
        );
    }
}

#[tokio::test]
async fn test_content_type_requirements() {
    let app = create_test_app();

    // 测试需要JSON content-type的路由
    let json_routes = vec![
        "/api/agent/log/add",
        "/api/agent/log/batch",
        "/api/agent/log/search",
        "/api/logs",
        "/api/logs/batch",
    ];

    for route in json_routes {
        // 不提供content-type的请求
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(route)
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // 应该会因为缺少正确的content-type而失败，或者因为其他原因（但不是404）
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Route {} should exist even without content-type",
            route
        );
    }
}

#[tokio::test]
async fn test_empty_body_on_post_routes() {
    let app = create_test_app();

    let post_routes = vec![
        "/api/agent/log/add",
        "/api/agent/log/batch",
        "/api/agent/log/search",
        "/api/logs",
        "/api/logs/batch",
    ];

    for route in post_routes {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(route)
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 空body应该导致反序列化错误，而不是404
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Route {} should exist even with empty body",
            route
        );

        // 通常应该是400 Bad Request或422 Unprocessable Entity
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status() == StatusCode::BAD_GATEWAY, // 可能因为QuickWit连接问题
            "Route {} should return appropriate error for empty body, got {}",
            route,
            response.status()
        );
    }
}
