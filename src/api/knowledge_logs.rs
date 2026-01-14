use axum::response::IntoResponse;
use log::error;
use std::sync::Arc;

use crate::{
    models::{
        AsyncDeleteResult, ClearResult, DeleteParams, DeleteResult, DeleteTask, DeleteTaskStatus,
        HttpResult, KnowledgeSearchParams, KnowledgeSearchResult, KnowledgeStatsResult,
        PushRequest, PushResult, StatsParams, UpdateRequest, UpdateResult,
    },
    my_error::AppError,
    services::KnowledgeQuickwitService,
};

/// 知识库索引创建接口
#[utoipa::path(
    get,
    path = "/api/knowledge/createIndex",
    tag = "知识库",
    responses(
        (status = 200, description = "索引创建成功"),
        (status = 500, description = "索引创建失败")
    )
)]
pub async fn knowledge_create_index(
    app_states: Arc<crate::models::AppStates>,
) -> Result<impl IntoResponse, AppError> {
    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.ensure_knowledge_index_exists().await {
        Ok(_) => Ok(HttpResult::<()>::success()),
        Err(e) => {
            error!("知识库索引检查创建失败: {}", e);
            Ok(HttpResult::<()>::error("500", &e.to_string()))
        }
    }
}

/// 异步删除知识库分段接口（非阻塞）
#[utoipa::path(
    post,
    path = "/api/knowledge/delete-async",
    tag = "知识库",
    request_body = DeleteParams,
    responses(
        (status = 200, description = "异步删除任务创建成功", body = HttpResult<AsyncDeleteResult>),
        (status = 400, description = "参数错误", body = HttpResult<AsyncDeleteResult>),
        (status = 500, description = "删除任务创建失败", body = HttpResult<AsyncDeleteResult>)
    )
)]
pub async fn knowledge_delete_segments_async(
    app_states: Arc<crate::models::AppStates>,
    params: DeleteParams,
) -> Result<impl IntoResponse, AppError> {
    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service
        .delete_knowledge_segments_async(params)
        .await
    {
        Ok(async_result) => Ok(HttpResult::<AsyncDeleteResult>::success_with_data(
            async_result,
        )),
        Err(e) => {
            error!("创建异步删除任务失败: {}", e);
            Ok(HttpResult::error_with_data(
                "5003",
                &format!("创建异步删除任务失败: {}", e),
                AsyncDeleteResult {
                    task_id: String::new(),
                    estimated_delete_count: 0,
                    created_time: String::new(),
                    status: "failed".to_string(),
                    query: String::new(),
                },
            ))
        }
    }
}

/// 查询所有删除任务
#[utoipa::path(
    get,
    path = "/api/knowledge/delete-tasks",
    tag = "知识库",
    responses(
        (status = 200, description = "查询成功", body = HttpResult<Vec<DeleteTask>>),
        (status = 500, description = "查询失败", body = HttpResult<Vec<DeleteTask>>)
    )
)]
pub async fn knowledge_get_delete_tasks(
    app_states: Arc<crate::models::AppStates>,
) -> Result<impl IntoResponse, AppError> {
    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.get_delete_tasks().await {
        Ok(tasks) => Ok(HttpResult::<Vec<DeleteTask>>::success_with_data(tasks)),
        Err(e) => {
            error!("查询删除任务失败: {}", e);
            Ok(HttpResult::<Vec<DeleteTask>>::error_with_data(
                "5002",
                &format!("查询删除任务失败: {}", e),
                vec![],
            ))
        }
    }
}

/// 查询指定删除任务状态
#[utoipa::path(
    get,
    path = "/api/knowledge/delete-tasks/{task_id}",
    tag = "知识库",
    params(
        ("task_id" = String, Path, description = "删除任务ID")
    ),
    responses(
        (status = 200, description = "查询成功", body = HttpResult<DeleteTask>),
        (status = 400, description = "任务ID不能为空", body = HttpResult<DeleteTask>),
        (status = 404, description = "删除任务不存在", body = HttpResult<DeleteTask>),
        (status = 500, description = "查询失败", body = HttpResult<DeleteTask>)
    )
)]
pub async fn knowledge_get_delete_task_status(
    task_id: String,
    app_states: Arc<crate::models::AppStates>,
) -> Result<impl IntoResponse, AppError> {
    if task_id.trim().is_empty() {
        return Ok(HttpResult::error_with_data(
            "4000",
            "任务ID不能为空",
            DeleteTask {
                task_id: String::new(),
                status: String::new(),
                query: String::new(),
                created_at: 0,
                started_at: None,
                ended_at: None,
                num_deleted_docs: None,
                error_message: Some("任务ID不能为空".to_string()),
            },
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.get_delete_task_status(&task_id).await {
        Ok(task) => Ok(HttpResult::<DeleteTask>::success_with_data(task)),
        Err(e) => {
            error!("查询删除任务状态失败: {}", e);
            let error_msg = format!("查询删除任务状态失败: {}", e);

            // 如果错误信息包含任务不存在，返回404错误
            if error_msg.contains("not found") || error_msg.contains("404") {
                Ok(HttpResult::error_with_data(
                    "4004",
                    "删除任务不存在",
                    DeleteTask {
                        task_id: String::new(),
                        status: String::new(),
                        query: String::new(),
                        created_at: 0,
                        started_at: None,
                        ended_at: None,
                        num_deleted_docs: None,
                        error_message: Some("删除任务不存在".to_string()),
                    },
                ))
            } else {
                Ok(HttpResult::error_with_data(
                    "5002",
                    &error_msg,
                    DeleteTask {
                        task_id: String::new(),
                        status: String::new(),
                        query: String::new(),
                        created_at: 0,
                        started_at: None,
                        ended_at: None,
                        num_deleted_docs: None,
                        error_message: Some(error_msg.clone()),
                    },
                ))
            }
        }
    }
}

/// 查询指定删除任务的简化状态
#[utoipa::path(
    get,
    path = "/api/knowledge/delete-tasks/{task_id}/status",
    tag = "知识库",
    params(
        ("task_id" = String, Path, description = "删除任务ID")
    ),
    responses(
        (status = 200, description = "查询成功", body = HttpResult<DeleteTaskStatus>),
        (status = 400, description = "任务ID不能为空", body = HttpResult<DeleteTaskStatus>),
        (status = 404, description = "删除任务不存在", body = HttpResult<DeleteTaskStatus>),
        (status = 500, description = "查询失败", body = HttpResult<DeleteTaskStatus>)
    )
)]
pub async fn knowledge_get_delete_task_simple_status(
    task_id: String,
    app_states: Arc<crate::models::AppStates>,
) -> Result<impl IntoResponse, AppError> {
    if task_id.trim().is_empty() {
        return Ok(HttpResult::error_with_data(
            "4000",
            "任务ID不能为空",
            DeleteTaskStatus {
                task_id: String::new(),
                status: String::new(),
                num_deleted_docs: None,
                error_message: Some("任务ID不能为空".to_string()),
            },
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service
        .get_delete_task_simple_status(&task_id)
        .await
    {
        Ok(status) => Ok(HttpResult::<DeleteTaskStatus>::success_with_data(status)),
        Err(e) => {
            error!("查询删除任务状态失败: {}", e);
            let error_msg = format!("查询删除任务状态失败: {}", e);

            // 如果错误信息包含任务不存在，返回404错误
            if error_msg.contains("not found") || error_msg.contains("404") {
                Ok(HttpResult::error_with_data(
                    "4004",
                    "删除任务不存在",
                    DeleteTaskStatus {
                        task_id: String::new(),
                        status: String::new(),
                        num_deleted_docs: None,
                        error_message: Some("删除任务不存在".to_string()),
                    },
                ))
            } else {
                Ok(HttpResult::error_with_data(
                    "5002",
                    &error_msg,
                    DeleteTaskStatus {
                        task_id: String::new(),
                        status: String::new(),
                        num_deleted_docs: None,
                        error_message: Some(error_msg.clone()),
                    },
                ))
            }
        }
    }
}

/// 知识库搜索接口
#[utoipa::path(
    post,
    path = "/api/knowledge/search",
    tag = "知识库",
    request_body = KnowledgeSearchParams,
    responses(
        (status = 200, description = "搜索成功", body = HttpResult<KnowledgeSearchResult>),
        (status = 400, description = "参数错误", body = HttpResult<KnowledgeSearchResult>),
        (status = 500, description = "搜索失败", body = HttpResult<KnowledgeSearchResult>)
    )
)]
pub async fn knowledge_search_logs(
    app_states: Arc<crate::models::AppStates>,
    params: KnowledgeSearchParams,
) -> Result<HttpResult<KnowledgeSearchResult>, AppError> {
    if params.tenant_id == 0 {
        let empty_result = KnowledgeSearchResult::new(vec![], 0, 0);
        return Ok(HttpResult::error_with_data(
            "400",
            "tenant_id不能为空",
            empty_result,
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.search_knowledge_segments(params).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => {
            let empty_result = KnowledgeSearchResult::new(vec![], 0, 0);
            Ok(HttpResult::error_with_data(
                "500",
                &e.to_string(),
                empty_result,
            ))
        }
    }
}

/// 知识库数据推送接口
#[utoipa::path(
    post,
    path = "/api/knowledge/push",
    tag = "知识库",
    request_body = PushRequest,
    responses(
        (status = 200, description = "数据推送成功", body = HttpResult<PushResult>),
        (status = 400, description = "参数错误", body = HttpResult<PushResult>),
        (status = 500, description = "推送失败", body = HttpResult<PushResult>)
    )
)]
pub async fn knowledge_push_segments(
    app_states: Arc<crate::models::AppStates>,
    request: PushRequest,
) -> Result<HttpResult<PushResult>, AppError> {
    // 验证参数
    if request.segments.is_empty() {
        return Ok(HttpResult::<PushResult>::error_with_data(
            "400",
            "分段数据不能为空",
            PushResult {
                indexed_count: 0,
                push_time: String::new(),
                success_raw_ids: None,
            },
        ));
    }

    // 验证每个分段的 tenant_id（现在是 i64 类型，检查是否为 0）
    for segment in &request.segments {
        if segment.tenant_id == 0 {
            return Ok(HttpResult::<PushResult>::error_with_data(
                "400",
                "分段中的 tenant_id 不能为空或0",
                PushResult {
                    indexed_count: 0,
                    push_time: String::new(),
                    success_raw_ids: None,
                },
            ));
        }
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    if let Err(e) = knowledge_service.ensure_knowledge_index_exists().await {
        return Ok(HttpResult::<PushResult>::error_with_data(
            "500",
            &format!("索引创建失败: {}", e),
            PushResult {
                indexed_count: 0,
                push_time: String::new(),
                success_raw_ids: None,
            },
        ));
    }

    // 幂等处理：检查并删除已存在的数据
    // 使用轻量级查询（max_hits=0）只检查数量，不返回文档内容，避免传输大文本字段
    use std::collections::HashMap;
    let mut tenant_groups: HashMap<i64, Vec<u64>> = HashMap::new();

    for segment in &request.segments {
        tenant_groups
            .entry(segment.tenant_id)
            .or_default()
            .push(segment.raw_id);
    }

    // 检查并删除已存在的数据
    let mut total_deleted_count = 0u64;
    for (tenant_id, raw_ids) in &tenant_groups {
        // 构建查询条件：检查这些 raw_id 是否存在
        // 使用 OR 查询，只检查数量（max_hits=0），不返回文档内容
        let raw_id_filters: Vec<String> =
            raw_ids.iter().map(|id| format!("raw_id:{}", id)).collect();
        let check_query = format!(
            "tenant_id:{} AND ({})",
            tenant_id,
            raw_id_filters.join(" OR ")
        );

        // 轻量级查询：只获取数量，不返回文档（避免传输大文本字段）
        match knowledge_service
            .get_segment_count_by_query(&check_query)
            .await
        {
            Ok(existing_count) => {
                if existing_count > 0 {
                    log::info!(
                        "检测到 {} 条已存在的数据 (tenant_id: {}), 准备删除",
                        existing_count,
                        tenant_id
                    );

                    // 只删除确实存在的数据
                    let delete_params = DeleteParams {
                        kb_id: None,
                        doc_id: None,
                        raw_ids: Some(raw_ids.iter().map(|&x| x as i64).collect()),
                        tenant_id: *tenant_id,
                        space_id: None,
                        start_timestamp: None,
                        end_timestamp: None,
                    };

                    match knowledge_service
                        .delete_knowledge_segments(delete_params)
                        .await
                    {
                        Ok(deleted_count) => {
                            total_deleted_count += deleted_count;
                            log::info!(
                                "幂等处理：已提交删除 {} 条已存在的数据 (tenant_id: {})",
                                deleted_count,
                                tenant_id
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "删除已存在数据失败 (tenant_id: {}): {}, 将继续插入（可能产生重复数据）",
                                tenant_id,
                                e
                            );
                            // 删除失败不影响插入，但会记录警告
                        }
                    }
                } else {
                    log::debug!("未检测到已存在的数据 (tenant_id: {}), 直接插入", tenant_id);
                }
            }
            Err(e) => {
                log::warn!(
                    "检查已存在数据失败 (tenant_id: {}): {}, 将继续插入（可能产生重复数据）",
                    tenant_id,
                    e
                );
                // 查询失败不影响插入，但会记录警告
            }
        }
    }

    // 如果删除了数据，等待一小段时间让删除任务提交
    // 注意：由于删除是异步的，这里只是等待任务提交，不等待执行完成
    if total_deleted_count > 0 {
        log::info!(
            "已提交删除 {} 条数据，等待删除任务提交完成",
            total_deleted_count
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // 插入所有数据
    log::info!("准备推送 {} 条数据", request.segments.len());

    match knowledge_service
        .batch_ingest_knowledge_segments(&request.segments)
        .await
    {
        Ok(_) => {
            let result = PushResult {
                indexed_count: request.segments.len() as u64,
                push_time: chrono::Utc::now().to_rfc3339(),
                success_raw_ids: Some(request.segments.iter().map(|s| s.raw_id).collect()),
            };
            log::info!("知识库数据推送成功: {} 条", result.indexed_count);
            Ok(HttpResult::success_with_data(result))
        }
        Err(e) => {
            log::error!("知识库数据推送失败: {}", e);
            Ok(HttpResult::<PushResult>::error_with_data(
                "500",
                &e.to_string(),
                PushResult {
                    indexed_count: 0,
                    push_time: String::new(),
                    success_raw_ids: None,
                },
            ))
        }
    }
}

/// 知识库数据删除接口
#[utoipa::path(
    post,
    path = "/api/knowledge/delete",
    tag = "知识库",
    request_body = DeleteParams,
    responses(
        (status = 200, description = "删除成功", body = HttpResult<DeleteResult>),
        (status = 400, description = "参数错误", body = HttpResult<DeleteResult>),
        (status = 500, description = "删除失败", body = HttpResult<DeleteResult>)
    )
)]
pub async fn knowledge_delete_segments(
    app_states: Arc<crate::models::AppStates>,
    params: DeleteParams,
) -> Result<HttpResult<DeleteResult>, AppError> {
    if params.tenant_id == 0 {
        return Ok(HttpResult::<DeleteResult>::error_with_data(
            "400",
            "tenant_id不能为空",
            DeleteResult {
                deleted_count: 0,
                delete_time: String::new(),
            },
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.delete_knowledge_segments(params).await {
        Ok(deleted_count) => {
            let result = DeleteResult {
                deleted_count,
                delete_time: chrono::Utc::now().to_rfc3339(),
            };
            Ok(HttpResult::success_with_data(result))
        }
        Err(e) => Ok(HttpResult::<DeleteResult>::error_with_data(
            "500",
            &e.to_string(),
            DeleteResult {
                deleted_count: 0,
                delete_time: String::new(),
            },
        )),
    }
}

/// 知识库全量清空接口
#[utoipa::path(
    post,
    path = "/api/knowledge/clear",
    tag = "知识库",
    responses(
        (status = 200, description = "清空成功", body = HttpResult<ClearResult>),
        (status = 500, description = "清空失败", body = HttpResult<ClearResult>)
    )
)]
pub async fn knowledge_clear_all_segments(
    app_states: Arc<crate::models::AppStates>,
) -> Result<HttpResult<ClearResult>, AppError> {
    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.clear_all_knowledge_segments().await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => Ok(HttpResult::<ClearResult>::error_with_data(
            "500",
            &e.to_string(),
            ClearResult {
                total_count_before: 0,
                deleted_count: 0,
                clear_time: String::new(),
            },
        )),
    }
}

/// 知识库统计接口
#[utoipa::path(
    post,
    path = "/api/knowledge/stats",
    tag = "知识库",
    request_body = StatsParams,
    responses(
        (status = 200, description = "获取成功", body = HttpResult<KnowledgeStatsResult>),
        (status = 400, description = "参数错误", body = HttpResult<KnowledgeStatsResult>),
        (status = 500, description = "获取失败", body = HttpResult<KnowledgeStatsResult>)
    )
)]
pub async fn knowledge_get_stats(
    app_states: Arc<crate::models::AppStates>,
    params: StatsParams,
) -> Result<HttpResult<KnowledgeStatsResult>, AppError> {
    if params.tenant_id == 0 {
        return Ok(HttpResult::<KnowledgeStatsResult>::error_with_data(
            "400",
            "tenant_id不能为空",
            KnowledgeStatsResult {
                tenant_id: 0,
                kb_id: None,
                space_id: None,
                doc_count: 0,
                total_segments: 0,
                doc_stats: vec![],
                stats_time: String::new(),
            },
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.get_knowledge_stats(params).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => {
            let empty_result = KnowledgeStatsResult {
                tenant_id: 0,
                kb_id: None,
                space_id: None,
                doc_count: 0,
                total_segments: 0,
                doc_stats: vec![],
                stats_time: String::new(),
            };
            Ok(HttpResult::error_with_data(
                "500",
                &e.to_string(),
                empty_result,
            ))
        }
    }
}

/// 知识库文本更新接口
#[utoipa::path(
    post,
    path = "/api/knowledge/update",
    tag = "知识库",
    request_body = UpdateRequest,
    responses(
        (status = 200, description = "更新成功", body = HttpResult<UpdateResult>),
        (status = 400, description = "参数错误", body = HttpResult<UpdateResult>),
        (status = 500, description = "更新失败", body = HttpResult<UpdateResult>)
    )
)]
pub async fn knowledge_update_segment(
    app_states: Arc<crate::models::AppStates>,
    request: UpdateRequest,
) -> Result<HttpResult<UpdateResult>, AppError> {
    // 验证必填字段
    if request.tenant_id == 0 {
        return Ok(HttpResult::<UpdateResult>::error_with_data(
            "400",
            "tenant_id 不能为空",
            UpdateResult {
                updated_count: 0,
                update_time: String::new(),
            },
        ));
    }

    if request.raw_id.trim().is_empty() {
        return Ok(HttpResult::<UpdateResult>::error_with_data(
            "400",
            "raw_id 不能为空",
            UpdateResult {
                updated_count: 0,
                update_time: String::new(),
            },
        ));
    }

    if request.raw_txt.trim().is_empty() {
        return Ok(HttpResult::<UpdateResult>::error_with_data(
            "400",
            "raw_txt 不能为空",
            UpdateResult {
                updated_count: 0,
                update_time: String::new(),
            },
        ));
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.update_knowledge_segment(request).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => Ok(HttpResult::<UpdateResult>::error_with_data(
            "500",
            &e.to_string(),
            UpdateResult {
                updated_count: 0,
                update_time: String::new(),
            },
        )),
    }
}

/// 查询分段ID列表接口
#[utoipa::path(
    post,
    path = "/api/knowledge/segment-ids",
    tag = "知识库",
    request_body = crate::models::SegmentIdsParams,
    responses(
        (status = 200, description = "查询成功", body = HttpResult<crate::models::SegmentIdsResult>),
        (status = 400, description = "参数错误", body = HttpResult<crate::models::SegmentIdsResult>),
        (status = 500, description = "查询失败", body = HttpResult<crate::models::SegmentIdsResult>)
    )
)]
pub async fn knowledge_query_segment_ids(
    app_states: Arc<crate::models::AppStates>,
    params: crate::models::SegmentIdsParams,
) -> Result<HttpResult<crate::models::SegmentIdsResult>, AppError> {
    // 验证必填字段
    if params.tenant_id == 0 {
        return Ok(
            HttpResult::<crate::models::SegmentIdsResult>::error_with_data(
                "4000",
                "参数错误: tenant_id 不能为空",
                crate::models::SegmentIdsResult {
                    tenant_id: 0,
                    kb_id: 0,
                    space_id: None,
                    total_count: 0,
                    segment_ids: vec![],
                    query_time: String::new(),
                },
            ),
        );
    }

    if params.kb_id == 0 {
        return Ok(
            HttpResult::<crate::models::SegmentIdsResult>::error_with_data(
                "4000",
                "参数错误: kb_id 不能为空",
                crate::models::SegmentIdsResult {
                    tenant_id: params.tenant_id,
                    kb_id: 0,
                    space_id: params.space_id,
                    total_count: 0,
                    segment_ids: vec![],
                    query_time: String::new(),
                },
            ),
        );
    }

    let knowledge_service = KnowledgeQuickwitService::new(app_states);

    match knowledge_service.query_segment_ids(params).await {
        Ok(result) => Ok(HttpResult::success_with_data(result)),
        Err(e) => {
            let empty_result = crate::models::SegmentIdsResult {
                tenant_id: 0,
                kb_id: 0,
                space_id: None,
                total_count: 0,
                segment_ids: vec![],
                query_time: String::new(),
            };
            Ok(HttpResult::error_with_data(
                "5001",
                &format!("Quickwit 查询失败: {}", e),
                empty_result,
            ))
        }
    }
}
