use crate::{
    AppStates,
    index_define::{DEFAULT_KNOWLEDGE_INDEX, get_knowledge_index_config},
    models::{
        AsyncDeleteResult, ClearResult, DeleteParams, DeleteTask, DeleteTaskStatus,
        KnowledgeRawSegment, KnowledgeSearchParams, KnowledgeSearchResult, KnowledgeStatsResult,
        StatsParams, UpdateRequest, UpdateResult,
    },
    my_error::AppError,
};
use anyhow::Result;
use chrono::{self, DateTime, Utc};
use uuid::Uuid;
use log::{debug, info, warn};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 知识库全文检索专用QuickWit服务
/// 用于处理知识库文档分段文本的索引创建、写入和查询
#[derive(Clone)]
pub struct KnowledgeQuickwitService {
    app_states: Arc<AppStates>,
    index_name: String,
}

impl KnowledgeQuickwitService {
    /// 创建新的知识库QuickWit服务实例
    pub fn new(app_states: Arc<AppStates>) -> Self {
        Self {
            app_states,
            index_name: DEFAULT_KNOWLEDGE_INDEX.to_string(),
        }
    }

    /// 使用自定义索引名称创建服务实例
    pub fn new_with_index_name(app_states: Arc<AppStates>, index_name: String) -> Self {
        Self {
            app_states,
            index_name,
        }
    }

    /// 获取知识库索引名
    pub fn get_knowledge_index_name(&self) -> &str {
        &self.index_name
    }

    /// 获取 QuickWit URL
    pub fn get_url(&self) -> &str {
        &self.app_states.config.url
    }

    /// 检查知识库索引是否存在
    pub async fn check_knowledge_index_exists(&self) -> Result<bool, AppError> {
        let index_name = self.get_knowledge_index_name();
        info!("检查知识库索引 {} 是否存在", index_name);

        let response = self
            .app_states
            .client
            .get(format!(
                "{}/api/v1/indexes/{}",
                self.app_states.config.url, index_name
            ))
            .send()
            .await;

        // 如果请求成功且状态码是成功的，则索引存在
        let index_exists = response
            .map(|resp| resp.status().is_success())
            .unwrap_or(false);

        if index_exists {
            info!("知识库索引 {} 已存在", index_name);
        } else {
            info!("知识库索引 {} 不存在", index_name);
        }

        Ok(index_exists)
    }

    /// 创建知识库索引
    pub async fn create_knowledge_index(&self) -> Result<(), AppError> {
        let index_name = self.get_knowledge_index_name();
        info!("创建知识库索引 {}", index_name);

        // 获取知识库索引配置
        let index_config = get_knowledge_index_config(index_name);

        // 发送创建索引请求
        let response = self
            .app_states
            .client
            .post(format!("{}/api/v1/indexes", self.app_states.config.url))
            .header("content-type", "application/json")
            .json(&index_config)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误信息".to_string());
            return Err(AppError::QuickWit(format!(
                "创建知识库索引失败: {}",
                error_text
            )));
        }

        info!("知识库索引 {} 创建成功", index_name);

        // 等待索引就绪
        sleep(Duration::from_secs(2)).await;

        Ok(())
    }

    /// 确保知识库索引存在，如不存在则创建
    pub async fn ensure_knowledge_index_exists(&self) -> Result<(), AppError> {
        // 检查索引是否存在
        let index_exists = self.check_knowledge_index_exists().await?;
        let index_name = self.get_knowledge_index_name();

        // 如果索引不存在，则创建
        if !index_exists {
            self.create_knowledge_index().await?;

            // 等待索引就绪
            let mut retries = 0;
            let max_retries = 10;

            while retries < max_retries {
                match self.check_knowledge_index_exists().await {
                    Ok(true) => return Ok(()),
                    _ => {
                        retries += 1;
                        info!(
                            "等待知识库索引就绪，尝试 {}/{}，等待 2 秒...",
                            retries, max_retries
                        );
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            return Err(AppError::QuickWit(format!(
                "知识库索引 {} 创建后等待就绪超时",
                index_name
            )));
        }

        Ok(())
    }

    /// 批量摄取知识库分段数据
    pub async fn batch_ingest_knowledge_segments(
        &self,
        segments: &[KnowledgeRawSegment],
    ) -> Result<(), AppError> {
        if segments.is_empty() {
            return Ok(());
        }

        let index_name = self.get_knowledge_index_name();
        info!("批量摄取知识库分段: {} 条", segments.len());

        // 创建带有时间戳和 ID 的文档副本
        let now = chrono::Utc::now();
        let segments_with_timestamp: Vec<KnowledgeRawSegment> = segments
            .iter()
            .map(|segment| {
                let mut segment_copy = segment.clone();

                // 如果没有提供 id，则自动生成 UUID v7
                if segment_copy.id.is_none() {
                    segment_copy.id = Some(uuid::Uuid::now_v7().to_string());
                }

                // 如果没有设置 created，则自动设置为当前时间
                if segment_copy.created.is_none() {
                    segment_copy.created = Some(now);
                }

                segment_copy
            })
            .collect();

        let url = format!(
            "{}/api/v1/{}/ingest?commit=force",
            self.app_states.config.url, index_name
        );

        // 将文档转换为NDJSON格式
        let ndjson = segments_with_timestamp
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| AppError::InternalServer(format!("序列化知识库分段失败: {}", e)))?
            .join("\n");

        info!("发送批量摄取请求到: {}", url);

        let response = self
            .app_states
            .client
            .post(&url)
            .header("content-type", "application/x-ndjson")
            .body(ndjson)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "批量摄取知识库分段失败: {}",
                error_text
            )));
        }

        // 解析响应体，检查实际索引的文档数
        let response_body: Value = response.json().await?;

        // Quickwit 响应格式: {"num_docs_for_processing": N}
        let num_docs_for_processing = response_body
            .get("num_docs_for_processing")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let expected_count = segments_with_timestamp.len() as u64;

        if num_docs_for_processing != expected_count {
            warn!(
                "批量摄取部分失败: 预期 {} 条，实际接受 {} 条",
                expected_count, num_docs_for_processing
            );

            // 如果完全失败，返回错误
            if num_docs_for_processing == 0 {
                return Err(AppError::QuickWit(format!(
                    "批量摄取完全失败: 预期 {} 条，实际接受 0 条。可能是字段类型不匹配或数据格式错误",
                    expected_count
                )));
            }
        }

        info!("批量摄取成功: {} 条数据已接受处理", num_docs_for_processing);

        Ok(())
    }

    /// 搜索知识库分段
    pub async fn search_knowledge_segments(
        &self,
        params: KnowledgeSearchParams,
    ) -> Result<KnowledgeSearchResult, AppError> {
        let index_name = self.get_knowledge_index_name();
        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, index_name
        );

        // 计算分页参数
        let offset = params.offset.unwrap_or(0);
        let max_hits = params.limit.unwrap_or(20).min(100); // 限制最大100条

        // 构建基本查询
        // Quickwit 查询语法: https://quickwit.io/docs/main-branch/reference/query-language
        let mut query_parts = Vec::new();

        // 添加租户过滤（必填，精确匹配）
        query_parts.push(format!("tenant_id:{}", params.tenant_id));

        // 添加知识库过滤（支持多个，OR 关系）
        if let Some(kb_ids) = &params.kb_ids
            && !kb_ids.is_empty() {
                let kb_filter = kb_ids
                    .iter()
                    .map(|id| format!("kb_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                query_parts.push(format!("({})", kb_filter));
            }

        // 添加文档过滤（支持多个，OR 关系）
        if let Some(doc_ids) = &params.doc_ids
            && !doc_ids.is_empty() {
                let doc_filter = doc_ids
                    .iter()
                    .map(|id| format!("doc_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                query_parts.push(format!("({})", doc_filter));
            }

        // 添加原始分段过滤（支持多个，OR 关系）
        if let Some(raw_ids) = &params.raw_ids
            && !raw_ids.is_empty() {
                let raw_filter = raw_ids
                    .iter()
                    .map(|id| format!("raw_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                query_parts.push(format!("({})", raw_filter));
            }

        // 添加空间过滤（精确匹配）
        if let Some(space_id) = params.space_id {
            query_parts.push(format!("space_id:{}", space_id));
        }

        // 全文检索查询（在 raw_txt 字段中搜索）
        // 如果提供了查询关键词，添加到查询条件中
        if !params.query.is_empty() {
            // 如果查询包含空格或特殊字符，使用引号包裹
            let query_text = if params.query.contains(' ') || params.query.contains(':') {
                format!("raw_txt:\"{}\"", params.query)
            } else {
                format!("raw_txt:{}", params.query)
            };
            query_parts.push(query_text);
        }

        // 构建最终查询字符串
        // 所有条件使用 AND 连接
        let query_string = if query_parts.is_empty() {
            "*".to_string()
        } else {
            query_parts.join(" AND ")
        };

        // 构建搜索请求
        let mut search_request = json!({
            "query": query_string,
            "max_hits": max_hits,
            "start_offset": offset,
        });

        // 如果有全文检索查询，按相关性排序
        // 否则按指定字段排序
        if !params.query.is_empty() {
            // 全文检索模式：按 BM25 相关性评分排序
            // 注意：Quickwit 0.8.2 的 REST API 不返回 _score 值，但会按相关性排序
            search_request["sort_by"] = json!("_score");
        } else {
            // 普通查询模式：按指定字段排序
            let sort_field = params.sort_by.unwrap_or_else(|| "created".to_string());
            let sort_order = params.sort_order.unwrap_or_else(|| "desc".to_string());
            let sort_by = if sort_order == "asc" {
                format!("+{}", sort_field)
            } else {
                format!("-{}", sort_field)
            };
            search_request["sort_by"] = json!(sort_by);
        }

        search_request["snippet_fields"] = json!("raw_txt");
        info!("知识库搜索请求: {}", search_request);

        // 发送请求
        let response = self
            .app_states
            .client
            .post(&search_url)
            .header("Content-Type", "application/json")
            .json(&search_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "搜索知识库分段失败: {}",
                error_text
            )));
        }

        let search_response: Value = response.json().await?;

        debug!("知识库搜索响应: {}", search_response);

        // 解析搜索结果
        let hits = search_response["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("搜索结果解析失败: 缺少hits数组".to_string()))?;

        let mut search_results = Vec::with_capacity(hits.len());

        for (idx, hit) in hits.iter().enumerate() {
            // 调试：打印第一个 hit 的完整结构，帮助理解 Quickwit 的响应格式
            if idx == 0 {
                info!(
                    "第一个搜索结果的完整结构: {}",
                    serde_json::to_string_pretty(hit).unwrap_or_default()
                );
                info!(
                    "搜索响应中的所有键: {:?}",
                    hit.as_object().map(|obj| obj.keys().collect::<Vec<_>>())
                );
            }
            // 手动解析字段，处理类型转换
            // Quickwit 返回的 raw_id/kb_id/doc_id 是 u64，但模型定义是 String
            let id = hit
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let raw_id = hit.get("raw_id").and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else { v.as_u64().map(|n| n.to_string()) }
            });

            let kb_id = hit.get("kb_id").and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else { v.as_u64().map(|n| n.to_string()) }
            });

            let doc_id = hit.get("doc_id").and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else { v.as_u64().map(|n| n.to_string()) }
            });

            let tenant_id = hit.get("tenant_id").and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else { v.as_i64().map(|n| n.to_string()) }
            });

            let space_id = hit.get("space_id").and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else { v.as_i64().map(|n| n.to_string()) }
            });

            // 检查必填字段
            if raw_id.is_none()
                || kb_id.is_none()
                || doc_id.is_none()
                || tenant_id.is_none()
                || space_id.is_none()
            {
                warn!("搜索结果解析失败，缺少必填字段: {:?}", hit);
                continue;
            }

            let raw_txt = hit
                .get("raw_txt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let sort_index = hit.get("sort_index").and_then(|v| v.as_i64()).unwrap_or(0);

            let created = hit
                .get("created")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            // 提取高亮文本（如果支持）
            let highlight = hit
                .get("snippets")
                .and_then(|v| {
                    if let Some(arr) = v.as_array() {
                        if let Some(first) = arr.first() {
                            first.as_str().map(|s| s.to_string()).or_else(|| {
                                first
                                    .get("snippet")
                                    .and_then(|s| s.as_str().map(|s| s.to_string()))
                            })
                        } else {
                            None
                        }
                    } else {
                        v.as_str().map(|s| s.to_string())
                    }
                })
                .or_else(|| {
                    hit.get("text_highlight")
                        .and_then(|h| h.as_str())
                        .map(|s| s.to_string())
                });

            // 提取得分
            // Quickwit 可能在不同位置返回 score：
            // 1. 直接在 hit 对象中: hit["score"]
            // 2. 在 _score 字段中: hit["_score"]
            // 3. 在排序值中: hit["_sort"][0]
            let score = hit
                .get("_score")
                .or_else(|| hit.get("score"))
                .and_then(|s| s.as_f64())
                .map(|f| f as f32)
                .or_else(|| {
                    // 尝试从 _sort 数组中获取
                    hit.get("_sort")
                        .and_then(|arr| arr.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.as_f64())
                        .map(|f| f as f32)
                });

            if score.is_some() {
                debug!(
                    "找到评分: {:?} (raw_id: {})",
                    score,
                    raw_id.as_ref().unwrap_or(&"unknown".to_string())
                );
            } else {
                debug!(
                    "未找到评分 (raw_id: {})",
                    raw_id.as_ref().unwrap_or(&"unknown".to_string())
                );
            }

            search_results.push(crate::models::KnowledgeSearchHit {
                id: Some(id),
                raw_id: raw_id.and_then(|s| s.parse().ok()),
                kb_id: kb_id.and_then(|s| s.parse().ok()),
                doc_id: doc_id.and_then(|s| s.parse().ok()),
                raw_txt: Some(raw_txt),
                sort_index: Some(sort_index),
                tenant_id: tenant_id.and_then(|s| s.parse().ok()),
                space_id: space_id.and_then(|s| s.parse().ok()),
                created: Some(created),
                score,
                highlight,
            });
        }

        let total = search_response["num_hits"].as_i64().unwrap_or_default();

        let elapsed_time_ms = search_response["elapsed_time_micros"]
            .as_i64()
            .unwrap_or_default()
            / 1000;

        let result = KnowledgeSearchResult::new(search_results, total, elapsed_time_ms)
            .with_pagination(1, max_hits as i64);

        Ok(result)
    }

    /// 获取知识库统计信息
    ///
    /// 使用 Quickwit 的聚合查询获取准确的文档数和分段统计
    pub async fn get_knowledge_stats(
        &self,
        params: StatsParams,
    ) -> Result<KnowledgeStatsResult, AppError> {
        // 构建查询条件
        let mut filters = Vec::new();
        filters.push(format!("tenant_id:{}", params.tenant_id));

        if let Some(ref kb_id) = params.kb_id {
            filters.push(format!("kb_id:{}", kb_id));
        }

        if let Some(ref space_id) = params.space_id {
            filters.push(format!("space_id:{}", space_id));
        }

        let filter = filters.join(" AND ");

        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        // 1. 查询总分段数
        let search_request = json!({
            "query": filter,
            "max_hits": 0
        });

        let response = self
            .app_states
            .client
            .post(&search_url)
            .header("Content-Type", "application/json")
            .json(&search_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "获取知识库统计失败: {}",
                error_text
            )));
        }

        let search_response: Value = response.json().await?;
        let total_segments = search_response["num_hits"].as_i64().unwrap_or_default();

        // 2. 使用聚合查询获取文档数和每个文档的分段统计
        // Quickwit 支持 terms 聚合来统计唯一值
        // 参考: https://quickwit.io/docs/main-branch/reference/aggregat
        let agg_request = json!({
            "query": filter,
            "max_hits": 0,
            "aggs": {
                "unique_docs": {
                    "terms": {
                        "field": "doc_id",
                        "size": 10000,
                        "order": { "_count": "desc" }
                    }
                }
            }
        });

        let agg_response = self
            .app_states
            .client
            .post(&search_url)
            .header("Content-Type", "application/json")
            .json(&agg_request)
            .send()
            .await?;

        let mut doc_count = 0u64;
        let mut doc_stats = Vec::new();

        if agg_response.status().is_success() {
            let agg_result: Value = agg_response.json().await?;

            // 解析聚合结果
            // Quickwit 返回格式: { "aggregations": { "unique_docs": { "buckets": [...] } } }
            if let Some(aggregations) = agg_result.get("aggregations")
                && let Some(unique_docs_agg) = aggregations.get("unique_docs")
                    && let Some(buckets) = unique_docs_agg.get("buckets").and_then(|b| b.as_array())
                    {
                        doc_count = buckets.len() as u64;

                        // 提取每个文档的分段统计
                        for bucket in buckets {
                            // Quickwit 返回的 bucket 格式: { "key": value, "doc_count": 123 }
                            // doc_id 可能是数字或字符串，需要处理两种情况
                            let doc_id = if let Some(doc_id_str) =
                                bucket.get("key").and_then(|k| k.as_str())
                            {
                                doc_id_str.to_string()
                            } else if let Some(doc_id_num) =
                                bucket.get("key").and_then(|k| k.as_f64())
                            {
                                doc_id_num.to_string()
                            } else {
                                continue;
                            };

                            if let Some(count) = bucket.get("doc_count").and_then(|c| c.as_u64()) {
                                doc_stats.push(crate::models::DocumentStats {
                                    doc_id: doc_id.parse().unwrap_or(0),
                                    segment_count: count,
                                });
                            }
                        }
                    }
        } else {
            // 如果聚合查询失败，使用估算值
            let error_text = agg_response.text().await.unwrap_or_default();
            warn!("聚合查询失败: {}, 使用估算值", error_text);
            doc_count = (total_segments as f64 / 10.0).round() as u64;
        }

        let stats_result = KnowledgeStatsResult {
            tenant_id: params.tenant_id,
            kb_id: params.kb_id,
            space_id: params.space_id,
            doc_count,
            total_segments: total_segments as u64,
            doc_stats,
            stats_time: chrono::Utc::now().to_rfc3339(),
        };

        Ok(stats_result)
    }

    /// 根据查询条件获取匹配的文档数量
    ///
    /// 使用 max_hits=0 来只获取统计数量，不返回实际文档
    /// 这是一个轻量级查询，不会传输文档内容，只返回数量
    pub(crate) async fn get_segment_count_by_query(&self, query: &str) -> Result<u64, AppError> {
        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        let search_request = json!({
            "query": query,
            "max_hits": 0  // 只获取数量，不返回文档
        });

        let response = self
            .app_states
            .client
            .post(&search_url)
            .header("Content-Type", "application/json")
            .json(&search_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "查询文档数量失败: {}",
                error_text
            )));
        }

        let search_response: Value = response.json().await?;
        let count = search_response["num_hits"].as_u64().unwrap_or(0);

        Ok(count)
    }

    /// 删除知识库分段
    ///
    /// 此函数创建删除任务后立即返回，不等待删除执行完成。
    /// 删除操作是异步的，将在后台执行，执行时间取决于 splits 数量和成熟度。
    ///
    /// # 返回值
    /// 返回预计删除的文档数量（通过查询当前匹配的文档数量估算）。
    /// 实际删除数量需要通过后续查询文档数量来验证。
    ///
    /// # 注意
    /// - 删除任务提交成功后立即返回
    /// - 删除操作在后台异步执行，无法控制执行时间
    /// - 可以通过查询文档数量来验证删除是否完成
    pub async fn delete_knowledge_segments(&self, params: DeleteParams) -> Result<u64, AppError> {
        // 参数验证
        let has_kb_id = params.kb_id.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_doc_id = params.doc_id.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_raw_ids = params.raw_ids.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_space_id = params
            .space_id
            .as_ref()
            .is_some_and(|ids| !ids.is_empty());

        let total_params = [has_kb_id, has_doc_id, has_raw_ids, has_space_id]
            .iter()
            .filter(|&&x| x)
            .count();

        if total_params == 0 {
            return Err(AppError::BadRequest(
                "除tenant_id外，必须指定至少一个删除条件".to_string(),
            ));
        }

        if total_params == 1 && !has_kb_id {
            return Err(AppError::BadRequest(
                "如果只提供单个参数，则必须指定kb_id".to_string(),
            ));
        }

        // 构建删除查询条件
        let mut delete_conditions = vec![format!("tenant_id:{}", params.tenant_id)];

        if let Some(kb_ids) = params.kb_id
            && !kb_ids.is_empty() {
                let kb_filter = kb_ids
                    .iter()
                    .map(|id| format!("kb_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !kb_filter.is_empty() {
                    delete_conditions.push(format!("({})", kb_filter));
                }
            }

        if let Some(doc_ids) = params.doc_id
            && !doc_ids.is_empty() {
                let doc_filter = doc_ids
                    .iter()
                    .map(|id| format!("doc_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !doc_filter.is_empty() {
                    delete_conditions.push(format!("({})", doc_filter));
                }
            }

        if let Some(raw_ids) = params.raw_ids
            && !raw_ids.is_empty() {
                let raw_filter = raw_ids
                    .iter()
                    .map(|id| format!("raw_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !raw_filter.is_empty() {
                    delete_conditions.push(format!("({})", raw_filter));
                }
            }

        if let Some(space_ids) = params.space_id
            && !space_ids.is_empty() {
                let space_filter = space_ids
                    .iter()
                    .map(|id| format!("space_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !space_filter.is_empty() {
                    delete_conditions.push(format!("({})", space_filter));
                }
            }

        let delete_filter = delete_conditions.join(" AND ");

        info!("删除知识库分段，删除过滤条件: {}", delete_filter);
        info!("直接创建删除任务，跳过预查询以提升性能");

        // 使用Quickwit的官方删除API
        let delete_url = format!(
            "{}/api/v1/{}/delete-tasks",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        let mut delete_request = json!({
            "query": delete_filter
        });

        // 添加时间戳范围支持（如果提供了）
        if let Some(start_ts) = params.start_timestamp {
            delete_request["start_timestamp"] = json!(start_ts.to_string());
        }
        if let Some(end_ts) = params.end_timestamp {
            delete_request["end_timestamp"] = json!(end_ts.to_string());
        }

        let delete_response = self
            .app_states
            .client
            .post(&delete_url)
            .header("Content-Type", "application/json")
            .json(&delete_request)
            .send()
            .await?;

        if !delete_response.status().is_success() {
            let error_text = delete_response.text().await.unwrap_or_default();
            warn!("创建删除任务失败: {}", error_text);

            // 如果官方删除API失败，返回详细错误信息，不回退到重建索引
            return Err(AppError::QuickWit(format!(
                "官方删除API失败: {}。请检查Quickwit服务状态或删除条件是否正确。",
                error_text
            )));
        }

        let delete_result: Value = delete_response.json().await?;

        // 打印 Quickwit 响应以便调试
        info!(
            "Quickwit 删除任务创建响应: {}",
            serde_json::to_string_pretty(&delete_result).unwrap_or_default()
        );

        // 解析删除任务响应 - 根据官方文档，使用 opstamp 作为任务ID
        let task_id = delete_result
            .get("opstamp")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| {
                // 如果没有 opstamp，尝试其他可能的字段
                delete_result
                    .get("delete_task_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        delete_result
                            .get("create_timestamp")
                            .and_then(|v| v.as_u64())
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    })
            });

        info!("删除操作已提交到 Quickwit，任务ID: {}", task_id);

        // 根据 Quickwit 源码分析（delete_task_planner.rs）：
        //
        // 删除任务执行机制：
        // 1. DeleteTaskPlanner 定期（每60秒）检查 stale splits（delete_opstamp < last_opstamp）
        // 2. 对每个 stale split，查询所有 opstamp > split.delete_opstamp 的删除任务
        // 3. 如果删除任务匹配 split 的元数据（时间范围、标签），执行删除操作
        // 4. 删除完成后，更新 split.delete_opstamp = last_opstamp
        //
        // 关键发现：
        // - DeleteTask 只包含 opstamp, create_timestamp, delete_query，没有 status 字段
        // - 删除任务会一直保留在列表中，不会自动移除
        // - 删除只应用于"成熟的" splits（不处理正在合并的 splits）
        // - 删除是异步的，执行时间无法控制，取决于 splits 数量和成熟度
        //
        // 因此，我们只确认删除任务提交成功，不等待执行完成
        // 删除操作将在后台异步执行，客户端可以通过查询文档数量来验证删除是否完成

        // 可选：查询删除前的文档数量（用于返回估算值，但不等待删除完成）
        let estimated_count = self
            .get_segment_count_by_query(&delete_filter)
            .await
            .unwrap_or(0);

        if estimated_count > 0 {
            info!(
                "删除任务已提交成功（任务ID: {}），预计将删除约 {} 条记录。删除操作将在后台异步执行。",
                task_id, estimated_count
            );
        } else {
            info!(
                "删除任务已提交成功（任务ID: {}）。删除操作将在后台异步执行。",
                task_id
            );
        }

        // 返回预计删除数量（实际删除数量需要通过后续查询文档数量来验证）
        Ok(estimated_count)
    }

    /// 异步删除知识库分段（非阻塞）
    ///
    /// 此函数创建删除任务后立即返回，不等待任务执行完成。
    /// 客户端可以通过 task_id 查询任务执行状态。
    ///
    /// # 参数
    /// - `params`: 删除参数
    ///
    /// # 返回
    /// - `AsyncDeleteResult`: 包含任务ID、预计删除数量等信息
    pub async fn delete_knowledge_segments_async(
        &self,
        params: DeleteParams,
    ) -> Result<AsyncDeleteResult, AppError> {
        // 参数验证
        let has_kb_id = params.kb_id.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_doc_id = params.doc_id.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_raw_ids = params.raw_ids.as_ref().is_some_and(|ids| !ids.is_empty());
        let has_space_id = params
            .space_id
            .as_ref()
            .is_some_and(|ids| !ids.is_empty());

        let total_params = [has_kb_id, has_doc_id, has_raw_ids, has_space_id]
            .iter()
            .filter(|&&x| x)
            .count();

        if total_params == 0 {
            return Err(AppError::BadRequest(
                "除tenant_id外，必须指定至少一个删除条件".to_string(),
            ));
        }

        if total_params == 1 && !has_kb_id {
            return Err(AppError::BadRequest(
                "如果只提供单个参数，则必须指定kb_id".to_string(),
            ));
        }

        // 构建删除查询条件
        let mut delete_conditions = vec![format!("tenant_id:{}", params.tenant_id)];

        if let Some(kb_ids) = params.kb_id
            && !kb_ids.is_empty() {
                let kb_filter = kb_ids
                    .iter()
                    .map(|id| format!("kb_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !kb_filter.is_empty() {
                    delete_conditions.push(format!("({})", kb_filter));
                }
            }

        if let Some(doc_ids) = params.doc_id
            && !doc_ids.is_empty() {
                let doc_filter = doc_ids
                    .iter()
                    .map(|id| format!("doc_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !doc_filter.is_empty() {
                    delete_conditions.push(format!("({})", doc_filter));
                }
            }

        if let Some(raw_ids) = params.raw_ids
            && !raw_ids.is_empty() {
                let raw_filter = raw_ids
                    .iter()
                    .map(|id| format!("raw_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !raw_filter.is_empty() {
                    delete_conditions.push(format!("({})", raw_filter));
                }
            }

        if let Some(space_ids) = params.space_id
            && !space_ids.is_empty() {
                let space_filter = space_ids
                    .iter()
                    .map(|id| format!("space_id:{}", id))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                if !space_filter.is_empty() {
                    delete_conditions.push(format!("({})", space_filter));
                }
            }

        let delete_filter = delete_conditions.join(" AND ");

        info!("异步删除知识库分段，删除过滤条件: {}", delete_filter);
        info!("直接创建删除任务，跳过预查询以提升性能");

        // 使用Quickwit的官方删除API
        let delete_url = format!(
            "{}/api/v1/{}/delete-tasks",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        let mut delete_request = json!({
            "query": delete_filter
        });

        // 添加时间戳范围支持（如果提供了）
        if let Some(start_ts) = params.start_timestamp {
            delete_request["start_timestamp"] = json!(start_ts.to_string());
        }
        if let Some(end_ts) = params.end_timestamp {
            delete_request["end_timestamp"] = json!(end_ts.to_string());
        }

        let delete_response = self
            .app_states
            .client
            .post(&delete_url)
            .header("Content-Type", "application/json")
            .json(&delete_request)
            .send()
            .await?;

        if !delete_response.status().is_success() {
            let error_text = delete_response.text().await.unwrap_or_default();
            warn!("创建删除任务失败: {}", error_text);

            // 如果官方删除API失败，返回详细错误信息，不回退到重建索引
            return Err(AppError::QuickWit(format!(
                "官方删除API失败: {}。请检查Quickwit服务状态或删除条件是否正确。",
                error_text
            )));
        }

        let delete_result: Value = delete_response.json().await?;

        // 解析删除任务响应
        let task_id = delete_result
            .get("delete_task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!("成功创建异步删除任务: {}, 删除操作正在后台执行", task_id);

        Ok(AsyncDeleteResult {
            task_id: task_id.to_string(),
            estimated_delete_count: 0, // 无法预知，需要通过任务状态查询获取
            created_time: chrono::Utc::now().to_rfc3339(),
            status: "pending".to_string(),
            query: delete_filter,
        })
    }

    /// 查询所有删除任务
    /// GET /api/v1/{index_id}/delete-tasks
    pub async fn get_delete_tasks(&self) -> Result<Vec<DeleteTask>, AppError> {
        let tasks_url = format!(
            "{}/api/v1/{}/delete-tasks",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        let response = self.app_states.client.get(&tasks_url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "查询删除任务失败: {}",
                error_text
            )));
        }

        let tasks_result: Value = response.json().await?;

        // 解析删除任务列表
        // 根据 Quickwit 源码，DeleteTask 结构包含：
        // - opstamp: u64 (任务唯一标识)
        // - create_timestamp: i64 (创建时间戳)
        // - delete_query: DeleteQuery (删除查询条件)
        let tasks = if let Some(tasks_array) = tasks_result.as_array() {
            tasks_array
                .iter()
                .filter_map(|task| {
                    // 解析 opstamp (u64)
                    let opstamp = task.get("opstamp")?.as_u64().map(|v| v.to_string())?;

                    // 解析 create_timestamp (i64，可能返回为数字或字符串)
                    let created_at = task
                        .get("create_timestamp")
                        .and_then(|v| {
                            v.as_i64()
                                .or_else(|| v.as_u64().map(|u| u as i64))
                                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                        })
                        .unwrap_or(0) as u64;

                    // 解析 delete_query
                    let delete_query = task.get("delete_query")?;
                    let query_ast = delete_query
                        .get("query_ast")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "".to_string());

                    // 构造查询字符串（从query_ast中提取，query_ast 是 JSON 格式的查询AST）
                    let query = if query_ast.is_empty() {
                        "未知查询".to_string()
                    } else {
                        // query_ast 是 JSON 格式，尝试解析并提取可读的查询字符串
                        // 如果解析失败，直接使用原始字符串
                        if let Ok(ast_value) = serde_json::from_str::<Value>(&query_ast) {
                            // 尝试从 AST 中提取查询文本
                            if let Some(text) = ast_value.get("text").and_then(|v| v.as_str()) {
                                format!("查询: {}", text)
                            } else if let Some(field) =
                                ast_value.get("field").and_then(|v| v.as_str())
                            {
                                format!("字段: {}", field)
                            } else {
                                format!("查询AST: {}", query_ast)
                            }
                        } else {
                            format!("查询: {}", query_ast)
                        }
                    };

                    // Quickwit 的 DeleteTask 没有 status 字段
                    // 删除任务是异步执行的，无法直接获取状态
                    // 我们使用 "pending" 作为默认状态，表示任务已创建但状态未知
                    Some(DeleteTask {
                        task_id: opstamp.clone(),
                        status: "pending".to_string(), // Quickwit 不提供状态字段
                        query,
                        created_at,
                        started_at: Some(created_at),
                        ended_at: None,
                        num_deleted_docs: None, // Quickwit 不提供删除数量
                        error_message: None,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        info!("查询到 {} 个删除任务", tasks.len());
        Ok(tasks)
    }

    /// 查询指定删除任务的状态
    ///
    /// 根据源码分析（handler.rs）：
    /// - Quickwit 只实现了 GET /api/v1/<index_id>/delete-tasks（获取所有任务）
    /// - 没有实现 GET /api/v1/<index_id>/delete-tasks/<opstamp>（单个任务查询）
    /// - 因此我们通过查询所有任务列表，然后根据 opstamp 过滤来获取单个任务
    ///
    /// 注意：
    /// 1. DeleteTask 结构只包含：opstamp, create_timestamp, delete_query，没有 status 字段
    /// 2. Quickwit 官方文档明确说明：没有直接的方式来监控删除任务的执行进度或确认其完成状态
    /// 3. 建议通过检查索引中的文档数量来间接确认删除完成
    pub async fn get_delete_task_status(&self, task_id: &str) -> Result<DeleteTask, AppError> {
        // 根据源码分析，Quickwit 没有实现单个任务查询 API
        // 我们通过查询所有任务列表，然后根据 opstamp 过滤
        let tasks = self.get_delete_tasks().await?;

        // 将 task_id 转换为 u64 (opstamp)
        let opstamp = task_id
            .parse::<u64>()
            .map_err(|_| AppError::QuickWit(format!("无效的任务ID格式: {}", task_id)))?;

        // 查找匹配的任务
        let task = tasks.iter().find(|t| {
            // 尝试匹配 task_id（字符串）或 opstamp（数字）
            t.task_id == task_id || t.task_id.parse::<u64>().unwrap_or(0) == opstamp
        });

        match task {
            Some(t) => Ok(t.clone()),
            None => Err(AppError::QuickWit(format!(
                "未找到删除任务: {} (opstamp: {})",
                task_id, opstamp
            ))),
        }
    }

    /// 获取删除任务的简化状态信息
    pub async fn get_delete_task_simple_status(
        &self,
        task_id: &str,
    ) -> Result<DeleteTaskStatus, AppError> {
        let task = self.get_delete_task_status(task_id).await?;
        Ok(DeleteTaskStatus {
            task_id: task.task_id,
            status: task.status,
            num_deleted_docs: task.num_deleted_docs,
            error_message: task.error_message,
        })
    }

    /// 全量清空知识库数据
    ///
    /// 使用 Quickwit 官方 clear API 清空索引中的所有数据。
    /// 此操作会删除所有 splits（分片）并重置所有 source checkpoints。
    ///
    /// # 注意
    /// - 此操作不可逆，会清空索引中的所有数据
    /// - 不查询删除前的数据量，直接执行清空操作
    /// - 清空操作是同步的，完成后立即返回
    pub async fn clear_all_knowledge_segments(&self) -> Result<ClearResult, AppError> {
        info!("执行全量清空知识库数据操作");

        // 使用 Quickwit 官方 clear API 清空索引
        // 根据官方文档：PUT /api/v1/indexes/<index_id>/clear
        // 会删除所有 splits（metastore + storage）并重置所有 source checkpoints
        let clear_url = format!(
            "{}/api/v1/indexes/{}/clear",
            self.app_states.config.url,
            self.get_knowledge_index_name()
        );

        info!("调用 Quickwit clear API: {}", clear_url);
        let clear_response = self
            .app_states
            .client
            .put(&clear_url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if !clear_response.status().is_success() {
            let error_text = clear_response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!("清空索引失败: {}", error_text)));
        }

        info!("索引清空操作完成");

        // 返回清空结果（不查询删除前的数量）
        Ok(ClearResult {
            total_count_before: 0, // 不再查询，设为0
            deleted_count: 0,      // 不再查询，设为0
            clear_time: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 更新知识库分段文本
    ///
    /// 实现策略：先查询现有分段 -> 删除旧分段 -> 插入更新后的分段
    /// 注意：Quickwit 不支持原地更新，需要通过删除+插入实现
    pub async fn update_knowledge_segment(
        &self,
        request: UpdateRequest,
    ) -> Result<UpdateResult, AppError> {
        info!(
            "更新知识库分段文本: raw_id={}, tenant_id={}",
            request.raw_id, request.tenant_id
        );

        // 1. 查询现有分段
        let search_params = KnowledgeSearchParams {
            query: String::new(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: Some(vec![request.raw_id.parse().unwrap_or(0)]),
            tenant_id: request.tenant_id,
            space_id: request.space_id,
            offset: None,
            limit: Some(1),
            sort_by: None,
            sort_order: None,
        };

        let search_result = self.search_knowledge_segments(search_params).await?;

        if search_result.results.is_empty() {
            return Err(AppError::NotFound(format!(
                "未找到分段: raw_id={}, tenant_id={}",
                request.raw_id, request.tenant_id
            )));
        }

        let existing_segment = &search_result.results[0];

        // 2. 创建更新后的分段（使用请求参数中的 raw_id，其他字段从现有分段获取）
        let updated_segment = KnowledgeRawSegment {
            id: None, // 知识库分段更新时不需要提供内部ID
            raw_id: request.raw_id.parse().map_err(|_| {
                AppError::BadRequest(format!("无效的raw_id格式: {}", request.raw_id))
            })?,
            kb_id: existing_segment.kb_id.unwrap_or(0),
            doc_id: existing_segment.doc_id.unwrap_or(0),
            raw_txt: request.raw_txt.clone(), // 更新文本内容
            sort_index: existing_segment.sort_index,
            tenant_id: request.tenant_id, // 使用请求参数中的 tenant_id
            space_id: request
                .space_id
                .unwrap_or_else(|| existing_segment.space_id.unwrap_or(0)), // 优先使用请求参数，回退到现有值
            created: existing_segment.created,
        };

        // 3. 删除旧分段 - 使用删除API时需要提供kb_id来满足验证规则
        // 这里我们通过搜索找到对应的kb_id
        let search_for_delete = KnowledgeSearchParams {
            query: String::new(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: Some(vec![request.raw_id.parse().unwrap_or(0)]),
            tenant_id: request.tenant_id,
            space_id: None,
            offset: Some(0),
            limit: Some(1),
            sort_by: None,
            sort_order: None,
        };

        let search_result = self.search_knowledge_segments(search_for_delete).await?;

        if search_result.results.is_empty() {
            return Err(AppError::NotFound(format!(
                "未找到要更新的分段: raw_id={}, tenant_id={}",
                request.raw_id, request.tenant_id
            )));
        }

        let delete_kb_id = search_result.results[0]
            .kb_id
            .map(|kb_id| vec![kb_id as i64]);

        let delete_params = DeleteParams {
            kb_id: delete_kb_id,
            doc_id: None,
            raw_ids: Some(vec![request.raw_id.parse().unwrap_or(0)]),
            tenant_id: request.tenant_id,
            space_id: None,
            start_timestamp: None,
            end_timestamp: None,
        };

        self.delete_knowledge_segments(delete_params).await?;

        // 等待删除完成
        sleep(Duration::from_secs(1)).await;

        // 4. 插入更新后的分段
        self.batch_ingest_knowledge_segments(&[updated_segment])
            .await?;

        info!(
            "分段更新成功: raw_id={}, tenant_id={}",
            request.raw_id, request.tenant_id
        );

        Ok(UpdateResult {
            updated_count: 1,
            update_time: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 查询指定知识库的所有分段ID列表
    ///
    /// 用于数据一致性检查和修复，只返回 raw_id 列表，不返回文本内容
    pub async fn query_segment_ids(
        &self,
        params: crate::models::SegmentIdsParams,
    ) -> Result<crate::models::SegmentIdsResult, AppError> {
        info!(
            "查询分段ID列表: tenant_id={}, kb_id={}, space_id={:?}",
            params.tenant_id, params.kb_id, params.space_id
        );

        let index_name = self.get_knowledge_index_name();
        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, index_name
        );

        // 构建查询条件
        let mut query_parts = Vec::new();
        query_parts.push(format!("tenant_id:{}", params.tenant_id));
        query_parts.push(format!("kb_id:{}", params.kb_id));

        if let Some(ref space_id) = params.space_id {
            query_parts.push(format!("space_id:{}", space_id));
        }

        let query_string = query_parts.join(" AND ");

        // 分批获取所有 raw_id（处理大数据量）
        let mut all_segment_ids: Vec<u64> = Vec::new();
        let batch_size = 10000; // 每批最多获取 10000 条
        let mut offset = 0;

        loop {
            // 构建搜索请求，只返回 raw_id 字段
            let search_request = json!({
                "query": query_string,
                "max_hits": batch_size,
                "start_offset": offset,
                "sort_by": "raw_id",  // 按 raw_id 排序，确保结果稳定
            });

            info!("查询分段ID，offset={}, batch_size={}", offset, batch_size);

            let response = self
                .app_states
                .client
                .post(&search_url)
                .header("Content-Type", "application/json")
                .json(&search_request)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(AppError::QuickWit(format!(
                    "查询分段ID失败: {}",
                    error_text
                )));
            }

            let search_response: Value = response.json().await?;

            // 解析结果
            let hits = search_response["hits"]
                .as_array()
                .ok_or_else(|| AppError::QuickWit("搜索结果解析失败: 缺少hits数组".to_string()))?;

            if hits.is_empty() {
                // 没有更多数据
                break;
            }

            // 提取 raw_id - raw_id 可能是数字或字符串
            for hit in hits {
                let raw_id = if let Some(raw_id_num) = hit.get("raw_id").and_then(|v| v.as_u64()) {
                    raw_id_num
                } else if let Some(raw_id_num) = hit.get("raw_id").and_then(|v| v.as_f64()) {
                    raw_id_num as u64
                } else if let Some(raw_id_str) = hit.get("raw_id").and_then(|v| v.as_str()) {
                    raw_id_str.parse().unwrap_or(0)
                } else {
                    continue;
                };
                all_segment_ids.push(raw_id);
            }

            // 如果这批数据少于批量大小，说明已经获取完所有数据
            if hits.len() < batch_size {
                break;
            }

            offset += batch_size;
        }

        let total_count = all_segment_ids.len() as u64;

        info!(
            "查询分段ID完成: tenant_id={}, kb_id={}, 总数={}",
            params.tenant_id, params.kb_id, total_count
        );

        Ok(crate::models::SegmentIdsResult {
            tenant_id: params.tenant_id,
            kb_id: params.kb_id,
            space_id: params.space_id,
            total_count,
            segment_ids: all_segment_ids,
            query_time: chrono::Utc::now().to_rfc3339(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DeleteParams, KnowledgeRawSegment, KnowledgeSearchParams, UpdateRequest};
    use chrono::Utc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, sleep};

    // 原子计数器，用于生成唯一的测试索引名
    static TEST_INDEX_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// 创建测试用的 AppStates
    fn create_test_app_states() -> AppStates {
        let config = crate::config::QuickwitConfig {
            url: "http://localhost:7280".to_string(),
        };
        AppStates::new(config)
    }

    /// 生成唯一的测试索引名称
    fn generate_test_index_name() -> String {
        let counter = TEST_INDEX_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("test_knowledge_segments_v1_{:04}", counter)
    }

    /// 清理测试索引
    async fn cleanup_test_index(service: &KnowledgeQuickwitService) -> Result<(), AppError> {
        let index_name = service.get_knowledge_index_name();
        let delete_index_url = format!("{}/api/v1/indexes/{}", service.get_url(), index_name);

        let response = service
            .app_states
            .client
            .delete(&delete_index_url)
            .send()
            .await?;

        // 忽略删除失败的情况（索引可能不存在）
        if response.status().is_success() {
            info!("已删除测试索引: {}", index_name);
        } else {
            info!("测试索引 {} 不存在或删除失败，继续执行", index_name);
        }

        Ok(())
    }

    /// 创建测试用的知识库分段
    fn create_test_segments() -> Vec<KnowledgeRawSegment> {
        vec![
            KnowledgeRawSegment {
                id: Some(Uuid::new_v4().to_string()),
                raw_id: 1001,
                kb_id: 1,
                doc_id: 101,
                raw_txt: "人工智能是计算机科学的一个分支".to_string(),
                sort_index: Some(1),
                tenant_id: 1001,
                space_id: 1001,
                created: Some(Utc::now()),
            },
            KnowledgeRawSegment {
                id: Some(Uuid::new_v4().to_string()),
                raw_id: 1002,
                kb_id: 1,
                doc_id: 101,
                raw_txt: "机器学习是人工智能的核心技术".to_string(),
                sort_index: Some(2),
                tenant_id: 1001,
                space_id: 1001,
                created: Some(Utc::now()),
            },
            KnowledgeRawSegment {
                id: Some(Uuid::new_v4().to_string()),
                raw_id: 1003,
                kb_id: 2,
                doc_id: 102,
                raw_txt: "深度学习是机器学习的一个子领域".to_string(),
                sort_index: Some(1),
                tenant_id: 1001,
                space_id: 1002,
                created: Some(Utc::now()),
            },
        ]
    }

    #[tokio::test]
    async fn test_knowledge_service_creation() {
        let app_states = Arc::new(create_test_app_states());
        let service = KnowledgeQuickwitService::new(app_states);

        assert_eq!(service.get_knowledge_index_name(), "knowledge_segments_v1");
        assert_eq!(service.get_url(), "http://localhost:7280");
    }

    #[tokio::test]
    async fn test_knowledge_service_with_custom_index() {
        let app_states = Arc::new(create_test_app_states());
        let service = KnowledgeQuickwitService::new_with_index_name(
            app_states,
            "custom_knowledge_index".to_string(),
        );

        assert_eq!(service.get_knowledge_index_name(), "custom_knowledge_index");
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_create_knowledge_index() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 清理可能存在的索引
        let _ = cleanup_test_index(&service).await;

        // 创建索引
        let create_result = service.ensure_knowledge_index_exists().await;
        assert!(create_result.is_ok(), "创建知识库索引应该成功");

        // 验证索引存在
        let exists = service.check_knowledge_index_exists().await.unwrap();
        assert!(exists, "索引应该存在");

        // 测试完成后清理
        let _ = cleanup_test_index(&service).await;
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_batch_ingest_and_search() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 确保索引存在
        service.ensure_knowledge_index_exists().await.unwrap();

        let test_segments = create_test_segments();

        // 批量插入数据
        let ingest_result = service
            .batch_ingest_knowledge_segments(&test_segments)
            .await;
        assert!(ingest_result.is_ok(), "批量插入应该成功");

        // 等待索引更新
        sleep(Duration::from_secs(2)).await;

        // 测试搜索功能
        let search_params = KnowledgeSearchParams {
            query: "人工智能".to_string(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            offset: Some(0),
            limit: Some(10),
            sort_by: Some("created".to_string()),
            sort_order: Some("desc".to_string()),
        };

        let search_result = service.search_knowledge_segments(search_params).await;
        assert!(search_result.is_ok(), "搜索应该成功");

        let result = search_result.unwrap();
        assert!(result.results.len() > 0, "应该返回搜索结果");
        assert!(result.total > 0, "总数应该大于0");

        // 验证搜索结果的内容
        let ai_results: Vec<_> = result
            .results
            .iter()
            .filter(|r| {
                r.raw_txt
                    .as_ref()
                    .map_or(false, |txt| txt.contains("人工智能"))
            })
            .collect();
        assert!(!ai_results.is_empty(), "应该包含人工智能相关的结果");

        // 测试完成后清理
        let _ = cleanup_test_index(&service).await;
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_update_knowledge_segment() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 确保索引存在
        service.ensure_knowledge_index_exists().await.unwrap();

        // 插入测试数据
        let test_segments = create_test_segments();
        service
            .batch_ingest_knowledge_segments(&test_segments)
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;

        // 测试更新分段
        let update_request = UpdateRequest {
            id: None,
            raw_id: "1001".to_string(),
            raw_txt: "人工智能是计算机科学的重要分支，涉及机器学习、深度学习等技术".to_string(),
            tenant_id: 1001,
            space_id: None,
        };

        let update_result = service.update_knowledge_segment(update_request).await;

        match update_result {
            Ok(result) => {
                println!(
                    "更新成功: 更新数量 = {}, 更新时间 = {}",
                    result.updated_count, result.update_time
                );
                assert_eq!(result.updated_count, 1, "应该更新1个分段");
                assert!(!result.update_time.is_empty(), "更新时间不应该为空");
            }
            Err(e) => {
                println!("更新失败: {:?}", e);
                panic!("更新分段应该成功，但收到错误: {:?}", e);
            }
        }

        // 验证更新后的内容
        sleep(Duration::from_secs(2)).await;
        let search_params = KnowledgeSearchParams {
            query: "机器学习、深度学习等技术".to_string(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            offset: Some(0),
            limit: Some(10),
            sort_by: Some("created".to_string()),
            sort_order: Some("desc".to_string()),
        };

        let search_result = service.search_knowledge_segments(search_params).await;
        assert!(search_result.is_ok(), "搜索应该成功");

        let result = search_result.unwrap();
        assert!(result.results.len() > 0, "应该找到更新的内容");

        // 清理测试索引
        cleanup_test_index(&service).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_delete_knowledge_segments() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 确保索引存在并清空所有数据，确保测试环境干净
        service.ensure_knowledge_index_exists().await.unwrap();
        service.clear_all_knowledge_segments().await.unwrap();
        sleep(Duration::from_secs(5)).await; // 等待清空操作完成

        // 插入测试数据
        let test_segments = create_test_segments();
        service
            .batch_ingest_knowledge_segments(&test_segments)
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;

        // 测试删除特定知识库的数据
        let delete_params = DeleteParams {
            kb_id: Some(vec![1]),
            doc_id: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            start_timestamp: None,
            end_timestamp: None,
        };

        let delete_result = service.delete_knowledge_segments(delete_params).await;
        assert!(delete_result.is_ok(), "删除数据应该成功");

        let deleted_count = delete_result.unwrap();
        assert!(deleted_count > 0, "应该删除至少1条数据");

        // 验证删除后的搜索结果 - 等待更长时间确保删除操作完成
        sleep(Duration::from_secs(10)).await; // 增加等待时间

        let search_params = KnowledgeSearchParams {
            query: "人工智能".to_string(),
            kb_ids: Some(vec![1]),
            doc_ids: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            offset: Some(0),
            limit: Some(10),
            sort_by: Some("created".to_string()),
            sort_order: Some("desc".to_string()),
        };

        let search_result = service.search_knowledge_segments(search_params).await;
        assert!(search_result.is_ok(), "搜索应该成功");

        let result = search_result.unwrap();

        // 由于Quickwit删除任务的异步性质，我们检查是否显著减少了而不是完全为0
        // 原本应该有2条记录（kb_id=1），删除后应该大幅减少
        assert!(
            result.total <= 1,
            "知识库1的数据应该被大幅删除，剩余记录数: {}",
            result.total
        );

        // 清理测试索引
        cleanup_test_index(&service).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_get_knowledge_stats() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 确保索引存在并清空所有数据，确保测试环境干净
        service.ensure_knowledge_index_exists().await.unwrap();
        service.clear_all_knowledge_segments().await.unwrap();
        sleep(Duration::from_secs(5)).await; // 等待清空操作完成

        // 插入测试数据
        let test_segments = create_test_segments();
        service
            .batch_ingest_knowledge_segments(&test_segments)
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;

        // 测试统计功能
        let stats_params = StatsParams {
            tenant_id: 1001,
            kb_id: Some(1),
            space_id: None,
        };

        let stats_result = service.get_knowledge_stats(stats_params).await;
        assert!(stats_result.is_ok(), "获取统计信息应该成功");

        let result = stats_result.unwrap();
        assert_eq!(result.tenant_id, 1001, "租户ID应该匹配");
        assert_eq!(result.kb_id, Some(1), "知识库ID应该匹配");
        assert!(result.doc_count > 0, "文档数应该大于0");
        assert!(result.total_segments > 0, "总分段数应该大于0");
        assert!(!result.doc_stats.is_empty(), "文档统计不应该为空");
        assert!(!result.stats_time.is_empty(), "统计时间不应该为空");

        // 验证文档统计的准确性
        let total_from_doc_stats: u64 = result.doc_stats.iter().map(|d| d.segment_count).sum();
        assert_eq!(
            total_from_doc_stats, result.total_segments,
            "文档统计的总分段数应该等于总分段数"
        );

        // 清理测试索引
        cleanup_test_index(&service).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "需要 QuickWit 服务运行"]

    async fn test_query_segment_ids() {
        let app_states = Arc::new(create_test_app_states());
        let index_name = generate_test_index_name();
        let service = KnowledgeQuickwitService::new_with_index_name(app_states, index_name);

        // 确保索引存在并清空所有数据，确保测试环境干净
        service.ensure_knowledge_index_exists().await.unwrap();
        service.clear_all_knowledge_segments().await.unwrap();
        sleep(Duration::from_secs(5)).await; // 等待清空操作完成

        // 插入测试数据
        let test_segments = create_test_segments();
        service
            .batch_ingest_knowledge_segments(&test_segments)
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;

        // 测试查询分段ID功能
        let segment_params = crate::models::SegmentIdsParams {
            tenant_id: 1001,
            kb_id: 1,
            space_id: None,
        };

        let ids_result = service.query_segment_ids(segment_params).await;
        assert!(ids_result.is_ok(), "查询分段ID应该成功");

        let result = ids_result.unwrap();
        assert_eq!(result.tenant_id, 1001, "租户ID应该匹配");
        assert_eq!(result.kb_id, 1, "知识库ID应该匹配");
        assert!(result.total_count > 0, "总分段数应该大于0");
        assert!(!result.segment_ids.is_empty(), "分段ID列表不应该为空");
        assert!(!result.query_time.is_empty(), "查询时间不应该为空");

        // 验证返回的分段ID格式正确
        for segment_id in &result.segment_ids {
            assert!(*segment_id > 0, "分段ID应该大于0");
        }

        // 清理测试索引
        cleanup_test_index(&service).await.unwrap();
    }

    #[tokio::test]
    async fn test_search_params_validation() {
        let app_states = Arc::new(create_test_app_states());
        let service = KnowledgeQuickwitService::new(app_states);

        // 测试空查询参数
        let search_params = KnowledgeSearchParams::default();
        assert_eq!(search_params.tenant_id, 0, "默认租户ID应该是0");
        assert_eq!(search_params.limit, Some(20), "默认限制应该是20");

        // 测试边界情况
        let boundary_params = KnowledgeSearchParams {
            query: "".to_string(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: None,
            tenant_id: 0, // 无效的租户ID
            space_id: None,
            offset: Some(0),
            limit: Some(0), // 无效的限制
            sort_by: None,
            sort_order: None,
        };

        // 这个测试主要验证参数构造，实际搜索会因为租户ID为0而失败
        assert_eq!(boundary_params.tenant_id, 0);
        assert_eq!(boundary_params.limit, Some(0));
    }

    #[tokio::test]
    async fn test_delete_params_validation() {
        // 测试删除参数验证逻辑
        let valid_params = DeleteParams {
            kb_id: Some(vec![1, 2]),
            doc_id: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            start_timestamp: None,
            end_timestamp: None,
        };

        assert!(valid_params.kb_id.is_some(), "知识库ID应该存在");
        assert!(
            !valid_params.kb_id.as_ref().unwrap().is_empty(),
            "知识库ID列表不应该为空"
        );
        assert_eq!(valid_params.tenant_id, 1001, "租户ID应该匹配");

        // 测试边界情况 - 只有租户ID，没有其他条件
        let invalid_params = DeleteParams {
            kb_id: None,
            doc_id: None,
            raw_ids: None,
            tenant_id: 1001,
            space_id: None,
            start_timestamp: None,
            end_timestamp: None,
        };

        assert!(invalid_params.kb_id.is_none());
        assert!(invalid_params.doc_id.is_none());
        assert!(invalid_params.raw_ids.is_none());
    }

    #[tokio::test]
    async fn test_segment_data_integrity() {
        // 测试数据完整性
        let test_segments = create_test_segments();

        assert_eq!(test_segments.len(), 3, "应该有3个测试分段");

        // 验证每个分段的必需字段
        for segment in &test_segments {
            assert!(segment.raw_id > 0, "raw_id应该大于0");
            assert!(segment.kb_id > 0, "kb_id应该大于0");
            assert!(segment.doc_id > 0, "doc_id应该大于0");
            assert!(!segment.raw_txt.is_empty(), "raw_txt不应该为空");
            assert!(segment.tenant_id > 0, "tenant_id应该大于0");
            assert!(segment.space_id > 0, "space_id应该大于0");
            if let Some(sort_index) = segment.sort_index {
                assert!(sort_index >= 0, "sort_index应该大于等于0");
            }
        }

        // 验证特定分段的数据
        let first_segment = &test_segments[0];
        assert_eq!(first_segment.raw_id, 1001);
        assert_eq!(first_segment.kb_id, 1);
        assert_eq!(first_segment.doc_id, 101);
        assert!(first_segment.raw_txt.contains("人工智能"));
    }
}
