use crate::{
    index_define::DEFAULT_AGENT_LOG_INDEX_V1, models::AppStates, my_error::AppError,
    services::AgentLogQuickwitService, storage::RedbStorageOptimized,
};
use chrono::Duration as ChronoDuration;
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 迁移状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    /// 是否已完成
    pub completed: bool,
    /// 已迁移的文档数量
    pub migrated_count: u64,
    /// 最后迁移时间
    pub last_migration_time: String,
    /// 源索引名称
    pub source_index: String,
    /// 目标索引名称
    pub target_index: String,
    /// 迁移开始时间
    pub started_at: Option<String>,
    /// 迁移完成时间
    pub completed_at: Option<String>,
    /// 源索引总文档数
    pub source_total_count: u64,
    /// 最后一批迁移的文档数
    pub last_batch_size: usize,
    /// 当前迁移批次
    pub current_batch: u64,
    /// 错误信息
    pub error_message: Option<String>,
    /// 已迁移的 request_id 列表（用于断点续传和验证）
    /// 注意：为了性能，只保留最近的一批，完整列表存储在单独的表中
    pub last_batch_request_ids: Vec<String>,
    /// 扫描进度偏移（用于分页推进的断点续扫）
    #[serde(default)]
    pub scan_offset: usize,
    /// 最近一批扫描的文档数量（用于观测与调试）
    #[serde(default)]
    pub last_scanned_count: usize,
}

impl MigrationStatus {
    pub fn new(source_index: String, target_index: String) -> Self {
        Self {
            completed: false,
            migrated_count: 0,
            last_migration_time: chrono::Utc::now().to_rfc3339(),
            source_index,
            target_index,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
            source_total_count: 0,
            last_batch_size: 0,
            current_batch: 0,
            error_message: None,
            last_batch_request_ids: Vec::new(),
            scan_offset: 0,
            last_scanned_count: 0,
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.last_migration_time = chrono::Utc::now().to_rfc3339();
    }

    pub fn update_progress(
        &mut self,
        batch_size: usize,
        migrated_count: u64,
        request_ids: Vec<String>,
    ) {
        self.last_batch_size = batch_size;
        self.migrated_count = migrated_count;
        self.current_batch += 1;
        self.last_migration_time = chrono::Utc::now().to_rfc3339();
        self.last_batch_request_ids = request_ids;
    }

    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.last_migration_time = chrono::Utc::now().to_rfc3339();
    }
}

// 键名常量
const MIGRATION_KEY: &str = "current";

/// Agent 日志迁移管理器
pub struct AgentLogMigrationManager {
    app_states: Arc<AppStates>,
    storage: RedbStorageOptimized,
    tenant_id: String,
    processed_count: AtomicU64,
    current_batch: u64,
}

impl AgentLogMigrationManager {
    /// 创建新的迁移管理器
    ///
    /// # 参数
    /// * `app_states` - 应用状态
    /// * `storage` - 存储实例
    pub fn new(app_states: Arc<AppStates>, storage: RedbStorageOptimized) -> Self {
        Self {
            app_states,
            storage,
            tenant_id: "default".to_string(),
            processed_count: AtomicU64::new(0),
            current_batch: 0,
        }
    }

    /// 使用默认存储创建迁移管理器
    ///
    /// 使用默认数据库：`storage::DEFAULT_DB_PATH`
    /// 数据存储在表：`storage::tables::MIGRATION_STATUS`
    pub fn new_with_default_storage(app_states: Arc<AppStates>) -> Result<Self, AppError> {
        let storage = RedbStorageOptimized::new_default()
            .map_err(|e| AppError::InternalServer(format!("创建存储失败: {}", e)))?;
        Ok(Self::new(app_states, storage))
    }

    /// 执行迁移（支持断点续传）
    pub async fn migrate(&mut self) -> Result<(), AppError> {
        let source_index = DEFAULT_AGENT_LOG_INDEX_V1;
        let service = AgentLogQuickwitService::new(self.app_states.clone());
        let target_index = service.get_agent_index_name();

        info!("🚀 开始数据迁移检查: {} -> {}", source_index, target_index);

        // 1. 优先检查 redb 中的迁移状态
        let mut status = match self.load_migration_status().await {
            Ok(status) => {
                if status.completed {
                    info!("✅ 数据迁移已完成，无需重复迁移");
                    return Ok(());
                }
                info!("🔄 检测到未完成的迁移任务，继续迁移");
                status
            }
            Err(_) => {
                info!("📝 创建新的迁移状态");
                MigrationStatus::new(source_index.to_string(), target_index.to_string())
            }
        };

        // 2. 检查源索引是否存在和是否为空
        if !self.check_index_exists(source_index).await {
            info!("源索引不存在，无需迁移");
            status.mark_completed();
            self.save_migration_status(&status).await?;
            return Ok(());
        }

        let source_total = self
            .get_index_document_count(source_index)
            .await
            .unwrap_or(0);
        if source_total == 0 {
            info!("源索引为空，无需迁移");
            status.mark_completed();
            self.save_migration_status(&status).await?;
            return Ok(());
        }

        status.source_total_count = source_total;
        info!("源索引 {} 总文档数: {}", source_index, source_total);

        // 3. 执行基于时间的分批迁移
        info!("🚀 使用基于时间的分批迁移策略，避免 QuickWit offset 限制");

        // 使用安全的起始时间：从足够久远的过去开始，确保覆盖所有数据
        // 使用90天前作为起点，足以覆盖3个月的数据保留期
        let safe_start_time = Utc::now() - ChronoDuration::days(90);

        // 获取已迁移的记录数
        let mut total_migrated = self
            .get_index_document_count(target_index)
            .await
            .unwrap_or(0);

        info!(
            "目标索引当前 {} 条记录，开始时间窗口迁移（起始时间: {}）",
            total_migrated,
            safe_start_time.format("%Y-%m-%d %H:%M:%S")
        );

        // 按月分批迁移（每批处理30天的数据）
        let mut current_time = safe_start_time;
        // 添加24小时缓冲以覆盖可能的未来时间戳（测试数据或时钟偏差）
        let end_time = Utc::now() + ChronoDuration::hours(24);

        while current_time < end_time {
            let batch_end_time = current_time + ChronoDuration::days(30); // 每批30天
            let actual_end_time = std::cmp::min(batch_end_time, end_time);

            info!(
                "🗓️  迁移时间窗口: {} 到 {}",
                current_time.format("%Y-%m-%d %H:%M:%S"),
                actual_end_time.format("%Y-%m-%d %H:%M:%S")
            );

            // 迁移该时间窗口的数据
            match self
                .migrate_batch_with_time_window(
                    source_index,
                    target_index,
                    current_time,
                    actual_end_time,
                )
                .await
            {
                Ok((scanned_count, migrated_count, request_ids)) => {
                    if migrated_count == 0 && scanned_count == 0 {
                        info!("   该时间窗口没有数据，跳过");
                    } else {
                        total_migrated += migrated_count as u64;
                        status.last_scanned_count = scanned_count;
                        status.update_progress(migrated_count, total_migrated, request_ids.clone());

                        // 记录已迁移的 request_id
                        self.mark_batch_request_ids_migrated(&request_ids).await?;

                        info!(
                            "   ✅ 时间窗口迁移完成: 扫描 {} 条，迁移 {} 条，累计 {}/{} ({:.1}%)",
                            scanned_count,
                            migrated_count,
                            total_migrated,
                            source_total,
                            (total_migrated as f64 / source_total as f64) * 100.0
                        );
                    }

                    // 定期保存状态
                    if status.current_batch % 5 == 0 {
                        self.save_migration_status(&status).await?;
                    }

                    status.current_batch += 1;
                    current_time = actual_end_time;
                }
                Err(e) => {
                    let error_msg = format!(
                        "时间窗口迁移失败 ({} 到 {}): {}",
                        current_time.format("%Y-%m-%d %H:%M:%S"),
                        actual_end_time.format("%Y-%m-%d %H:%M:%S"),
                        e
                    );
                    error!("{}", error_msg);
                    status.set_error(error_msg);
                    self.save_migration_status(&status).await?;
                    return Err(e);
                }
            }
        }

        info!(
            "✅ 时间窗口迁移完成: 总计迁移 {} / {} 条文档",
            total_migrated, source_total
        );

        // 4. 最终完整性验证：检查每个 request_id 是否都已迁移
        info!("🔍 开始最终完整性验证（检查每个 request_id）...");
        let (is_complete, missing_ids) = self.verify_migration_completeness(source_index).await?;

        let final_target_count = self.get_index_document_count(target_index).await?;
        let final_source_count = self.get_index_document_count(source_index).await?;

        info!(
            "最终验证结果: 源索引 {} 条，目标索引 {} 条，完整性: {}",
            final_source_count,
            final_target_count,
            if is_complete { "通过" } else { "失败" }
        );

        // 5. 根据验证结果更新状态
        if is_complete && final_target_count >= final_source_count {
            info!("✅ 数据迁移验证成功（数量和明细都已验证）");
            status.migrated_count = final_target_count;
            status.source_total_count = final_source_count;
            status.mark_completed();
        } else {
            let error_msg = if !is_complete {
                format!(
                    "❌ 数据迁移不完整: 源 {} 条，目标 {} 条，发现 {} 条未迁移的数据",
                    final_source_count,
                    final_target_count,
                    missing_ids.len()
                )
            } else {
                format!(
                    "❌ 数据迁移数量不匹配: 源 {} 条，目标 {} 条，差异 {} 条",
                    final_source_count,
                    final_target_count,
                    final_source_count - final_target_count
                )
            };

            warn!("{}", error_msg);
            status.set_error(error_msg);

            // 如果有未迁移的数据，记录前 10 个 ID 用于调试
            if !missing_ids.is_empty() {
                let sample_ids: Vec<String> = missing_ids.iter().take(10).cloned().collect();
                warn!("未迁移的 request_id 示例: {:?}", sample_ids);
            }
        }

        // 6. 保存最终状态
        self.save_migration_status(&status).await?;

        if status.completed {
            info!("🎉 数据迁移全部完成！");
        } else {
            error!("❌ 数据迁移验证失败，这不应该发生！");
            error!("💡 可能原因：数据一致性问题、网络异常或索引损坏");
            error!("🔧 建议检查：源索引是否有新数据写入，或考虑重新开始迁移");
        }

        Ok(())
    }

    /// 迁移一批数据（使用 offset 跳过已迁移的数据）
    ///
    /// 返回：(迁移数量, request_id 列表)
    async fn migrate_batch_with_offset(
        &self,
        source_index: &str,
        target_index: &str,
        offset: usize,
        batch_size: usize,
    ) -> Result<(usize, usize, Vec<String>), AppError> {
        // QuickWit 的 start_offset 限制是 10,000
        const QUICKWIT_MAX_OFFSET: usize = 10_000;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        // 确保不超出 QuickWit 的 offset 限制
        let actual_offset = std::cmp::min(offset, QUICKWIT_MAX_OFFSET);

        let query = serde_json::json!({
            "query": "*",
            "max_hits": batch_size,
            "start_offset": actual_offset,
            "sort_by": "+request_start_time"
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::InternalServer(format!("查询源索引失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::InternalServer(format!(
                "查询源索引失败: HTTP {} - {}",
                status, body
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::InternalServer(format!("解析响应失败: {}", e)))?;

        let hits = result["hits"]
            .as_array()
            .ok_or_else(|| AppError::InternalServer("解析结果失败".to_string()))?;

        if hits.is_empty() {
            return Ok((0, 0, Vec::new()));
        }

        // 解析为 AgentLogEntry 记录
        use crate::models::AgentLogEntry;
        let mut records: Vec<AgentLogEntry> = Vec::new();
        let mut request_ids: Vec<String> = Vec::new();

        for hit in hits {
            // 尝试从不同位置获取数据
            let doc = hit.get("source").unwrap_or(hit);

            match serde_json::from_value::<AgentLogEntry>(doc.clone()) {
                Ok(mut record) => {
                    // 为旧数据添加 biz_type 字段，默认值为 "AGENT"
                    if record.biz_type.is_none() {
                        record.biz_type = Some("AGENT".to_string());
                    }
                    request_ids.push(record.request_id.clone());
                    records.push(record);
                }
                Err(e) => {
                    warn!("解析记录失败: {}, 数据: {:?}", e, doc);
                }
            }
        }

        if records.is_empty() {
            return Ok((hits.len(), 0, Vec::new()));
        }

        let mut filtered_records: Vec<AgentLogEntry> = Vec::with_capacity(records.len());
        let mut filtered_ids: Vec<String> = Vec::with_capacity(records.len());
        for (i, rec) in records.into_iter().enumerate() {
            let id = &request_ids[i];
            if !self.is_request_id_migrated(id).await {
                filtered_ids.push(id.clone());
                filtered_records.push(rec);
            }
        }

        if filtered_records.is_empty() {
            return Ok((hits.len(), 0, Vec::new()));
        }

        let hits_count = hits.len();
        let batch_count = filtered_records.len();

        // 写入目标索引
        let new_service = AgentLogQuickwitService::new_with_index_name(
            self.app_states.clone(),
            target_index.to_string(),
        );

        new_service
            .batch_ingest_agent_logs(&filtered_records)
            .await?;

        // 等待数据写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok((hits_count, batch_count, filtered_ids))
    }

    /// 检查索引是否存在
    async fn check_index_exists(&self, index_name: &str) -> bool {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/indexes/{}",
            self.app_states.config.url, index_name
        );

        match client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// 获取索引的文档数量（优化：不返回任何字段，只获取总数）
    async fn get_index_document_count(&self, index_name: &str) -> Result<u64, AppError> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, index_name
        );

        // 优化：max_hits=0 表示不返回文档，只获取总数
        let query = serde_json::json!({
            "query": "*",
            "max_hits": 0
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::InternalServer(format!("查询索引失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::InternalServer(format!(
                "查询索引文档数量失败: HTTP {} - {}",
                status, body
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::InternalServer(format!("解析响应失败: {}", e)))?;

        let count = result.get("num_hits").and_then(|v| v.as_u64()).unwrap_or(0);

        Ok(count)
    }

    /// 加载迁移状态
    async fn load_migration_status(&self) -> Result<MigrationStatus, AppError> {
        use crate::storage::tables;

        self.storage
            .load(tables::MIGRATION_STATUS, MIGRATION_KEY)
            .map_err(AppError::InternalServer)?
            .ok_or_else(|| AppError::InternalServer("状态不存在".to_string()))
    }

    /// 保存迁移状态
    async fn save_migration_status(&self, status: &MigrationStatus) -> Result<(), AppError> {
        use crate::storage::tables;

        self.storage
            .save(tables::MIGRATION_STATUS, MIGRATION_KEY, status)
            .map_err(AppError::InternalServer)?;

        info!("迁移状态已保存到数据库: {}", self.storage.db_path());
        Ok(())
    }

    /// 获取迁移状态（用于 API 查询）
    pub async fn get_status(&self) -> Option<MigrationStatus> {
        self.load_migration_status().await.ok()
    }

    /// 清理迁移状态（用于重新迁移）
    pub async fn reset_migration(&self) -> Result<(), AppError> {
        use crate::storage::tables;

        self.storage
            .delete(tables::MIGRATION_STATUS, MIGRATION_KEY)
            .map_err(AppError::InternalServer)?;

        info!("迁移状态已重置");
        Ok(())
    }

    /// 批量记录已迁移的 request_id（性能优化版本）
    ///
    /// 使用优化的批量标记接口，单次事务处理100个ID，大幅提升性能
    async fn mark_batch_request_ids_migrated(
        &self,
        request_ids: &[String],
    ) -> Result<(), AppError> {
        use crate::storage::tables;

        if request_ids.is_empty() {
            return Ok(());
        }

        // 使用优化的批量标记接口
        self.storage
            .mark_ids_batch(tables::MIGRATED_IDS, request_ids)
            .map_err(|e| AppError::InternalServer(format!("批量标记迁移ID失败: {}", e)))?;

        info!("✅ 已批量标记 {} 个 request_id 为已迁移", request_ids.len());
        Ok(())
    }

    /// 检查 request_id 是否已迁移
    async fn is_request_id_migrated(&self, request_id: &str) -> bool {
        use crate::storage::tables;

        self.storage
            .exists(tables::MIGRATED_IDS, request_id)
            .unwrap_or(false)
    }

    /// 验证迁移完整性：检查源索引的所有 request_id 是否都已迁移
    async fn verify_migration_completeness(
        &self,
        source_index: &str,
    ) -> Result<(bool, Vec<String>), AppError> {
        info!("开始验证迁移完整性...");

        // 从源索引获取所有 request_id（分批获取）
        let mut missing_ids = Vec::new();
        let batch_size = 1000;
        let mut offset = 0;

        loop {
            // 获取一批 request_id
            let request_ids = self
                .fetch_request_ids_from_index(source_index, offset, batch_size)
                .await?;

            if request_ids.is_empty() {
                break;
            }

            // 检查每个 ID 是否已迁移
            for request_id in &request_ids {
                if !self.is_request_id_migrated(request_id).await {
                    missing_ids.push(request_id.clone());
                }
            }

            offset += batch_size;

            // 如果这批数据少于批量大小，说明已经获取完所有数据
            if request_ids.len() < batch_size {
                break;
            }
        }

        let is_complete = missing_ids.is_empty();

        if is_complete {
            info!("✅ 迁移完整性验证通过：所有数据都已迁移");
        } else {
            warn!("⚠️ 发现 {} 条未迁移的数据", missing_ids.len());
        }

        Ok((is_complete, missing_ids))
    }

    /// 从索引获取 request_id 列表（只查询 request_id 字段，优化列存储性能）
    async fn fetch_request_ids_from_index(
        &self,
        index_name: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>, AppError> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, index_name
        );

        // 优化：只查询 request_id 字段，减少列存储的 I/O
        let query = serde_json::json!({
            "query": "*",
            "max_hits": limit,
            "start_offset": offset,
            "sort_by": "+request_start_time"
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::InternalServer(format!("查询索引失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::InternalServer(format!(
                "查询索引失败: HTTP {} - {}",
                status, body
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::InternalServer(format!("解析响应失败: {}", e)))?;

        let hits = result["hits"]
            .as_array()
            .ok_or_else(|| AppError::InternalServer("解析结果失败".to_string()))?;

        let request_ids: Vec<String> = hits
            .iter()
            .filter_map(|hit| {
                hit.get("source")
                    .and_then(|src| src.get("request_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(request_ids)
    }

    /// 使用时间戳方式获取下一批的开始时间戳（优化版）
    ///
    /// 优化策略：
    /// 1. 优先使用目标索引中最后一条记录的时间戳作为起点
    /// 2. 使用 1 小时的时间窗口，确保数据连续性
    /// 3. 如果目标索引查询失败，快速回退到源索引
    async fn get_next_batch_timestamp(
        &self,
        source_index: &str,
        offset: usize,
        _batch_size: usize,
    ) -> Result<DateTime<Utc>, AppError> {
        // 方法1：查询目标索引中已迁移的最新记录时间戳（最优策略）
        let client = reqwest::Client::new();
        let target_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url,
            crate::index_define::DEFAULT_AGENT_LOG_INDEX
        );

        // 查询目标索引中最新迁移的记录时间戳（用于获取迁移边界）
        let target_query = serde_json::json!({
            "query": "*",
            "max_hits": 1,
            "sort_by": "-request_start_time"
        });

        if let Ok(response) = client.post(&target_url).json(&target_query).send().await
            && response.status().is_success()
                && let Ok(result) = response.json::<serde_json::Value>().await
                    && let Some(hit) = result["hits"].as_array().and_then(|arr| arr.first()) {
                        // QuickWit v0.8+ 直接在 hits 数组中返回数据，没有 source 嵌套
                        let request_start_time = hit
                            .get("request_start_time")
                            .or_else(|| hit.get("source").and_then(|s| s.get("request_start_time")))
                            .and_then(|v| v.as_str());

                        if let Some(time_str) = request_start_time
                            && let Ok(parsed_time) = DateTime::parse_from_rfc3339(time_str) {
                                // 从目标索引最新记录的时间戳开始查询下一批
                                // 但是注意：目标索引中10,000条记录是按时间戳排序的，
                                // 需要找到实际对应的request_id边界，然后查询剩余记录
                                let next_timestamp = parsed_time.with_timezone(&Utc)
                                    + ChronoDuration::milliseconds(1);
                                info!("✅ 基于目标索引最新记录，时间戳起点: {}", next_timestamp);

                                // 额外优化：如果偏移量接近10,000，直接使用从源索引获取的精确时间戳
                                if offset >= 9990 {
                                    // 重新从源索引获取精确的边界时间戳
                                    let source_url = format!(
                                        "{}/api/v1/{}/search",
                                        self.app_states.config.url, source_index
                                    );
                                    let boundary_query = serde_json::json!({
                                        "query": "*",
                                        "max_hits": 1,
                                        "start_offset": offset,
                                        "sort_by": "+request_start_time"
                                    });
                                    if let Ok(response) =
                                        client.post(&source_url).json(&boundary_query).send().await
                                        && let Ok(result) =
                                            response.json::<serde_json::Value>().await
                                            && let Some(hit) = result["hits"]
                                                .as_array()
                                                .and_then(|arr| arr.first())
                                            {
                                                let boundary_time = hit
                                                    .get("request_start_time")
                                                    .or_else(|| {
                                                        hit.get("source").and_then(|s| {
                                                            s.get("request_start_time")
                                                        })
                                                    })
                                                    .and_then(|v| v.as_str());
                                                if let Some(boundary_str) = boundary_time
                                                    && let Ok(boundary_parsed) =
                                                        DateTime::parse_from_rfc3339(boundary_str)
                                                    {
                                                        let boundary_timestamp = boundary_parsed
                                                            .with_timezone(&Utc)
                                                            + ChronoDuration::milliseconds(1);
                                                        info!(
                                                            "✅ 使用源索引边界时间戳: {}",
                                                            boundary_timestamp
                                                        );
                                                        return Ok(boundary_timestamp);
                                                    }
                                            }
                                }

                                return Ok(next_timestamp);
                            }
                    }

        warn!("⚠️  目标索引查询失败，尝试从源索引获取时间戳...");

        // 方法2：从源索引获取时间戳（备选策略）
        let source_url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        // 优先查询 offset 位置的时间戳（支持 offset >= 10000 的情况）
        let source_query = serde_json::json!({
            "query": "*",
            "max_hits": 1,
            "start_offset": offset,
            "sort_by": "+request_start_time"
        });

        if let Ok(response) = client.post(&source_url).json(&source_query).send().await
            && response.status().is_success()
                && let Ok(result) = response.json::<serde_json::Value>().await
                    && let Some(hit) = result["hits"].as_array().and_then(|arr| arr.first()) {
                        // QuickWit v0.8+ 直接在 hits 数组中返回数据，没有 source 嵌套
                        let request_start_time = hit
                            .get("request_start_time")
                            .or_else(|| hit.get("source").and_then(|s| s.get("request_start_time")))
                            .and_then(|v| v.as_str());

                        if let Some(time_str) = request_start_time
                            && let Ok(parsed_time) = DateTime::parse_from_rfc3339(time_str) {
                                let timestamp = parsed_time.with_timezone(&Utc);
                                info!("✅ 基于源索引 offset {}，时间戳起点: {}", offset, timestamp);
                                return Ok(timestamp);
                            }
                    }

        // 方法3：获取源索引最早时间戳（最后的备选方案）
        let source_query = serde_json::json!({
            "query": "*",
            "max_hits": 1,
            "sort_by": "+request_start_time"
        });

        if let Ok(response) = client.post(&source_url).json(&source_query).send().await
            && response.status().is_success()
                && let Ok(result) = response.json::<serde_json::Value>().await
                    && let Some(hit) = result["hits"].as_array().and_then(|arr| arr.first()) {
                        // QuickWit v0.8+ 直接在 hits 数组中返回数据，没有 source 嵌套
                        let request_start_time = hit
                            .get("request_start_time")
                            .or_else(|| hit.get("source").and_then(|s| s.get("request_start_time")))
                            .and_then(|v| v.as_str());

                        if let Some(time_str) = request_start_time
                            && let Ok(parsed_time) = DateTime::parse_from_rfc3339(time_str) {
                                let base_time = parsed_time.with_timezone(&Utc);
                                // 基于已处理的记录数和时间窗口估算时间位置
                                let hours_offset = (offset.saturating_sub(10_000)) / 100;
                                let estimated_time =
                                    base_time + ChronoDuration::hours(hours_offset as i64);
                                info!(
                                    "⚠️  使用源索引估算，时间戳起点: {} (offset: {})",
                                    estimated_time, offset
                                );
                                return Ok(estimated_time);
                            }
                    }

        // 方法4：最后的备选方案，使用当前时间前推
        warn!("⚠️  无法确定精确时间位置，使用默认策略");
        let fallback_time = Utc::now() - ChronoDuration::hours(12); // 12小时前，更安全的起点
        info!("📍 使用备选时间戳起点: {}", fallback_time);
        Ok(fallback_time)
    }

    /// 基于时间窗口迁移数据（新策略，避免 offset 限制）
    ///
    /// # 参数
    /// * `source_index` - 源索引名称
    /// * `target_index` - 目标索引名称
    /// * `start_time` - 开始时间戳
    /// * `end_time` - 结束时间戳
    ///
    /// # 返回
    /// (扫描的记录数, 实际迁移的记录数, request_id列表)
    async fn migrate_batch_with_time_window(
        &mut self,
        source_index: &str,
        target_index: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<(usize, usize, Vec<String>), AppError> {
        info!(
            "🗓️ 自适应时间窗口迁移: {} 到 {}",
            start_time.format("%Y-%m-%d %H:%M:%S"),
            end_time.format("%Y-%m-%d %H:%M:%S")
        );

        // 首先查询该时间窗口内的数据条数（不计实际数据，只计num_hits）
        let count = self
            .count_window_documents(source_index, start_time, end_time)
            .await?;

        // 如果数据量超过10,000条，动态缩小时间窗口
        if count > 10000 {
            info!("   ⚠️  数据量 {} 条超过10,000条，开始缩小时间窗口", count);
            return self
                .migrate_with_reduced_window(source_index, target_index, start_time, end_time)
                .await;
        }

        // 数据量合适，执行迁移
        info!("   📊 数据量 {} 条，进行迁移", count);

        // 构建时间范围查询
        let search_filter = format!(
            "request_start_time:[{} TO {}]",
            start_time.format("%Y-%m-%dT%H:%M:%SZ"),
            end_time.format("%Y-%m-%dT%H:%M:%SZ")
        );

        // 构建 QuickWit 查询
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        let query = serde_json::json!({
            "query": search_filter,
            "max_hits": 10000, // 每次最多获取10000条
            "sort_by": "+request_start_time"
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::QuickWit(format!("时间窗口查询失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "时间窗口查询失败: HTTP {} - {}",
                status, body
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::QuickWit(format!("解析时间窗口响应失败: {}", e)))?;

        let hits = result["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("解析时间窗口结果失败".to_string()))?;

        if hits.is_empty() {
            return Ok((0, 0, Vec::new()));
        }

        info!("   发现 {} 条记录在该时间窗口", hits.len());

        // 解析为 AgentLogEntry 记录
        use crate::models::AgentLogEntry;
        let mut records: Vec<AgentLogEntry> = Vec::new();
        let mut request_ids: Vec<String> = Vec::new();

        for hit in hits {
            // QuickWit v0.8+ 可能直接返回数据或嵌套在 source 下
            let doc = hit.get("source").unwrap_or(hit);

            match serde_json::from_value::<AgentLogEntry>(doc.clone()) {
                Ok(mut record) => {
                    // 为旧数据添加 biz_type 字段，默认值为 "AGENT"
                    if record.biz_type.is_none() {
                        record.biz_type = Some("AGENT".to_string());
                    }
                    request_ids.push(record.request_id.clone());
                    records.push(record);
                }
                Err(e) => {
                    warn!("解析记录失败: {}, 数据: {:?}", e, doc);
                }
            }
        }

        if records.is_empty() {
            return Ok((hits.len(), 0, Vec::new()));
        }

        // 过滤已迁移的记录
        let mut filtered_records: Vec<AgentLogEntry> = Vec::with_capacity(records.len());
        let mut filtered_ids: Vec<String> = Vec::with_capacity(records.len());
        for (i, rec) in records.into_iter().enumerate() {
            let id = &request_ids[i];
            if !self.is_request_id_migrated(id).await {
                filtered_ids.push(id.clone());
                filtered_records.push(rec);
            }
        }

        if filtered_records.is_empty() {
            info!("   时间窗口内的所有记录都已迁移，跳过");
            return Ok((hits.len(), 0, filtered_ids));
        }

        // 写入目标索引
        let new_service = AgentLogQuickwitService::new_with_index_name(
            self.app_states.clone(),
            target_index.to_string(),
        );

        new_service
            .batch_ingest_agent_logs(&filtered_records)
            .await?;

        // 等待数据写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let migrated_count = filtered_ids.len() as u64;
        self.processed_count
            .fetch_add(migrated_count, Ordering::SeqCst);

        info!(
            "   ✅ 时间窗口迁移完成: {}/{} 条记录",
            migrated_count,
            request_ids.len()
        );

        Ok((hits.len(), migrated_count as usize, filtered_ids))
    }

    /// 使用时间戳方式迁移一批数据（优化版，避免数据遗漏）
    async fn migrate_batch_with_timestamp(
        &mut self,
        source_index: &str,
        target_index: &str,
        start_timestamp: DateTime<Utc>,
        batch_size: usize,
    ) -> Result<(usize, usize, Vec<String>), AppError> {
        info!(
            "使用优化时间戳分页迁移数据，开始时间戳: {}",
            start_timestamp
        );

        // 使用较小的时间窗口（1小时），避免数据遗漏
        let time_window = ChronoDuration::hours(1);
        let end_timestamp = start_timestamp + time_window;

        // 构建 QuickWit REST API 查询
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        // 使用 ">" 查询，确保从指定时间戳之后开始，避免数据重叠或遗漏
        // QuickWit时间戳查询语法：request_start_time:[2025-11-26T07:00:00Z TO ...]
        let search_filter = format!(
            "request_start_time:[{} TO {}]",
            start_timestamp.format("%Y-%m-%dT%H:%M:%SZ"),
            end_timestamp.format("%Y-%m-%dT%H:%M:%SZ")
        );

        let query = serde_json::json!({
            "query": search_filter,
            "max_hits": batch_size,
            "sort_by": "+request_start_time"
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::QuickWit(format!("时间戳查询请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::QuickWit(format!(
                "时间戳查询失败: HTTP {} - {}",
                status, body
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::QuickWit(format!("解析时间戳响应失败: {}", e)))?;

        let hits = result["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("解析时间戳结果失败".to_string()))?;

        // 如果这个时间段没有数据，快速跳过到下一个时间段
        if hits.is_empty() {
            info!(
                "时间段 {} - {} 没有数据，跳过",
                start_timestamp.format("%Y-%m-%d %H:%M"),
                end_timestamp.format("%Y-%m-%d %H:%M")
            );
            return Ok((0, 0, Vec::new()));
        }

        // 记录这批数据的时间戳范围
        let mut actual_start_time = start_timestamp;
        let mut actual_end_time = end_timestamp;
        if let (Some(first_hit), Some(last_hit)) = (hits.first(), hits.last())
            && let (Some(first_time), Some(last_time)) = (
                first_hit["source"]["request_start_time"].as_str(),
                last_hit["source"]["request_start_time"].as_str(),
            ) {
                actual_start_time = DateTime::parse_from_rfc3339(first_time)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(start_timestamp);
                actual_end_time = DateTime::parse_from_rfc3339(last_time)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(end_timestamp);
                info!(
                    "时间戳分页范围: {} 到 {} (共 {} 条记录)",
                    first_time,
                    last_time,
                    hits.len()
                );
            }

        // 解析为 AgentLogEntry 记录
        use crate::models::AgentLogEntry;
        let mut records: Vec<AgentLogEntry> = Vec::new();
        let mut request_ids: Vec<String> = Vec::new();

        for hit in hits {
            // QuickWit v0.8+ 可能直接返回数据或嵌套在 source 下
            let doc = hit.get("source").unwrap_or(hit);

            match serde_json::from_value::<AgentLogEntry>(doc.clone()) {
                Ok(mut record) => {
                    // 为旧数据添加 biz_type 字段，默认值为 "AGENT"
                    if record.biz_type.is_none() {
                        record.biz_type = Some("AGENT".to_string());
                    }
                    request_ids.push(record.request_id.clone());
                    records.push(record);
                }
                Err(e) => {
                    warn!("解析记录失败: {}, 数据: {:?}", e, doc);
                }
            }
        }

        if records.is_empty() {
            return Ok((hits.len(), 0, Vec::new()));
        }

        // 过滤已迁移的记录
        let mut filtered_records: Vec<AgentLogEntry> = Vec::with_capacity(records.len());
        let mut filtered_ids: Vec<String> = Vec::with_capacity(records.len());
        for (i, rec) in records.into_iter().enumerate() {
            let id = &request_ids[i];
            if !self.is_request_id_migrated(id).await {
                filtered_ids.push(id.clone());
                filtered_records.push(rec);
            }
        }
        let _filtered_count = request_ids.len() - filtered_records.len();

        if filtered_records.is_empty() {
            info!(
                "时间段 {} - {} 的所有记录都已迁移，跳过",
                actual_start_time.format("%Y-%m-%d %H:%M"),
                actual_end_time.format("%Y-%m-%d %H:%M")
            );
            return Ok((hits.len(), 0, filtered_ids));
        }

        // 写入目标索引（使用与现有方法相同的方式）
        let new_service = AgentLogQuickwitService::new_with_index_name(
            self.app_states.clone(),
            target_index.to_string(),
        );

        new_service
            .batch_ingest_agent_logs(&filtered_records)
            .await?;

        // 等待数据写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let migrated_count = filtered_ids.len() as u64;
        self.processed_count
            .fetch_add(migrated_count, Ordering::SeqCst);

        info!(
            "时间段 {} - {} 迁移完成: {} / {} 条记录",
            actual_start_time.format("%Y-%m-%d %H:%M"),
            actual_end_time.format("%Y-%m-%d %H:%M"),
            migrated_count,
            request_ids.len()
        );

        Ok((hits.len(), migrated_count as usize, filtered_ids))
    }

    /// 获取实际数据的起始时间戳（而不是固定3个月前）
    async fn get_actual_start_time(&self, source_index: &str) -> Result<DateTime<Utc>, AppError> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        // 查询最早记录
        let query = serde_json::json!({
            "query": "*",
            "max_hits": 1,
            "sort_by": "+request_start_time"
        });

        if let Ok(response) = client.post(&url).json(&query).send().await
            && response.status().is_success()
                && let Ok(result) = response.json::<serde_json::Value>().await
                    && let Some(hit) = result["hits"].as_array().and_then(|arr| arr.first())
                        && let Some(request_start_time) = hit
                            .get("request_start_time")
                            .or_else(|| hit.get("source").and_then(|s| s.get("request_start_time")))
                            .and_then(|v| v.as_str())
                            && let Ok(parsed_time) =
                                DateTime::parse_from_rfc3339(request_start_time)
                            {
                                return Ok(parsed_time.with_timezone(&Utc));
                            }

        // 如果查询失败，使用默认时间
        warn!("无法获取源索引起始时间，使用默认值");
        Ok(Utc::now() - ChronoDuration::days(7)) // 默认7天前
    }

    /// 统计时间窗口内的文档数量（优化查询，只获取num_hits）
    async fn count_window_documents(
        &self,
        source_index: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<usize, AppError> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        let search_filter = format!(
            "request_start_time:[{} TO {}]",
            start_time.format("%Y-%m-%dT%H:%M:%SZ"),
            end_time.format("%Y-%m-%dT%H:%M:%SZ")
        );

        let query = serde_json::json!({
            "query": search_filter,
            "max_hits": 0 // 只获取数量，不返回实际数据
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::QuickWit(format!("时间窗口计数查询失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::QuickWit("时间窗口计数查询失败".to_string()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::QuickWit(format!("解析计数响应失败: {}", e)))?;

        let count = result.get("num_hits").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        Ok(count)
    }

    /// 自适应时间窗口迁移（动态缩小窗口大小）
    async fn migrate_with_reduced_window(
        &mut self,
        source_index: &str,
        target_index: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<(usize, usize, Vec<String>), AppError> {
        let current_start = start_time;
        let mut total_scanned = 0;
        let mut total_migrated = 0;
        let mut all_request_ids = Vec::new();

        // 渐进式缩小时间窗口
        let window_steps = vec![
            (30, "30天"), // 第一步：30天
            (7, "7天"),   // 第二步：7天
            (1, "1天"),   // 第三步：1天
            (6, "6小时"), // 第四步：6小时
            (1, "1小时"), // 第五步：1小时
        ];

        for (hours, step_name) in window_steps {
            let step_duration = ChronoDuration::hours(hours);
            let mut step_start = current_start;

            while step_start < end_time {
                let step_end = std::cmp::min(step_start + step_duration, end_time);

                // 重新计数这个更小窗口的数据量
                let count = self
                    .count_window_documents(source_index, step_start, step_end)
                    .await?;

                if count == 0 {
                    info!(
                        "   ⏭️  {} 窗口 {} 到 {} 无数据，跳过",
                        step_name,
                        step_start.format("%H:%M"),
                        step_end.format("%H:%M")
                    );
                    step_start = step_end;
                    continue;
                }

                if count > 10000 {
                    info!("   ⚠️  {} 窗口仍有 {} 条数据，继续缩小", step_name, count);
                    break; // 继续缩小窗口
                }

                info!("   ✅ {} 窗口 {} 条数据，执行迁移", step_name, count);

                // 执行该时间窗口的迁移
                let (scanned, migrated, request_ids) = self
                    .migrate_single_window(source_index, target_index, step_start, step_end)
                    .await?;

                total_scanned += scanned;
                total_migrated += migrated;
                all_request_ids.extend(request_ids);

                info!(
                    "   📊 {} 窗口完成: 迁移 {}/{} 条",
                    step_name, migrated, scanned
                );

                step_start = step_end;
            }

            // 如果当前步长下所有小窗口都完成了，检查是否还需要继续
            if step_start >= end_time {
                break;
            }
        }

        Ok((total_scanned, total_migrated, all_request_ids))
    }

    /// 迁移单个时间窗口的数据
    async fn migrate_single_window(
        &mut self,
        source_index: &str,
        target_index: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<(usize, usize, Vec<String>), AppError> {
        // 构建时间范围查询
        let search_filter = format!(
            "request_start_time:[{} TO {}]",
            start_time.format("%Y-%m-%dT%H:%M:%SZ"),
            end_time.format("%Y-%m-%dT%H:%M:%SZ")
        );

        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/{}/search",
            self.app_states.config.url, source_index
        );

        let query = serde_json::json!({
            "query": search_filter,
            "max_hits": 10000,
            "sort_by": "+request_start_time"
        });

        let response = client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| AppError::QuickWit(format!("时间窗口查询失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::QuickWit("时间窗口查询失败".to_string()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::QuickWit(format!("解析时间窗口响应失败: {}", e)))?;

        let hits = result["hits"]
            .as_array()
            .ok_or_else(|| AppError::QuickWit("解析时间窗口结果失败".to_string()))?;

        if hits.is_empty() {
            return Ok((0, 0, Vec::new()));
        }

        // 解析为 AgentLogEntry 记录
        use crate::models::AgentLogEntry;
        let mut records: Vec<AgentLogEntry> = Vec::new();
        let mut request_ids: Vec<String> = Vec::new();

        for hit in hits {
            // QuickWit v0.8+ 可能直接返回数据或嵌套在 source 下
            let doc = hit.get("source").unwrap_or(hit);

            match serde_json::from_value::<AgentLogEntry>(doc.clone()) {
                Ok(mut record) => {
                    if record.biz_type.is_none() {
                        record.biz_type = Some("AGENT".to_string());
                    }
                    request_ids.push(record.request_id.clone());
                    records.push(record);
                }
                Err(e) => {
                    warn!("解析记录失败: {}, 数据: {:?}", e, doc);
                }
            }
        }

        if records.is_empty() {
            return Ok((hits.len(), 0, Vec::new()));
        }

        // 过滤已迁移的记录
        let mut filtered_records: Vec<AgentLogEntry> = Vec::with_capacity(records.len());
        let mut filtered_ids: Vec<String> = Vec::with_capacity(records.len());
        for (i, rec) in records.into_iter().enumerate() {
            let id = &request_ids[i];
            if !self.is_request_id_migrated(id).await {
                filtered_ids.push(id.clone());
                filtered_records.push(rec);
            }
        }

        if filtered_records.is_empty() {
            return Ok((hits.len(), 0, filtered_ids));
        }

        // 写入目标索引
        let new_service = AgentLogQuickwitService::new_with_index_name(
            self.app_states.clone(),
            target_index.to_string(),
        );

        new_service
            .batch_ingest_agent_logs(&filtered_records)
            .await?;

        // 等待数据写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let migrated_count = filtered_ids.len() as u64;
        self.processed_count
            .fetch_add(migrated_count, Ordering::SeqCst);

        Ok((hits.len(), migrated_count as usize, filtered_ids))
    }
}
