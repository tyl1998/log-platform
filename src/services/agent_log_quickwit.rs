use crate::{
    AppStates,
    index_define::{DEFAULT_AGENT_LOG_INDEX, get_agent_index_config},
    models::{AgentLogEntry, AgentLogSearchParams, AgentLogSearchResult, PageQuery},
    my_error::AppError,
};
use anyhow::Result;
use chrono;
use log::{debug, info};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 智能体日志专用QuickWit服务
/// 用于处理智能体日志的索引创建、日志写入和查询
#[derive(Clone)]
pub struct AgentLogQuickwitService {
    app_states: Arc<AppStates>,
    index_name: String,
}

impl AgentLogQuickwitService {
    /// 创建新的智能体日志QuickWit服务实例
    pub fn new(app_states: Arc<AppStates>) -> Self {
        Self {
            app_states,
            index_name: DEFAULT_AGENT_LOG_INDEX.to_string(),
        }
    }

    /// 使用自定义索引名称创建服务实例
    pub fn new_with_index_name(app_states: Arc<AppStates>, index_name: String) -> Self {
        Self {
            app_states,
            index_name,
        }
    }

    /// 使用默认索引名称创建服务实例
    pub fn new_with_default_index(app_states: Arc<AppStates>) -> Self {
        Self::new(app_states)
    }

    /// 获取QuickWit URL
    pub fn get_url(&self) -> &str {
        &self.app_states.config.url
    }

    /// 获取智能体日志索引名
    pub fn get_agent_index_name(&self) -> &str {
        &self.index_name
    }

    /// 检查智能体日志索引是否存在
    pub async fn check_agent_index_exists(&self) -> Result<bool, AppError> {
        let agent_index = self.get_agent_index_name();
        info!("检查智能体日志索引 {} 是否存在", agent_index);

        let response = self
            .app_states
            .client
            .get(format!(
                "{}/api/v1/indexes/{}",
                self.app_states.config.url, agent_index
            ))
            .send()
            .await;

        // 如果请求成功且状态码是成功的，则索引存在
        let index_exists = response.is_ok() && response.unwrap().status().is_success();

        if index_exists {
            info!("智能体日志索引 {} 已存在", agent_index);
        } else {
            info!("智能体日志索引 {} 不存在", agent_index);
        }

        Ok(index_exists)
    }

    /// 创建智能体日志索引
    pub async fn create_agent_index(&self) -> Result<(), AppError> {
        let agent_index = self.get_agent_index_name();
        info!("创建智能体日志索引 {}", agent_index);

        // 获取智能体索引配置
        let index_config = get_agent_index_config(agent_index);

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
                "创建智能体日志索引失败: {}",
                error_text
            )));
        }

        info!("智能体日志索引 {} 创建成功", agent_index);

        // 等待索引就绪
        sleep(Duration::from_secs(2)).await;

        Ok(())
    }

    /// 确保智能体日志索引存在，如不存在则创建
    pub async fn ensure_agent_index_exists(&self) -> Result<(), AppError> {
        // 检查索引是否存在
        let index_exists = self.check_agent_index_exists().await?;
        let agent_index = self.get_agent_index_name();

        // 如果索引不存在，则创建
        if !index_exists {
            self.create_agent_index().await?;

            // 等待索引就绪
            let mut retries = 0;
            let max_retries = 10;

            while retries < max_retries {
                match self.check_agent_index_exists().await {
                    Ok(true) => return Ok(()),
                    _ => {
                        retries += 1;
                        info!(
                            "等待智能体日志索引就绪，尝试 {}/{}，等待 2 秒...",
                            retries, max_retries
                        );
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            return Err(AppError::QuickWit(format!(
                "智能体日志索引 {} 创建后等待就绪超时",
                agent_index
            )));
        }

        Ok(())
    }

    /// 批量摄取智能体日志前自动确保索引存在
    pub async fn batch_ingest_agent_logs_with_no_index_check(
        &self,
        logs: &[AgentLogEntry],
    ) -> Result<(), AppError> {
        let agent_index = self.get_agent_index_name();
        info!(
            "开始批量摄取智能体日志，索引: {}, 日志数量: {}",
            agent_index,
            logs.len()
        );

        // 批量摄取日志
        info!("开始批量摄取操作...");
        self.batch_ingest_agent_logs(logs).await
    }

    /// 摄取智能体日志
    pub async fn ingest_agent_log(&self, log: &AgentLogEntry) -> Result<(), AppError> {
        let agent_index = self.get_agent_index_name();
        info!("摄取智能体日志: {}, 索引: {}", log.request_id, agent_index);

        // 创建日志副本并设置时间戳
        let mut log_with_timestamp = log.clone();
        // 如果没有设置created_at，则自动设置为当前时间
        if log_with_timestamp.created_at.is_none() {
            log_with_timestamp.created_at = Some(chrono::Utc::now());
        }
        // 更新updated_at为当前时间
        log_with_timestamp.updated_at = Some(chrono::Utc::now());

        let url = format!(
            "{}/api/v1/{}/ingest?commit=force",
            self.app_states.config.url, agent_index
        );

        let response = self
            .app_states
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&log_with_timestamp)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "摄取智能体日志失败: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 批量摄取智能体日志
    pub async fn batch_ingest_agent_logs(&self, logs: &[AgentLogEntry]) -> Result<(), AppError> {
        if logs.is_empty() {
            return Ok(());
        }

        let agent_index = self.get_agent_index_name();
        info!("批量摄取智能体日志: {} 条", logs.len());

        // 创建带有时间戳的日志副本
        let now = chrono::Utc::now();
        let logs_with_timestamp: Vec<AgentLogEntry> = logs
            .iter()
            .map(|log| {
                let mut log_copy = log.clone();
                // 如果没有设置created_at，则自动设置为当前时间
                if log_copy.created_at.is_none() {
                    log_copy.created_at = Some(now);
                }
                // 更新updated_at为当前时间
                log_copy.updated_at = Some(now);
                log_copy
            })
            .collect();

        let url = format!(
            "{}/api/v1/{}/ingest?commit=force",
            self.app_states.config.url, agent_index
        );

        // 将日志转换为NDJSON格式
        let ndjson = logs_with_timestamp
            .iter()
            .map(|log| serde_json::to_string(log).unwrap_or_default())
            .collect::<Vec<String>>()
            .join("\n");

        info!("发送批量摄取请求到: {}", url);
        info!(
            "请求体前500字符: {}",
            &ndjson[0..std::cmp::min(500, ndjson.len())]
        );

        let response = self
            .app_states
            .client
            .post(&url)
            .header("content-type", "application/json")
            .body(ndjson)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "批量摄取智能体日志失败: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 搜索智能体日志
    pub async fn search_agent_logs(
        &self,
        params: PageQuery<AgentLogSearchParams>,
    ) -> Result<AgentLogSearchResult, AppError> {
        let agent_index = self.get_agent_index_name();
        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, agent_index
        );

        // 根据页大小,当前页,计算offset
        let offset = params.get_offset();
        let max_hits = params.get_max_hits();

        // 构建基本查询
        let mut query_parts = Vec::new();

        // 如果有查询过滤器，则根据条件构建查询
        if let Some(ref filter) = params.query_filter {
            // 使用宏生成的方法直接构建查询部分
            query_parts = filter.build_query_parts();
        }

        // 构建最终查询字符串，如果没有任何条件则查询所有
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

        // 添加时间范围过滤（如果提供）
        if let Some(ref filter) = params.query_filter {
            if filter.start_time.is_some() || filter.end_time.is_some() {
                if let Some(start_time) = filter.start_time {
                    search_request["start_timestamp"] = json!(start_time.timestamp());
                }

                if let Some(end_time) = filter.end_time {
                    search_request["end_timestamp"] = json!(end_time.timestamp());
                }
            }
        }

        // 添加排序字段（如果提供）
        if let Some(orders) = &params.orders {
            if !orders.is_empty() {
                // 定义可以用于排序的字段（必须是非text类型且fast为true的字段）
                let sortable_fields = vec![
                    "input_token",
                    "output_token",
                    "request_start_time",
                    "request_end_time",
                    "elapsed_time_ms",
                    "created_at",
                ];

                // 过滤掉不支持排序的字段
                let valid_sort_fields: Vec<String> = orders
                    .iter()
                    .filter(|order| sortable_fields.contains(&order.column.as_str()))
                    .map(|order| {
                        // 根据排序方向构建排序字段
                        // 如果是降序，在字段名前加"-"
                        let field = &order.column;
                        if order.asc {
                            format!("-{}", field) // 升序，在字段名前加"-"
                        } else {
                            field.clone() // 降序，直接使用字段名
                        }
                    })
                    .collect();

                if !valid_sort_fields.is_empty() {
                    // 将所有排序字段用逗号连接成一个字符串
                    let sort_by_string = valid_sort_fields.join(",");
                    search_request["sort_by"] = json!(sort_by_string);
                    info!("添加排序字段: {}", sort_by_string);
                } else {
                    // 如果没有有效的排序字段，使用默认时间排序
                    search_request["sort_by"] = json!("request_start_time");
                    info!("没有有效的排序字段，使用默认时间排序");
                }
            }
        } else {
            //没有排序字段，默认按时间排序: request_start_time
            search_request["sort_by"] = json!("request_start_time");
        }

        info!("智能体日志搜索请求: {}", search_request);

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
                "搜索智能体日志失败: {}",
                error_text
            )));
        }

        let search_response: Value = response.json().await?;

        debug!("智能体日志搜索响应: {}", search_response);

        // 解析搜索结果
        let hits = search_response["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("搜索结果解析失败: 缺少hits数组".to_string()))?;

        let mut agent_logs = Vec::with_capacity(hits.len());

        for hit in hits {
            // 尝试两种解析方式：
            // 1. 直接从hit解析（新版本QuickWit的格式）
            // 2. 从hit["source"]解析（旧版本或特定配置的格式）
            let log_result =
                if let Ok(mut log) = serde_json::from_value::<AgentLogEntry>(hit.clone()) {
                    // execute_result 字段太大,去掉
                    log.execute_result = None;
                    Some(log)
                } else if let Some(source) = hit.get("source") {
                    serde_json::from_value::<AgentLogEntry>(source.clone()).ok()
                } else {
                    None
                };

            match log_result {
                Some(log) => agent_logs.push(log),
                None => {
                    info!("解析智能体日志条目失败: {}", hit);
                }
            }
        }

        let total = search_response["num_hits"].as_i64().unwrap_or_default();

        let elapsed_time_ms = search_response["elapsed_time_micros"]
            .as_i64()
            .unwrap_or_default()
            / 1000;

        // 使用新构造函数创建结果
        let result = AgentLogSearchResult::new(agent_logs, total, elapsed_time_ms)
            .with_pagination(params.current, params.page_size);

        Ok(result)
    }

    /// 搜索智能体日志,根据 request_id 查询单个日志详情
    pub async fn search_agent_log_detail(
        &self,
        params: PageQuery<AgentLogSearchParams>,
    ) -> Result<AgentLogSearchResult, AppError> {
        let agent_index = self.get_agent_index_name();
        let search_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, agent_index
        );

        // 只查询一条数据
        let offset = 0;
        let max_hits = 1;

        // 构建基本查询
        let mut query_parts = Vec::new();

        // 如果有查询过滤器，则根据条件构建查询
        if let Some(ref filter) = params.query_filter {
            // 使用宏生成的方法直接构建查询部分
            query_parts = filter.build_query_parts();

            if let Some(request_id) = filter.request_id.clone() {
                query_parts.push(format!("request_id:{}", request_id));
            } else {
                return Err(AppError::QuickWit(
                    "搜索智能体日志失败: request_id 不能为空".to_string(),
                ));
            }
            //agent_id
            if let Some(agent_id) = filter.agent_id.clone() {
                query_parts.push(format!("agent_id:{}", agent_id));
            }
        }

        // 构建最终查询字符串，如果没有任何条件则查询所有
        let query_string = if query_parts.is_empty() {
            "*".to_string()
        } else {
            query_parts.join(" AND ")
        };

        // 构建搜索请求
        let search_request = json!({
            "query": query_string,
            "max_hits": max_hits,
            "start_offset": offset,
        });

        info!("智能体日志详情搜索请求: {}", search_request);

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
                "搜索智能体详情日志失败: {}",
                error_text
            )));
        }

        let search_response: Value = response.json().await?;

        debug!("智能体详情日志搜索响应: {}", search_response);

        // 解析搜索结果
        let hits = search_response["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("搜索结果解析失败: 缺少hits数组".to_string()))?;

        let mut agent_logs = Vec::with_capacity(hits.len());

        for hit in hits {
            // 尝试两种解析方式：
            // 1. 直接从hit解析（新版本QuickWit的格式）
            // 2. 从hit["source"]解析（旧版本或特定配置的格式）
            let log_result = if let Ok(log) = serde_json::from_value::<AgentLogEntry>(hit.clone()) {
                Some(log)
            } else if let Some(source) = hit.get("source") {
                serde_json::from_value::<AgentLogEntry>(source.clone()).ok()
            } else {
                None
            };

            match log_result {
                Some(log) => agent_logs.push(log),
                None => {
                    info!("解析智能体日志条目失败: {}", hit);
                }
            }
        }

        let total = search_response["num_hits"].as_i64().unwrap_or_default();

        let elapsed_time_ms = search_response["elapsed_time_micros"]
            .as_i64()
            .unwrap_or_default()
            / 1000;

        // 使用新构造函数创建结果
        let result = AgentLogSearchResult::new(agent_logs, total, elapsed_time_ms)
            .with_pagination(params.current, params.page_size);

        Ok(result)
    }

    /// 删除索引
    pub async fn delete_agent_logs(&self, index_name: String) -> Result<(), AppError> {
        let delete_url = format!(
            "{}/api/v1/indexes/{}",
            self.app_states.config.url, index_name
        );
        let response = self.app_states.client.delete(&delete_url).send().await?;
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "删除智能体日志索引失败: {}",
                error_text
            )));
        }
        Ok(())
    }

    /// 添加向量类型字段查询
    fn add_vector_field_query(
        field_name: &str,
        values: &[String],
        operator: &str,
        query_parts: &mut Vec<String>,
    ) {
        let valid_values: Vec<String> = values
            .iter()
            .filter(|id| !id.trim().is_empty())
            .map(|id| format!("{}:{}", field_name, id.trim()))
            .collect();

        if !valid_values.is_empty() {
            if valid_values.len() == 1 {
                query_parts.push(valid_values[0].clone());
            } else {
                query_parts.push(format!("({})", valid_values.join(operator)));
            }
        }
    }
}
