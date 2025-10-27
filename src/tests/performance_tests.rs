#![cfg(test)]

use crate::tests::test_helpers::TestHelper;
use axum::http::StatusCode;
use std::time::Instant;
use tower::ServiceExt;

#[tokio::test]
async fn test_health_check_performance() {
    let app = TestHelper::create_app();
    let start = Instant::now();

    let response = app
        .oneshot(TestHelper::create_get_request("/health"))
        .await
        .unwrap();

    let duration = start.elapsed();

    assert_eq!(response.status(), StatusCode::OK);
    // 健康检查应该在100ms内完成
    assert!(
        duration.as_millis() < 100,
        "Health check took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_agent_log_search_performance() {
    let app = TestHelper::create_app();
    let search_body =
        TestHelper::create_search_with_sorting_json("tenant_001", "request_start_time", false);

    let start = Instant::now();

    let response = app
        .oneshot(TestHelper::create_post_request(
            "/api/agent/log/search",
            &search_body,
        ))
        .await
        .unwrap();

    let duration = start.elapsed();

    // 测试环境下可能无法连接到QuickWit，但应该能快速响应
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
    // 搜索请求应该在5秒内完成（考虑到可能的网络延迟）
    assert!(
        duration.as_secs() < 5,
        "Search took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_large_page_size_search() {
    let app = TestHelper::create_app();
    let large_page_search = TestHelper::create_performance_search_params(100);

    let start = Instant::now();

    let response = app
        .oneshot(TestHelper::create_post_request(
            "/api/agent/log/search",
            &large_page_search,
        ))
        .await
        .unwrap();

    let duration = start.elapsed();

    // 大分页查询应该正常处理
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
    // 即使是大分页也应该在合理时间内响应
    assert!(
        duration.as_secs() < 10,
        "Large page search took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_batch_ingest_performance() {
    let app = TestHelper::create_app();

    // 创建小批量数据进行性能测试
    let batch_logs = TestHelper::create_batch_agent_logs(10, "perf_test_tenant");
    let batch_body = serde_json::to_string(&batch_logs).unwrap();

    let start = Instant::now();

    let response = app
        .oneshot(TestHelper::create_post_request(
            "/api/agent/log/batch",
            &batch_body,
        ))
        .await
        .unwrap();

    let duration = start.elapsed();

    // 批量插入应该正常处理
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
    // 批量插入应该在合理时间内完成
    assert!(
        duration.as_secs() < 5,
        "Batch ingest took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_concurrent_health_checks() {
    let app = TestHelper::create_app();

    let start = Instant::now();

    // 并发发送多个健康检查请求
    let mut handles = vec![];

    for _ in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            app_clone
                .oneshot(TestHelper::create_get_request("/health"))
                .await
                .unwrap()
        });
        handles.push(handle);
    }

    // 等待所有请求完成
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await);
    }

    let duration = start.elapsed();

    // 所有请求都应该成功
    for result in results {
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // 10个并发请求应该在2秒内完成
    assert!(
        duration.as_secs() < 2,
        "Concurrent requests took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_json_serialization_performance() {
    // 测试大量数据的JSON序列化性能
    let start = Instant::now();

    let large_batch = TestHelper::create_large_batch_logs(1000);
    let _json_string = serde_json::to_string(&large_batch).unwrap();

    let duration = start.elapsed();

    // 1000条记录的序列化应该在1秒内完成
    assert!(
        duration.as_secs() < 1,
        "JSON serialization took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_memory_usage_with_large_requests() {
    let app = TestHelper::create_app();

    // 创建较大的批量数据
    let large_batch = TestHelper::create_batch_agent_logs(50, "memory_test_tenant");
    let large_body = serde_json::to_string(&large_batch).unwrap();

    // 连续发送几个大请求
    for _ in 0..3 {
        let response = app
            .clone()
            .oneshot(TestHelper::create_post_request(
                "/api/agent/log/batch",
                &large_body,
            ))
            .await
            .unwrap();

        // 应该能处理大请求而不出现内存错误
        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY
        );
    }
}

#[tokio::test]
async fn test_error_handling_performance() {
    let app = TestHelper::create_app();

    let start = Instant::now();

    // 测试错误处理的性能 - 无效JSON
    let response = app
        .oneshot(TestHelper::create_post_request(
            "/api/agent/log/add",
            &TestHelper::create_invalid_json_body(),
        ))
        .await
        .unwrap();

    let duration = start.elapsed();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // 错误处理应该很快
    assert!(
        duration.as_millis() < 100,
        "Error handling took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_sorting_performance() {
    let app = TestHelper::create_app();

    // 测试不同排序字段的性能
    let valid_fields = TestHelper::get_valid_sort_fields();

    for field in valid_fields {
        let search_body = TestHelper::create_search_with_sorting_json("tenant_001", field, false);

        let start = Instant::now();

        let response = app
            .clone()
            .oneshot(TestHelper::create_post_request(
                "/api/agent/log/search",
                &search_body,
            ))
            .await
            .unwrap();

        let duration = start.elapsed();

        // 每个排序字段都应该能正常处理
        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY
        );
        assert!(
            duration.as_secs() < 3,
            "Sorting by {} took too long: {:?}",
            field,
            duration
        );
    }
}

#[tokio::test]
async fn test_invalid_sorting_handling_performance() {
    let app = TestHelper::create_app();

    let invalid_sort_body = TestHelper::create_search_with_invalid_sorting_json("tenant_001");

    let start = Instant::now();

    let response = app
        .oneshot(TestHelper::create_post_request(
            "/api/agent/log/search",
            &invalid_sort_body,
        ))
        .await
        .unwrap();

    let duration = start.elapsed();

    // 无效排序字段的过滤应该很快完成
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_GATEWAY);
    assert!(
        duration.as_secs() < 3,
        "Invalid sorting handling took too long: {:?}",
        duration
    );
}
