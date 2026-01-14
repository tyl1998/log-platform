use crate::impl_searchable_params;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 知识库文档分段结构
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KnowledgeRawSegment {
    /// 分段ID（可选，如果不提供则自动生成 UUID v7）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 原始分段ID（必填，对应 MySQL 表 knowledge_raw_segment 的主键 id）
    pub raw_id: u64,
    /// 知识库ID（必填）
    pub kb_id: u64,
    /// 文档ID（必填）
    pub doc_id: u64,
    /// 原始文本内容（全文检索核心字段）
    pub raw_txt: String,
    /// 排序索引，在归属同一个文档下，段的排序
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    /// 租户ID（必填，用于多租户隔离）
    pub tenant_id: i64,
    /// 空间ID
    pub space_id: i64,
    /// 创建时间（时间戳字段）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,
}

/// 知识库搜索请求参数
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KnowledgeSearchParams {
    /// 搜索关键词（全文检索）
    pub query: String,
    /// 知识库ID列表（OR关系），可选
    pub kb_ids: Option<Vec<u64>>,
    /// 文档ID列表（OR关系），可选
    pub doc_ids: Option<Vec<u64>>,
    /// 原始分段ID列表（OR关系），可选
    pub raw_ids: Option<Vec<u64>>,
    /// 租户ID（必填）
    pub tenant_id: i64,
    /// 空间ID（可选）
    pub space_id: Option<i64>,
    /// 分页偏移量
    pub offset: Option<u64>,
    /// 返回数量限制
    pub limit: Option<u64>,
    /// 排序字段
    pub sort_by: Option<String>,
    /// 排序方向（asc/desc）
    pub sort_order: Option<String>,
}

/// 知识库搜索结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KnowledgeSearchResult {
    /// 搜索结果列表
    pub results: Vec<KnowledgeSearchHit>,
    /// 总匹配数量
    pub total: i64,
    /// 处理耗时（毫秒）
    pub elapsed_time_ms: i64,
    /// 当前页码
    pub current: i64,
    /// 每页大小
    pub page_size: i64,
}

/// 知识库搜索命中项
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KnowledgeSearchHit {
    /// 分段ID（Quickwit 文档 ID）
    pub id: Option<String>, // 可选：某些搜索场景可能不返回文档ID
    /// 原始分段ID（MySQL 主键）
    pub raw_id: Option<u64>, // 可选：某些聚合搜索可能不包含具体ID
    /// 知识库ID
    pub kb_id: Option<u64>, // 可选：跨知识库搜索时可能为空
    /// 文档ID
    pub doc_id: Option<u64>, // 可选：某些搜索可能不包含文档信息
    /// 原始文本
    pub raw_txt: Option<String>, // 可选：某些搜索可能只返回元数据，不返回内容
    /// 排序索引
    pub sort_index: Option<i64>, // 可选：不是所有知识库都需要排序
    /// 租户ID
    pub tenant_id: Option<i64>, // 可选：某些系统可能不使用多租户
    /// 空间ID
    pub space_id: Option<i64>, // 可选：某些知识库可能不区分空间
    /// 创建时间
    pub created: Option<DateTime<Utc>>, // 可选：时间戳字段在某些搜索中可能缺失
    /// 搜索得分
    pub score: Option<f32>,
    /// 高亮文本
    pub highlight: Option<String>,
}

/// 统计数据参数
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StatsParams {
    /// 租户ID（必填）
    pub tenant_id: i64,
    /// 知识库ID（可选）
    pub kb_id: Option<u64>,
    /// 空间ID（可选）
    pub space_id: Option<i64>,
}

/// 文档统计信息
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DocumentStats {
    /// 文档ID
    pub doc_id: u64,
    /// 分段数量
    pub segment_count: u64,
}

/// 知识库统计结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KnowledgeStatsResult {
    /// 租户ID
    pub tenant_id: i64,
    /// 知识库ID
    pub kb_id: Option<u64>,
    /// 空间ID
    pub space_id: Option<i64>,
    /// 文档总数
    pub doc_count: u64,
    /// 分段总数
    pub total_segments: u64,
    /// 每个文档的分段统计
    pub doc_stats: Vec<DocumentStats>,
    /// 统计时间
    pub stats_time: String,
}

/// 知识库数据推送请求
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PushRequest {
    /// 分段数据列表（每个分段包含 tenant_id）
    pub segments: Vec<KnowledgeRawSegment>,
}

/// 知识库数据推送结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PushResult {
    /// 成功索引的文档数量
    pub indexed_count: u64,
    /// 推送时间
    pub push_time: String,
    /// 成功的 raw_id 列表（用于客户端确认）
    pub success_raw_ids: Option<Vec<u64>>,
}

/// 知识库文本更新请求
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateRequest {
    /// 分段ID（可选，与 raw_id 配合使用）
    pub id: Option<String>,
    /// 原始分段ID（必填，用于定位要更新的分段）
    pub raw_id: String,
    /// 新的文本内容（必填）
    pub raw_txt: String,
    /// 租户ID（必填）
    pub tenant_id: i64,
    /// 空间ID（可选）
    pub space_id: Option<i64>,
}

/// 知识库文本更新结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateResult {
    /// 更新的文档数量
    pub updated_count: u64,
    /// 更新时间
    pub update_time: String,
}

/// 知识库数据删除参数
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeleteParams {
    /// 知识库 ID 列表（如果只提供这一个参数，不能为空）
    pub kb_id: Option<Vec<i64>>,
    /// 文档ID列表（支持批量删除）
    pub doc_id: Option<Vec<i64>>,
    /// 原始分段ID列表
    pub raw_ids: Option<Vec<i64>>,
    /// 租户ID（必填）
    pub tenant_id: i64,
    /// 空间ID列表（支持批量删除）
    pub space_id: Option<Vec<i64>>,
    /// 开始时间戳（可选，Unix秒级时间戳，用于时间范围删除）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_timestamp: Option<i64>,
    /// 结束时间戳（可选，Unix秒级时间戳，用于时间范围删除）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_timestamp: Option<i64>,
}

/// 知识库数据删除结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeleteResult {
    /// 删除的文档数量
    pub deleted_count: u64,
    /// 删除时间
    pub delete_time: String,
}

/// 异步删除任务创建结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AsyncDeleteResult {
    /// 删除任务ID
    pub task_id: String,
    /// 预计删除的文档数量
    pub estimated_delete_count: u64,
    /// 任务创建时间
    pub created_time: String,
    /// 任务状态（通常是 "pending"）
    pub status: String,
    /// 删除查询条件
    pub query: String,
}

/// 知识库全量清空结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ClearResult {
    /// 清空前的总文档数
    pub total_count_before: u64,
    /// 实际删除的文档数
    pub deleted_count: u64,
    /// 清空操作时间
    pub clear_time: String,
}

// 使用宏实现搜索参数构建
impl_searchable_params!(
    KnowledgeSearchParams,
    scalar_fields: [tenant_id],
    array_fields: [
        kb_ids => " OR ",
        doc_ids => " OR ",
        raw_ids => " OR "
    ]
);

impl Default for KnowledgeSearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            kb_ids: None,
            doc_ids: None,
            raw_ids: None,
            tenant_id: 0,
            space_id: None,
            offset: None,
            limit: Some(20),
            sort_by: Some("created".to_string()),
            sort_order: Some("desc".to_string()),
        }
    }
}

impl KnowledgeSearchResult {
    /// 创建新的搜索结果
    pub fn new(results: Vec<KnowledgeSearchHit>, total: i64, elapsed_time_ms: i64) -> Self {
        Self {
            results,
            total,
            elapsed_time_ms,
            current: 1,
            page_size: 20,
        }
    }

    /// 设置分页信息
    pub fn with_pagination(mut self, current: i64, page_size: i64) -> Self {
        self.current = current;
        self.page_size = page_size;
        self
    }
}

/// 查询分段ID列表请求参数
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SegmentIdsParams {
    /// 租户ID（必填，多租户隔离）
    pub tenant_id: i64,
    /// 知识库ID（必填）
    pub kb_id: u64,
    /// 空间ID（可选，额外过滤条件）
    pub space_id: Option<i64>,
}

/// 查询分段ID列表响应结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SegmentIdsResult {
    /// 租户ID
    pub tenant_id: i64,
    /// 知识库ID
    pub kb_id: u64,
    /// 空间ID
    pub space_id: Option<i64>,
    /// 总分段数量
    pub total_count: u64,
    /// 分段ID列表（raw_id）
    pub segment_ids: Vec<u64>,
    /// 查询时间
    pub query_time: String,
}

/// 删除任务信息
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeleteTask {
    /// 删除任务ID
    pub task_id: String,
    /// 任务状态: pending, running, success, failed
    pub status: String,
    /// 删除查询条件
    pub query: String,
    /// 任务创建时间（时间戳）
    pub created_at: u64,
    /// 任务开始执行时间（时间戳，可选）
    pub started_at: Option<u64>,
    /// 任务结束时间（时间戳，可选）
    pub ended_at: Option<u64>,
    /// 实际删除的文档数量（可选）
    pub num_deleted_docs: Option<u64>,
    /// 错误信息（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// 删除任务简化状态信息
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeleteTaskStatus {
    /// 删除任务ID
    pub task_id: String,
    /// 任务状态: pending, running, success, failed
    pub status: String,
    /// 实际删除的文档数量（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_deleted_docs: Option<u64>,
    /// 错误信息（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
