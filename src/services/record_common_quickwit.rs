use crate::{
    AppStates,
    config::QuickwitConfig,
    index_define::{DEFAULT_RECORD_COMMON_LOG_INDEX, get_record_common_log_index_config},
    models::{
        LogEntry, LogLevel, LogQuery, LogSearchResult, ServiceDependency, ServiceStats, TraceQuery,
    },
    my_error::AppError,
};
use anyhow::Result;
use chrono::Utc;
use log::info;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Clone)]
pub struct RecordCommonLogQuickwitService {
    app_states: Arc<AppStates>,
    index_name: String,
}

impl RecordCommonLogQuickwitService {
    pub fn new(app_states: Arc<AppStates>) -> Self {
        Self {
            app_states,
            index_name: DEFAULT_RECORD_COMMON_LOG_INDEX.to_string(),
        }
    }

    /// 使用默认索引名称创建服务实例
    pub fn new_with_default_index(app_states: Arc<AppStates>) -> Self {
        Self::new(app_states)
    }

    /// 使用指定的索引名称创建服务实例
    pub fn new_with_index_name(app_states: Arc<AppStates>, index_name: String) -> Self {
        Self {
            app_states,
            index_name,
        }
    }

    /// 获取QuickWit URL
    pub fn get_url(&self) -> &str {
        &self.app_states.config.url
    }

    /// 获取索引名称
    pub fn get_index_name(&self) -> &str {
        &self.index_name
    }

    /// 等待索引就绪
    pub async fn wait_for_index_ready(
        &self,
        max_retries: u32,
        delay_secs: u64,
    ) -> Result<bool, AppError> {
        info!("等待索引就绪: {}", self.index_name);
        for i in 0..max_retries {
            let response = self
                .app_states
                .client
                .get(format!(
                    "{}/api/v1/indexes/{}",
                    self.app_states.config.url, self.index_name
                ))
                .send()
                .await;

            if response.is_ok() {
                info!("索引已就绪: {}", self.index_name);
                return Ok(true);
            }

            info!(
                "索引未就绪，尝试 {}/{}，等待 {} 秒...",
                i + 1,
                max_retries,
                delay_secs
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }

        Err(AppError::QuickWit(format!(
            "索引 {} 等待超时",
            self.index_name
        )))
    }

    /// 确认索引中有数据
    pub async fn ensure_index_has_data(
        &self,
        max_retries: u32,
        delay_secs: u64,
    ) -> Result<bool, AppError> {
        info!("确认索引 {} 中是否有数据", self.index_name);
        for i in 0..max_retries {
            let query = LogQuery {
                query: "*".to_string(),
                start_timestamp: None,
                end_timestamp: None,
                start_offset: None,
                max_hits: Some(10),
            };

            match self.search_logs(&query).await {
                Ok(result) if !result.hits.is_empty() => {
                    info!("索引 {} 中有 {} 条数据", self.index_name, result.hits.len());
                    return Ok(true);
                }
                _ => {
                    info!(
                        "索引 {} 中暂无数据，尝试 {}/{}，等待 {} 秒...",
                        self.index_name,
                        i + 1,
                        max_retries,
                        delay_secs
                    );
                    sleep(Duration::from_secs(delay_secs)).await;
                }
            }
        }

        Err(AppError::QuickWit(format!(
            "索引 {} 数据确认超时",
            self.index_name
        )))
    }

    /// 创建日志索引
    pub async fn create_index(&self) -> Result<(), AppError> {
        // 使用索引定义文件中的配置
        let index_config = get_record_common_log_index_config(&self.index_name);

        // 检查索引是否已存在
        info!("检查索引 {} 是否存在", self.index_name);

        let check_url = format!(
            "{}/api/v1/indexes/{}",
            self.app_states.config.url, self.index_name
        );
        info!("检查索引URL: {}", check_url);

        // 使用匹配模式避免borrow after move错误
        let index_exists = match self.app_states.client.get(&check_url).send().await {
            Ok(response) if response.status().is_success() => {
                info!("索引存在检查成功, 状态码: {}", response.status());
                true
            }
            Ok(response) => {
                info!("索引不存在, 状态码: {}", response.status());
                false
            }
            Err(err) => {
                info!("索引检查请求失败: {}", err);
                false
            }
        };

        if !index_exists {
            info!("创建索引 {}", self.index_name);

            let create_url = format!("{}/api/v1/indexes", self.app_states.config.url);
            info!("创建索引URL: {}", create_url);
            info!(
                "索引配置: {}",
                serde_json::to_string(&index_config).unwrap_or_default()
            );

            let create_response = self
                .app_states
                .client
                .post(&create_url)
                .header("content-type", "application/json")
                .json(&index_config)
                .send()
                .await?;

            // 先保存状态码再处理响应
            let status = create_response.status();

            if !status.is_success() {
                let error_text = create_response
                    .text()
                    .await
                    .unwrap_or_else(|_| "无法获取错误信息".to_string());

                info!("创建索引失败，状态码: {}, 错误: {}", status, error_text);

                return Err(AppError::QuickWit(format!("创建索引失败: {}", error_text)));
            }

            // 等待索引创建完成 - 增加等待时间
            info!("等待索引 {} 创建完成", self.index_name);
            sleep(Duration::from_secs(2)).await;
        } else {
            info!("索引 {} 已存在", self.index_name);
        }

        Ok(())
    }

    /// 写入日志
    pub async fn ingest_log(&self, log: &LogEntry) -> Result<(), AppError> {
        info!("向索引 {} 写入单条日志", self.index_name);

        info!(
            "单条日志写入请求URL: {}/api/v1/indexes/{}/ingest?commit=force",
            self.app_states.config.url, self.index_name
        );

        let ingest_response = self
            .app_states
            .client
            .post(format!(
                "{}/api/v1/indexes/{}/ingest?commit=force",
                self.app_states.config.url, self.index_name
            ))
            .header("Content-Type", "application/json")
            .json(log)
            .send()
            .await?;

        // 先保存状态码再处理响应
        let status = ingest_response.status();

        if !status.is_success() {
            let error_text = ingest_response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误信息".to_string());

            info!("单条日志写入失败，状态码: {}, 错误: {}", status, error_text);

            return Err(AppError::QuickWit(format!("写入日志失败: {}", error_text)));
        }

        info!("单条日志写入成功，状态码: {}", status);
        Ok(())
    }

    /// 批量写入日志
    pub async fn batch_ingest_logs(&self, logs: &[LogEntry]) -> Result<(), AppError> {
        info!("向索引 {} 批量写入 {} 条日志", self.index_name, logs.len());
        // 生成符合NDJSON格式的请求体，每行是一个JSON对象，不带逗号分隔
        let ndjson = logs
            .iter()
            .map(|log| serde_json::to_string(log).unwrap())
            .collect::<Vec<_>>()
            .join("\n");

        info!(
            "批量写入请求URL: {}/api/v1/indexes/{}/ingest?commit=force",
            self.app_states.config.url, self.index_name
        );

        let batch_response = self
            .app_states
            .client
            .post(format!(
                "{}/api/v1/indexes/{}/ingest?commit=force",
                self.app_states.config.url, self.index_name
            ))
            // QuickWit API接受application/json和application/x-ndjson格式
            .header("Content-Type", "application/json")
            .body(ndjson)
            .send()
            .await?;

        // 先保存状态码再处理响应
        let status = batch_response.status();

        if !status.is_success() {
            let error_text = batch_response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误信息".to_string());

            info!("批量写入失败，状态码: {}, 错误: {}", status, error_text);

            return Err(AppError::QuickWit(format!(
                "批量写入日志失败: {}",
                error_text
            )));
        }

        info!("批量写入成功，状态码: {}", status);
        Ok(())
    }

    /// 查询日志
    pub async fn search_logs(&self, query: &LogQuery) -> Result<LogSearchResult, AppError> {
        let mut search_params = serde_json::Map::new();
        search_params.insert("query".to_string(), json!(query.query));

        if let Some(start_timestamp) = query.start_timestamp {
            search_params.insert("start_timestamp".to_string(), json!(start_timestamp));
        }

        if let Some(end_timestamp) = query.end_timestamp {
            search_params.insert("end_timestamp".to_string(), json!(end_timestamp));
        }

        if let Some(start_offset) = query.start_offset {
            search_params.insert("start_offset".to_string(), json!(start_offset));
        }

        if let Some(max_hits) = query.max_hits {
            search_params.insert("max_hits".to_string(), json!(max_hits));
        }

        let response = self
            .app_states
            .client
            .post(format!(
                "{}/api/v1/indexes/{}/search",
                self.app_states.config.url, self.index_name
            ))
            .json(&search_params)
            .send()
            .await?
            .json::<Value>()
            .await?;

        let hits = response["hits"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|hit| {
                let source = hit["source"].clone();
                serde_json::from_value::<LogEntry>(source).ok()
            })
            .collect::<Vec<_>>();

        let total_hits = response["num_hits"].as_i64().unwrap_or(0);
        let elapsed_time_ms = response["elapsed_time_micros"].as_i64().unwrap_or(0) / 1000;

        Ok(LogSearchResult {
            hits,
            total_hits,
            elapsed_time_ms,
        })
    }

    /// 根据trace_id查询完整的调用链
    pub async fn get_trace(&self, trace_id: &str) -> Result<Vec<LogEntry>, AppError> {
        let query = format!("trace_id:{}", trace_id);
        let log_query = LogQuery {
            query,
            start_timestamp: None,
            end_timestamp: None,
            start_offset: None,
            max_hits: Some(1000), // 获取足够多的spans
        };

        let result = self.search_logs(&log_query).await?;

        // 按时间排序
        let mut spans = result.hits;
        spans.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(spans)
    }

    /// 计算服务间的依赖关系
    pub async fn calculate_service_dependencies(
        &self,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> Result<Vec<ServiceDependency>, AppError> {
        // 查询所有具有parent_span_id的日志条目
        let query = LogQuery {
            query: "parent_span_id:*".to_string(),
            start_timestamp,
            end_timestamp,
            start_offset: None,
            max_hits: Some(10000),
        };

        let result = self.search_logs(&query).await?;

        // 统计服务之间的调用关系
        let mut dependencies: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();

        for log in &result.hits {
            if let (Some(parent_span), Some(service)) = (&log.parent_span_id, &log.service) {
                // 查找父span所属的服务
                let parent_query = LogQuery {
                    query: format!("span_id:{}", parent_span),
                    start_timestamp: None,
                    end_timestamp: None,
                    start_offset: None,
                    max_hits: Some(1),
                };

                if let Ok(parent_result) = self.search_logs(&parent_query).await {
                    if let Some(parent_log) = parent_result.hits.first() {
                        if let Some(parent_service) = &parent_log.service {
                            // 增加依赖计数
                            let key = (parent_service.clone(), service.clone());
                            *dependencies.entry(key).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // 转换为结果格式
        let dependencies = dependencies
            .into_iter()
            .map(|((parent, child), calls_count)| ServiceDependency {
                parent,
                child,
                calls_count,
            })
            .collect::<Vec<_>>();

        Ok(dependencies)
    }

    /// 获取服务性能统计
    pub async fn get_service_stats(
        &self,
        service_name: Option<&str>,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> Result<Vec<ServiceStats>, AppError> {
        // 根据服务名称构建查询
        let query_str = if let Some(service) = service_name {
            format!("service:{}", service)
        } else {
            "service:*".to_string()
        };

        let query = LogQuery {
            query: query_str,
            start_timestamp,
            end_timestamp,
            start_offset: None,
            max_hits: Some(10000),
        };

        let result = self.search_logs(&query).await?;

        // 按服务名称分组统计
        let mut stats: std::collections::HashMap<String, (i64, i64, f64)> =
            std::collections::HashMap::new();

        for log in &result.hits {
            if let Some(service) = &log.service {
                let (op_count, error_count, total_duration) =
                    stats.entry(service.clone()).or_insert((0, 0, 0.0));

                // 增加操作计数
                *op_count += 1;

                // 如果有错误，增加错误计数
                if log.error.unwrap_or(false) {
                    *error_count += 1;
                }

                // 如果有耗时，累加总耗时
                if let Some(duration) = log.duration_ms {
                    *total_duration += duration as f64;
                }
            }
        }

        // 转换为结果格式
        let service_stats = stats
            .into_iter()
            .map(
                |(service_name, (operation_count, error_count, total_duration))| {
                    let avg_duration_ms = if operation_count > 0 {
                        total_duration / operation_count as f64
                    } else {
                        0.0
                    };

                    ServiceStats {
                        service_name,
                        operation_count,
                        error_count,
                        avg_duration_ms,
                    }
                },
            )
            .collect::<Vec<_>>();

        Ok(service_stats)
    }

    /// 与Jaeger集成的接口，转换QuickWit查询结果为Jaeger格式
    pub async fn export_to_jaeger_format(
        &self,
        trace_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let spans = self.get_trace(trace_id).await?;

        // 收集所有的process信息
        let mut processes = serde_json::Map::new();
        let mut span_jsons = Vec::new();

        for span in &spans {
            // 处理process信息
            if let Some(service) = &span.service {
                if !processes.contains_key(service) {
                    let process_info = json!({
                        "serviceName": service,
                        "tags": []
                    });
                    processes.insert(service.clone(), process_info);
                }
            }

            // 转换span信息
            let span_json = json!({
                "traceID": span.trace_id.clone().unwrap_or_else(|| trace_id.to_string()),
                "spanID": span.span_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                "operationName": span.operation_name.clone().unwrap_or_else(|| "unknown".to_string()),
                "references": span.parent_span_id.as_ref().map(|parent_id| {
                    vec![json!({
                        "refType": "CHILD_OF",
                        "traceID": span.trace_id.clone().unwrap_or_else(|| trace_id.to_string()),
                        "spanID": parent_id
                    })]
                }).unwrap_or_else(|| Vec::new()),
                "startTime": span.timestamp.timestamp_micros(),
                "duration": span.duration_ms.unwrap_or(0) * 1000, // 转为微秒
                "tags": span.tags.clone().unwrap_or_else(|| json!({})),
                "logs": [
                    {
                        "timestamp": span.timestamp.timestamp_micros(),
                        "fields": [
                            {
                                "key": "message",
                                "value": span.message.clone()
                            }
                        ]
                    }
                ],
                "processID": span.service.clone().unwrap_or_else(|| "unknown".to_string()),
                "warnings": null
            });

            span_jsons.push(span_json);
        }

        // 构建完整的Jaeger格式
        let jaeger_format = json!({
            "traceID": trace_id,
            "spans": span_jsons,
            "processes": processes,
            "warnings": null
        });

        Ok(jaeger_format)
    }

    /// 自动创建日志采集器，方便集成到应用中
    pub fn create_log_collector(&self, service_name: String) -> LogCollector {
        LogCollector::new(service_name, self.app_states.clone())
    }
}

/// 日志采集器，方便应用集成
#[derive(Clone)]
pub struct LogCollector {
    service: String,
    quickwit_service: Arc<AppStates>,
    current_trace_id: Option<String>,
    current_span_id: Option<String>,
    parent_span_id: Option<String>,
    host: Option<String>,
    ip: Option<String>,
    app_version: Option<String>,
    index_name: String,
}

impl LogCollector {
    pub fn new(service: String, quickwit_service: Arc<AppStates>) -> Self {
        Self {
            service,
            quickwit_service,
            current_trace_id: None,
            current_span_id: None,
            parent_span_id: None,
            host: None,
            ip: None,
            app_version: None,
            index_name: DEFAULT_RECORD_COMMON_LOG_INDEX.to_string(),
        }
    }

    /// 设置环境信息
    pub fn set_environment(
        &mut self,
        host: Option<String>,
        ip: Option<String>,
        app_version: Option<String>,
    ) -> &mut Self {
        self.host = host;
        self.ip = ip;
        self.app_version = app_version;
        self
    }

    /// 开始一个新的trace
    pub fn start_trace(&mut self) -> String {
        let trace_id = Uuid::new_v4().to_string();
        self.current_trace_id = Some(trace_id.clone());
        self.current_span_id = None;
        self.parent_span_id = None;
        trace_id
    }

    /// 开始一个新的span
    pub fn start_span(&mut self, _operation_name: &str, parent_span_id: Option<String>) -> String {
        let span_id = Uuid::new_v4().to_string();
        self.parent_span_id = parent_span_id.or_else(|| self.current_span_id.clone());
        self.current_span_id = Some(span_id.clone());

        span_id
    }

    /// 结束当前span并记录
    pub async fn end_span(
        &mut self,
        operation_name: &str,
        duration_ms: i64,
        error: bool,
    ) -> Result<(), AppError> {
        if let (Some(trace_id), Some(span_id)) = (&self.current_trace_id, &self.current_span_id) {
            let log_entry = LogEntry {
                id: Some(Uuid::new_v4().to_string()),
                timestamp: Utc::now(),
                level: if error {
                    LogLevel::Error
                } else {
                    LogLevel::Info
                },
                message: format!("Operation: {}", operation_name),
                service: Some(self.service.clone()),
                trace_id: Some(trace_id.clone()),
                span_id: Some(span_id.clone()),
                parent_span_id: self.parent_span_id.clone(),
                operation_name: Some(operation_name.to_string()),
                host: self.host.clone(),
                ip: self.ip.clone(),
                app_version: self.app_version.clone(),
                duration_ms: Some(duration_ms),
                status_code: None,
                error: Some(error),
                metadata: None,
                tags: None,
            };

            // 使用RecordCommonLogQuickwitService来处理日志写入
            let quickwit_service = RecordCommonLogQuickwitService {
                app_states: self.quickwit_service.clone(),
                index_name: self.index_name.clone(),
            };
            quickwit_service.ingest_log(&log_entry).await?;
        }

        Ok(())
    }

    /// 记录一条日志
    pub async fn log(
        &self,
        level: LogLevel,
        message: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let log_entry = LogEntry {
            id: Some(Uuid::new_v4().to_string()),
            timestamp: Utc::now(),
            level,
            message: message.to_string(),
            service: Some(self.service.clone()),
            trace_id: self.current_trace_id.clone(),
            span_id: self.current_span_id.clone(),
            parent_span_id: self.parent_span_id.clone(),
            operation_name: None,
            host: self.host.clone(),
            ip: self.ip.clone(),
            app_version: self.app_version.clone(),
            duration_ms: None,
            status_code: None,
            error: None,
            metadata,
            tags: None,
        };

        // 使用RecordCommonLogQuickwitService来处理日志写入
        let quickwit_service = RecordCommonLogQuickwitService {
            app_states: self.quickwit_service.clone(),
            index_name: self.index_name.clone(),
        };
        quickwit_service.ingest_log(&log_entry).await
    }
}
