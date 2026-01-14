use utoipa::OpenApi;

/// 空响应数据（用于操作类接口）
#[derive(utoipa::ToSchema, serde::Serialize)]
pub struct EmptyData {}

/// 知识库分段数据模型
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeRawSegment {
    /// 唯一标识符（可选，不提供则自动生成 UUID v7）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 原始分段 ID（必填，对应 MySQL 主键）
    pub raw_id: u64,
    /// 知识库 ID
    pub kb_id: u64,
    /// 文档 ID
    pub doc_id: u64,
    /// 原始文本内容
    pub raw_txt: String,
    /// 排序索引
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    /// 租户 ID
    pub tenant_id: i64,
    /// 空间 ID
    pub space_id: i64,
    /// 创建时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<chrono::DateTime<chrono::Utc>>,
}

/// 知识库搜索请求参数
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSearchParams {
    /// 搜索关键词（全文检索）
    pub query: String,
    /// 知识库 ID 列表（OR 关系），可选
    pub kb_ids: Option<Vec<String>>,
    /// 文档 ID 列表（OR 关系），可选
    pub doc_ids: Option<Vec<String>>,
    /// 原始分段 ID 列表（OR 关系），可选
    pub raw_ids: Option<Vec<String>>,
    /// 租户 ID（必填）
    pub tenant_id: String,
    /// 空间 ID（可选）
    pub space_id: Option<String>,
    /// 分页偏移量
    pub offset: Option<u64>,
    /// 返回数量限制
    pub limit: Option<u64>,
    /// 排序字段
    pub sort_by: Option<String>,
    /// 排序方向（asc/desc）
    pub sort_order: Option<String>,
}

/// 知识库搜索命中结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSearchHit {
    /// 分段ID
    pub id: String,
    /// 原始分段ID（必填）
    pub raw_id: String,
    /// 知识库ID
    pub kb_id: String,
    /// 文档ID
    pub doc_id: String,
    /// 原始文本
    pub raw_txt: String,
    /// 排序索引
    pub sort_index: i64,
    /// 租户ID
    pub tenant_id: String,
    /// 空间ID
    pub space_id: String,
    /// 创建时间
    pub created: chrono::DateTime<chrono::Utc>,
    /// BM25 相关性评分
    pub score: Option<f32>,
    /// 高亮文本
    pub highlight: Option<String>,
}

/// 知识库搜索结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSearchResult {
    /// 搜索结果列表
    pub results: Vec<KnowledgeSearchHit>,
    /// 总匹配数量
    pub total: i64,
    /// 处理耗时（毫秒）
    pub took_ms: i64,
}

/// 知识库数据推送请求
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct PushRequest {
    /// 分段数据列表（每个分段包含 tenant_id）
    pub segments: Vec<KnowledgeRawSegment>,
}

/// 知识库数据推送结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct PushResult {
    /// 成功索引的文档数量
    pub indexed_count: u64,
    /// 推送时间
    pub push_time: String,
    /// 成功的 raw_id 列表（用于客户端确认）
    pub success_raw_ids: Option<Vec<String>>,
}

/// 知识库数据删除参数
#[derive(utoipa::ToSchema, Clone, serde::Serialize)]
pub struct DeleteParams {
    /// 知识库 ID 列表（如果只提供这一个参数，不能为空）
    pub kb_id: Option<Vec<i64>>,
    /// 文档 ID 列表（支持批量删除）
    pub doc_id: Option<Vec<i64>>,
    /// 原始分段 ID 列表
    pub raw_ids: Option<Vec<i64>>,
    /// 租户 ID（必填）
    pub tenant_id: i64,
    /// 空间 ID 列表（支持批量删除）
    pub space_id: Option<Vec<i64>>,
}

/// 知识库数据删除结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeleteResult {
    /// 删除的文档数量
    pub deleted_count: u64,
    /// 删除时间
    pub delete_time: String,
}

/// 知识库文本更新请求
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateRequest {
    /// 分段 ID（可选）
    pub id: Option<u64>,
    /// 原始分段 ID（必填，用于定位要更新的分段）
    pub raw_id: String,
    /// 新的文本内容（必填）
    pub raw_txt: String,
    /// 租户 ID（必填）
    pub tenant_id: String,
    /// 空间 ID（可选）
    pub space_id: Option<String>,
}

/// 知识库文本更新结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateResult {
    /// 更新的文档数量
    pub updated_count: u64,
    /// 更新时间
    pub update_time: String,
}

/// 知识库全量清空结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClearResult {
    /// 清空前的总文档数
    pub total_count_before: u64,
    /// 实际删除的文档数
    pub deleted_count: u64,
    /// 清空操作时间
    pub clear_time: String,
}

/// 知识库文档统计信息
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentStats {
    /// 文档 ID
    pub doc_id: String,
    /// 文档名称
    pub doc_name: Option<String>,
    /// 分段总数
    pub segment_count: u64,
}

/// 知识库统计结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeStatsResult {
    /// 租户 ID
    pub tenant_id: String,
    /// 知识库 ID
    pub kb_id: Option<String>,
    /// 空间 ID
    pub space_id: Option<String>,
    /// 文档总数
    pub doc_count: u64,
    /// 分段总数
    pub total_segments: u64,
    /// 文档统计信息列表
    pub doc_stats: Vec<DocumentStats>,
    /// 统计时间
    pub stats_time: String,
}

/// 知识库数据统计参数
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsParams {
    /// 租户 ID（必填）
    pub tenant_id: String,
    /// 知识库 ID（可选）
    pub kb_id: Option<String>,
    /// 空间 ID（可选）
    pub space_id: Option<String>,
}

/// 查询分段ID列表请求参数
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct SegmentIdsParams {
    /// 租户ID（必填，多租户隔离）
    pub tenant_id: String,
    /// 知识库ID（必填）
    pub kb_id: String,
    /// 空间ID（可选，额外过滤条件）
    pub space_id: Option<String>,
}

/// 查询分段ID列表响应结果
#[derive(utoipa::ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct SegmentIdsResult {
    /// 租户ID
    pub tenant_id: String,
    /// 知识库ID
    pub kb_id: String,
    /// 空间ID
    pub space_id: Option<String>,
    /// 总分段数量
    pub total_count: u64,
    /// 分段ID列表（raw_id）
    pub segment_ids: Vec<String>,
    /// 查询时间
    pub query_time: String,
}

/// OpenAPI 文档定义
#[derive(OpenApi)]
#[openapi(
    paths(
        // 基础路由
        crate::api::record_common_logs::health_check,
        // 知识库路由
        crate::api::knowledge_logs::knowledge_create_index,
        crate::api::knowledge_logs::knowledge_search_logs,
        crate::api::knowledge_logs::knowledge_push_segments,
        crate::api::knowledge_logs::knowledge_delete_segments,
        crate::api::knowledge_logs::knowledge_update_segment,
        crate::api::knowledge_logs::knowledge_clear_all_segments,
        crate::api::knowledge_logs::knowledge_get_stats,
        crate::api::knowledge_logs::knowledge_query_segment_ids,
        // Agent 日志路由
        crate::api::agent_logs::agent_ingest_log,
        crate::api::agent_logs::agent_batch_ingest_logs,
        crate::api::agent_logs::agent_search_logs,
        crate::api::agent_logs::agent_query_detail_log,
        crate::api::agent_logs::agent_create_index,
        crate::api::agent_logs::agent_create_v2_index,
        crate::api::agent_logs::agent_migrate_data,
        crate::api::agent_logs::agent_delete_logs,
        // 通用日志路由
        crate::api::record_common_logs::ingest_log,
        crate::api::record_common_logs::batch_ingest_logs,
        crate::api::record_common_logs::search_logs,
    ),
    components(
        schemas(
            // 通用响应
            EmptyData,
            // 知识库相关
            KnowledgeRawSegment,
            KnowledgeSearchParams,
            KnowledgeSearchHit,
            KnowledgeSearchResult,
            PushRequest,
            PushResult,
            DeleteParams,
            DeleteResult,
            UpdateRequest,
            UpdateResult,
            ClearResult,
            DocumentStats,
            KnowledgeStatsResult,
            StatsParams,
            SegmentIdsParams,
            SegmentIdsResult,
        )
    ),
    tags(
        (name = "健康检查", description = "系统健康检查相关接口"),
        (name = "知识库", description = "知识库全文检索相关接口"),
        (name = "智能体日志", description = "智能体日志管理相关接口"),
        (name = "通用日志", description = "通用日志管理相关接口")
    )
)]
pub struct ApiDoc;
