# 知识库全文检索设计文档

## 1. 概述

本文档描述了如何利用QuickWit实现知识库文档分段文本内容的全文检索功能，支持多知识库搜索、中文分词以及高效的查询性能。

## 2. 数据来源

从MySQL数据库的 `knowledge_raw_segment` 表获取文档分段数据，主要关注以下字段：
- `id`: 分段ID (主键)
- `raw_id`: 所属原始分段ID,前端手动新增的没有归属分段内容
- `kb_id`: 知识库ID
- `doc_id`: 文档ID
- `raw_txt`: 原始文本内容 (全文检索核心)
- `sort_index`: 排序索引
- `tenant_id`: 租户ID
- `space_id`: 空间ID
- `created`: 创建时间
- `modified`: 修改时间

## 3. QuickWit索引设计

### 3.1 索引基本信息

```yaml
version: 0.7
index_id: "knowledge_segments_v1"
doc_mapping:
  mode: dynamic
  timestamp_field: created
  partition_key: tenant_id  # 租户级分片，数据按租户ID物理隔离，提升多租户查询性能
  tag_fields: ["kb_id", "doc_id", "raw_id", "tenant_id", "space_id"]
  store_source: true
  field_mappings:
    - name: id
      type: u64
      fast: true
    
    - name: raw_id
      type: u64
      fast: true
    
    - name: kb_id
      type: u64
      fast: true
    
    - name: doc_id
      type: u64
      fast: true
    
    - name: tenant_id
      type: i64
      fast: true
    
    - name: space_id
      type: i64
      fast: true
    
    - name: raw_txt
      type: text
      tokenizer: chinese_compatible
      record: position
      fieldnorms: true
    
    - name: sort_index
      type: i64
      fast: true
    
    - name: created
      type: datetime
      input_formats: [rfc3339]
      fast: true
      fast_precision: seconds

search_settings:
  default_search_fields: ["raw_txt"]

indexing_settings:
  commit_timeout_secs: 60
  split_num_docs_target: 1000000
  merge_policy:
    type: "stable_log"
    min_level_num_docs: 100000
    merge_factor: 10
    max_merge_factor: 12
    maturation_period: 48h

# 无数据保留策略，数据永久保存
# retention:
#   period: 1 year
#   schedule: daily
```

### 3.2 字段说明

| 字段名 | 类型 | 配置说明 | 用途 |
|--------|------|----------|------|
| `id` | u64 | 快速字段(fast: true) | 分段唯一标识 |
| `raw_id` | u64 | 快速字段(fast: true) | 原始分段ID关联 |
| `kb_id` | u64 | 快速字段(fast: true) | 知识库过滤查询 |
| `doc_id` | u64 | 快速字段(fast: true) | 文档级别过滤 |
| `tenant_id` | i64 | 快速字段(fast: true) | 租户隔离 |
| `space_id` | i64 | 快速字段(fast: true) | 空间过滤 |
| `raw_txt` | text | chinese_compatible分词器 + position记录 | 全文检索核心字段 |
| `sort_index` | i64 | 快速字段(fast: true) | 结果排序 |
| `created` | datetime | 快速字段 + 时间戳字段 | 时间范围查询和分片 |

### 3.3 中文分词配置说明

1. **tokenizer**: 使用 `chinese_compatible` 分词器，支持中英文混合内容
2. **record**: 设置为 `position`，支持短语查询
3. **fieldnorms**: 启用字段长度归一化，提高搜索相关性评分

## 4. 实现方案

### 4.1 数据同步机制

```rust
// 知识库文档分段模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRawSegment {
    pub id: u64,
    pub raw_id: Option<u64>, // 所属原始分段ID,前端手动新增的没有归属分段内容
    pub kb_id: u64,
    pub doc_id: u64,
    pub raw_txt: String,
    pub sort_index: i64,
    pub tenant_id: i64,
    pub space_id: i64,
    pub created: DateTime<Utc>,
}

// 搜索参数模型
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeSearchRequest {
    pub query: String,                          // 搜索查询
    pub kb_ids: Option<Vec<u64>>,              // 知识库ID列表，可选
    pub doc_ids: Option<Vec<u64>>,             // 文档ID列表，可选
    pub raw_ids: Option<Vec<u64>>,             // 原始分段ID列表，可选
    pub tenant_id: i64,                        // 租户ID（必填）
    pub space_id: Option<i64>,                 // 空间ID
    pub offset: Option<usize>,                 // 分页偏移
    pub limit: Option<usize>,                  // 结果数量限制
    pub sort_by: Option<String>,               // 排序字段
    pub sort_order: Option<String>,            // 排序方向 asc/desc
}
```

### 4.2 索引创建和管理

```rust
// 创建知识库索引
pub async fn create_knowledge_index() -> Result<(), Box<dyn std::error::Error>> {
    let index_config = include_str!("knowledge_index_config.yaml");
    let quickwit_client = QuickwitClient::new("http://localhost:7280");
    quickwit_client.create_index(index_config).await?;
    Ok(())
}

// 批量索引文档数据
pub async fn index_knowledge_segments(
    quickwit_client: &QuickwitClient,
    segments: Vec<KnowledgeRawSegment>
) -> Result<u64, Error> {
    let batch_size = 1000;
    let mut total_indexed = 0;
    
    // 分批处理
    for chunk in segments.chunks(batch_size) {
        quickwit_client.index_documents(chunk.to_vec()).await?;
        total_indexed += chunk.len();
    }
    
    Ok(total_indexed as u64)
}
```

### 4.3 搜索接口实现

```rust
// 全文搜索实现
pub async fn search_knowledge_segments(
    quickwit_client: &QuickwitClient,
    request: KnowledgeSearchRequest
) -> Result<Vec<KnowledgeSearchResult>, Error> {
    let mut query_builder = quickwit_client.search_query();
    
    // 基础查询
    query_builder.set_query(&request.query);
    
    // 构建过滤条件
    let mut filters = Vec::new();
    
    if let Some(kb_ids) = &request.kb_ids {
        if !kb_ids.is_empty() {
            let kb_filter = kb_ids.iter()
                .map(|id| format!("kb_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filters.push(format!("({})", kb_filter));
        }
    }
    
    if let Some(doc_ids) = &request.doc_ids {
        if !doc_ids.is_empty() {
            let doc_filter = doc_ids.iter()
                .map(|id| format!("doc_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filters.push(format!("({})", doc_filter));
        }
    }
    
    if let Some(raw_ids) = &request.raw_ids {
        if !raw_ids.is_empty() {
            let raw_filter = raw_ids.iter()
                .map(|id| format!("raw_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filters.push(format!("({})", raw_filter));
        }
    }
    
    // 租户ID是必填字段
    filters.push(format!("tenant_id:{}", request.tenant_id));
    
    if let Some(space_id) = request.space_id {
        filters.push(format!("space_id:{}", space_id));
    }
    
    // 应用过滤条件
    if !filters.is_empty() {
        let filter_query = filters.join(" AND ");
        query_builder.set_filter(&filter_query);
    }
    
    // 设置分页
    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(20).min(100); // 限制最大100条
    
    query_builder.set_start(offset as u64);
    query_builder.set_limit(limit as u64);
    
    // 设置排序
    let sort_field = request.sort_by.unwrap_or_else(|| "created".to_string());
    let sort_order = request.sort_order.unwrap_or_else(|| "desc".to_string());
    query_builder.set_sort(&format!("{}:{}", sort_field, sort_order));
    
    // 执行搜索
    let results = query_builder.execute().await?;
    
    // 转换结果
    let search_results: Vec<KnowledgeSearchResult> = results.hits
        .into_iter()
        .map(|hit| {
            let segment: KnowledgeRawSegment = serde_json::from_value(hit.document)
                .unwrap_or_default();
            
            KnowledgeSearchResult {
                id: segment.id,
                raw_id: segment.raw_id,
                kb_id: segment.kb_id,
                doc_id: segment.doc_id,
                raw_txt: segment.raw_txt,
                highlight: hit.highlights.get("raw_txt").cloned(),
                score: hit.score,
                sort_index: segment.sort_index,
                created: segment.created,
            }
        })
        .collect();
    
    Ok(search_results)
}
```

## 5. API设计

**重要安全要求**：
- 除全量清空数据接口外，**所有HTTP接口都必须包含tenant_id参数**
- tenant_id用于确保租户数据隔离，防止跨租户访问
- 所有查询、推送、删除、更新操作都需要验证tenant_id与请求数据的租户归属一致性

### 5.1 搜索API

```rust
#[get("/knowledge/search")]
pub async fn api_search_knowledge(
    Query(request): Query<KnowledgeSearchRequest>,
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<Vec<KnowledgeSearchResult>>>> {
    let results = search_knowledge_segments(&quickwit_client, request).await?;
    Ok(Json(ApiResponse::success(results)))
}
```

### 5.2 数据推送API

```rust
#[post("/knowledge/push")]
pub async fn api_push_knowledge_segments(
    Json(request): Json<PushRequest>,
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<PushResult>>> {
    // 验证租户权限
    let segments = request.segments.into_iter()
        .filter(|segment| segment.tenant_id == request.tenant_id)
        .collect();
    
    let count = index_knowledge_segments(&quickwit_client, segments).await?;
    
    Ok(Json(ApiResponse::success(PushResult {
        indexed_count: count,
        push_time: Utc::now().to_rfc3339()
    })))
}
```

### 5.3 数据删除API

```rust
#[post("/knowledge/delete")]
pub async fn api_delete_knowledge_segments(
    Query(params): Query<DeleteParams>,
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<DeleteResult>>> {
    // 安全检查：如果没有任何删除参数，必须指定kb_id
    let has_kb_id = params.kb_id.as_ref().map_or(false, |ids| !ids.is_empty());
    let has_doc_id = params.doc_id.as_ref().map_or(false, |ids| !ids.is_empty());
    let has_raw_ids = params.raw_ids.as_ref().map_or(false, |ids| !ids.is_empty());
    let has_space_id = params.space_id.as_ref().map_or(false, |ids| !ids.is_empty());
    
    // 租户ID是必填参数，不计入条件检查
    let total_params = vec![has_kb_id, has_doc_id, has_raw_ids, has_space_id]
        .iter()
        .filter(|&&x| x)
        .count();
    
    if total_params == 0 {
        return Err(Error::BadRequest("除tenant_id外，必须指定至少一个删除条件".to_string()));
    }
    
    // 如果只提供一个参数，则必须为kb_id
    if total_params == 1 && !has_kb_id {
        return Err(Error::BadRequest("如果只提供单个参数，则必须指定kb_id".to_string()));
    }
    
    // 构建删除过滤条件
    let mut filter_conditions = Vec::new();
    
    // 如果没有指定任何其他参数，但指定了kb_id，允许删除
    if let Some(kb_id) = params.kb_id {
        if kb_id.is_empty() {
            return Err(Error::BadRequest("kb_id不能为空列表".to_string()));
        }
        let kb_filter = kb_id.iter()
            .map(|id| format!("kb_id:{}", id))
            .collect::<Vec<_>>()
            .join(" OR ");
        filter_conditions.push(format!("({})", kb_filter));
    }
    
    if let Some(doc_ids) = params.doc_id {
        if !doc_ids.is_empty() {
            let doc_filter = doc_ids.iter()
                .map(|id| format!("doc_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filter_conditions.push(format!("({})", doc_filter));
        }
    }
    
    if let Some(raw_ids) = params.raw_ids {
        if !raw_ids.is_empty() {
            let raw_filter = raw_ids.iter()
                .map(|id| format!("raw_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filter_conditions.push(format!("({})", raw_filter));
        }
    }
    
    // 租户ID是必填字段
    filter_conditions.push(format!("tenant_id:{}", params.tenant_id));
    
    if let Some(space_ids) = params.space_id {
        if !space_ids.is_empty() {
            let space_filter = space_ids.iter()
                .map(|id| format!("space_id:{}", id))
                .collect::<Vec<_>>()
                .join(" OR ");
            filter_conditions.push(format!("({})", space_filter));
        }
    }
    
    let filter = filter_conditions.join(" AND ");
    
    let delete_count = quickwit_client.delete_by_filter("knowledge_segments_v1", &filter).await?;
    
    Ok(Json(ApiResponse::success(DeleteResult {
        deleted_count: delete_count,
        delete_time: Utc::now().to_rfc3339()
    })))
}
```

### 5.4 全量清空数据API

```rust
#[post("/knowledge/clear")]
pub async fn api_clear_all_knowledge_segments(
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<ClearResult>>> {
    // 危险操作：全量清空数据
    log::warn!("执行全量清空知识库数据操作，时间: {}", Utc::now());
    
    // 先统计即将删除的数据量
    let mut count_query = quickwit_client.search_query();
    count_query.set_limit(0); // 只获取总数，不获取文档
    
    let total_count = match count_query.execute().await {
        Ok(result) => result.num_hits,
        Err(e) => return Err(Error::InternalError(format!("获取数据总数失败: {}", e)))
    };
    
    // 执行全量删除
    let delete_filter = "*"; // 匹配所有文档
    let deleted_count = match quickwit_client.delete_by_filter("knowledge_segments_v1", delete_filter).await {
        Ok(count) => count,
        Err(e) => return Err(Error::InternalError(format!("全量删除失败: {}", e)))
    };
    
    log::info!("全量清空知识库数据完成，删除数量: {}", deleted_count);
    
    Ok(Json(ApiResponse::success(ClearResult {
        total_count_before: total_count,
        deleted_count: deleted_count,
        clear_time: Utc::now().to_rfc3339()
    })))
}
```

### 5.4 统计数据API

```rust
#[post("/knowledge/stats")]
pub async fn api_get_knowledge_stats(
    Json(params): Json<StatsParams>,
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<KnowledgeStatsResult>>> {
    let mut query_builder = quickwit_client.search_query();
    
    // 构建过滤条件
    let mut filters = Vec::new();
    filters.push(format!("tenant_id:{}", params.tenant_id));
    
    if let Some(kb_id) = params.kb_id {
        filters.push(format!("kb_id:{}", kb_id));
    }
    
    if let Some(space_id) = params.space_id {
        filters.push(format!("space_id:{}", space_id));
    }
    
    let filter = filters.join(" AND ");
    query_builder.set_filter(&filter);
    query_builder.set_limit(0); // 只获取总数，不获取文档
    
    // 执行查询获取总分段数
    let total_segments = match query_builder.execute().await {
        Ok(result) => result.num_hits,
        Err(e) => return Err(Error::InternalError(format!("获取分段总数失败: {}", e)))
    };
    
    // 使用聚合查询统计每个文档的分段数
    let mut agg_query = quickwit_client.search_query();
    agg_query.set_filter(&filter);
    agg_query.set_limit(0);
    
    // 添加文档ID聚合
    agg_query.add_aggregation("doc_stats", "terms", "doc_id", Some(1000));
    
    let agg_result = match agg_query.execute().await {
        Ok(result) => result,
        Err(e) => return Err(Error::InternalError(format!("获取文档统计失败: {}", e)))
    };
    
    // 解析聚合结果
    let doc_stats = if let Some(doc_agg) = agg_result.aggregations.get("doc_stats") {
        match doc_agg {
            serde_json::Value::Object(buckets) => {
                if let Some(buckets_array) = buckets.get("buckets") {
                    if let Some(buckets) = buckets_array.as_array() {
                        buckets.iter().map(|bucket| {
                            let doc_id = bucket.get("key").and_then(|k| k.as_u64()).unwrap_or(0);
                            let segment_count = bucket.get("doc_count").and_then(|c| c.as_u64()).unwrap_or(0);
                            DocumentStats {
                                doc_id,
                                segment_count
                            }
                        }).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            },
            _ => Vec::new()
        }
    } else {
        Vec::new()
    };
    
    let doc_count = doc_stats.len() as u64;
    
    Ok(Json(ApiResponse::success(KnowledgeStatsResult {
        tenant_id: params.tenant_id,
        kb_id: params.kb_id,
        space_id: params.space_id,
        doc_count,
        total_segments,
        doc_stats,
        stats_time: Utc::now().to_rfc3339()
    })))
}
```

### 5.5 文本修改API

```rust
#[post("/knowledge/update")]
pub async fn api_update_knowledge_segment(
    Json(request): Json<UpdateRequest>,
    Extension(quickwit_client): Extension<Arc<QuickwitClient>>
) -> Result<Json<ApiResponse<UpdateResult>>> {
    // 更新知识库分段的文本内容，实现方式：先删除旧数据，再插入新数据
    
    // 验证必要字段
    if request.id.is_none() && request.raw_id.is_none() {
        return Err(Error::BadRequest("必须指定id或raw_id".to_string()));
    }
    
    if request.raw_txt.is_none() {
        return Err(Error::BadRequest("必须提供raw_txt更新内容".to_string()));
    }
    
    // 构建删除过滤条件
    let mut delete_filter = Vec::new();
    
    if let Some(id) = request.id {
        delete_filter.push(format!("id:{}", id));
    }
    
    if let Some(raw_id) = request.raw_id {
        delete_filter.push(format!("raw_id:{}", raw_id));
    }
    
    // 租户ID是必填字段
    delete_filter.push(format!("tenant_id:{}", request.tenant_id));
    
    if let Some(space_id) = request.space_id {
        delete_filter.push(format!("space_id:{}", space_id));
    }
    
    let filter = delete_filter.join(" AND ");
    
    // 1. 查询现有数据
    let mut search_query = quickwit_client.search_query();
    search_query.set_filter(&filter);
    search_query.set_limit(1000); // 限制查询数量
    
    let existing_docs = search_query.execute().await?.hits;
    
    if existing_docs.is_empty() {
        return Err(Error::NotFound("未找到要更新的数据".to_string()));
    }
    
    // 2. 构建新文档列表
    let mut new_docs = Vec::new();
    for hit in existing_docs {
        let mut segment: KnowledgeRawSegment = serde_json::from_value(hit.document)
            .map_err(|e| Error::InternalError(format!("解析数据失败: {}", e)))?;
        
        // 更新文本内容
        segment.raw_txt = request.raw_txt.clone().unwrap();
        
        // 更新创建时间
        segment.created = Utc::now();
        
        new_docs.push(segment);
    }
    
    // 3. 删除旧数据
    quickwit_client.delete_by_filter("knowledge_segments_v1", &filter).await?;
    
    // 4. 插入新数据
    quickwit_client.index_documents(new_docs).await?;
    
    Ok(Json(ApiResponse::success(UpdateResult {
        updated_count: existing_docs.len() as u64,
        update_time: Utc::now().to_rfc3339()
    })))
}
```

### 5.6 相关数据模型

```rust
// 统计请求模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsParams {
    pub tenant_id: i64,                      // 租户ID（必填）
    pub kb_id: Option<u64>,                 // 知识库ID（可选）
    pub space_id: Option<i64>,                // 空间ID（可选）
}

// 文档统计模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStats {
    pub doc_id: u64,                        // 文档ID
    pub segment_count: u64,                   // 分段数量
}

// 统计结果模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStatsResult {
    pub tenant_id: i64,                      // 租户ID
    pub kb_id: Option<u64>,                 // 知识库ID
    pub space_id: Option<i64>,                // 空间ID
    pub doc_count: u64,                      // 文档总数
    pub total_segments: u64,                  // 分段总数
    pub doc_stats: Vec<DocumentStats>,         // 每个文档的分段统计
    pub stats_time: String,                   // 统计时间
}

// 推送请求模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    pub tenant_id: i64,                      // 租户ID（必填）
    pub segments: Vec<KnowledgeRawSegment>,    // 知识库分段列表
}

// 推送结果模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    pub indexed_count: u64,                  // 索引的文档数量
    pub push_time: String,                   // 推送时间
}

// 更新请求模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    pub id: Option<u64>,                     // 分段ID (与raw_id二选一)
    pub raw_id: Option<u64>,                 // 原始分段ID (与id二选一)
    pub raw_txt: Option<String>,              // 新的文本内容
    pub tenant_id: i64,                      // 租户ID (必填)
    pub space_id: Option<i64>,               // 空间ID (可选，安全检查)
}

// 删除请求模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteParams {
    pub kb_id: Option<Vec<u64>>,             // 知识库ID列表(如果只提供这一个参数，不能为空)
    pub doc_id: Option<Vec<u64>>,            // 文档ID列表，支持批量删除
    pub raw_ids: Option<Vec<u64>>,           // 原始分段ID列表
    pub tenant_id: i64,                       // 租户ID（必填）
    pub space_id: Option<Vec<i64>>,           // 空间ID列表，支持批量删除
}

// 更新结果模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub updated_count: u64,                  // 更新的文档数量
    pub update_time: String,                 // 更新时间
}

// 删除结果模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub deleted_count: u64,                  // 删除的文档数量
    pub delete_time: String,                 // 删除时间
}

// 全量清空结果模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearResult {
    pub total_count_before: u64,             // 清空前的总文档数
    pub deleted_count: u64,                  // 实际删除的文档数
    pub clear_time: String,                  // 清空操作时间
}
```

## 6. 性能优化

### 6.1 索引优化策略

1. **分片策略**：基于`created`字段进行时间分片，提高查询效率
2. **合并策略**：使用`stable_log`合并策略，平衡写入放大和查询性能
3. **快速字段**：将所有过滤字段设置为`fast`字段，支持高效过滤

### 6.2 查询优化

1. **缓存热门查询**：对频繁查询的关键词实现结果缓存
2. **批量同步**：采用批量同步机制，减少索引构建时间
3. **分页控制**：限制单次查询结果数量，避免大结果集

## 7. 扩展计划

### 7.1 短期扩展

1. **语义搜索**：集成向量模型，实现语义搜索功能
2. **结果高亮**：优化搜索结果高亮显示
3. **搜索建议**：实现搜索词自动补全和建议

### 7.2 长期扩展

1. **多语言支持**：扩展支持其他语言的分词器
2. **个性化搜索**：基于用户行为的个性化搜索结果排序
3. **知识图谱**：结合知识图谱增强搜索结果

## 8. 数据管理策略

### 8.1 数据保留
- 数据默认永久保存，不设置自动过期策略
- 通过接口推送数据到QuickWit进行索引
- 通过统计接口检查数据完整性，排查缺失数据
- 通过手动调用API接口删除不再需要的数据
- 删除操作基于过滤条件，支持按知识库、文档、分段、租户和空间级别删除

### 8.2 数据管理示例

#### 数据删除示例
```bash
# 删除指定知识库的所有数据
curl -X POST "http://your-api/knowledge/delete?kb_id=123"

# 删除多个知识库的数据
curl -X POST "http://your-api/knowledge/delete?kb_id=123&kb_id=456"

# 删除指定文档的所有数据
curl -X POST "http://your-api/knowledge/delete?kb_id=123&doc_id=456"

# 删除多个文档的数据
curl -X POST "http://your-api/knowledge/delete?kb_id=123&doc_id=456&doc_id=457"

# 删除指定的多个分段数据
curl -X POST "http://your-api/knowledge/delete?kb_id=123&raw_ids=100,200,300"

# 删除指定租户下的所有数据
curl -X POST "http://your-api/knowledge/delete?tenant_id=1001&kb_id=123"

# 删除多个租户下的数据
curl -X POST "http://your-api/knowledge/delete?tenant_id=1001&kb_id=123"
# 注意：每个请求只能指定一个tenant_id，因为它是必填参数且不支持数组

# 删除多个空间下的数据
curl -X POST "http://your-api/knowledge/delete?space_id=201&space_id=202"

# 错误示例：没有提供任何删除条件（会报错）
curl -X POST "http://your-api/knowledge/delete"

# 错误示例：只提供租户ID但没有知识库ID（会报错）
curl -X POST "http://your-api/knowledge/delete?tenant_id=1001"
# 错误原因：tenant_id是必填参数，但除tenant_id外还需要提供至少一个其他参数
```

#### 数据统计示例
```bash
# 获取指定租户下所有知识库的统计信息
curl -X POST "http://your-api/knowledge/stats" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": 1001
  }'

# 获取指定知识库的统计信息
curl -X POST "http://your-api/knowledge/stats" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": 1001,
    "kb_id": 123
  }'

# 获取指定空间下知识库的统计信息
curl -X POST "http://your-api/knowledge/stats" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": 1001,
    "space_id": 201,
    "kb_id": 123
  }'
```

#### 数据推送示例
```bash
# 推送知识库分段数据
curl -X POST "http://your-api/knowledge/push" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": 1001,
    "segments": [
      {
        "id": 1,
        "raw_id": 100,
        "kb_id": 123,
        "doc_id": 456,
        "raw_txt": "这是第一个知识库分段的文本内容",
        "sort_index": 1,
        "tenant_id": 1001,
        "space_id": 201,
        "created": "2023-10-01T12:00:00Z"
      },
      {
        "id": 2,
        "raw_id": 101,
        "kb_id": 123,
        "doc_id": 456,
        "raw_txt": "这是第二个知识库分段的文本内容",
        "sort_index": 2,
        "tenant_id": 1001,
        "space_id": 201,
        "created": "2023-10-01T12:05:00Z"
      }
    ]
  }'
```

#### 全量清空示例
```bash
# 全量清空所有知识库数据（危险操作，无需参数）
curl -X POST "http://your-api/knowledge/clear"

# 注意：此操作会删除所有租户、所有知识库的所有数据
# 建议在生产环境中添加额外的安全认证
```

#### 文本更新示例
```bash
# 更新指定ID的文本内容
curl -X POST "http://your-api/knowledge/update" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 123,
    "raw_txt": "这是更新后的文本内容"
  }'

# 更新指定raw_id的文本内容，必须提供租户ID
curl -X POST "http://your-api/knowledge/update" \
  -H "Content-Type: application/json" \
  -d '{
    "raw_id": 456,
    "raw_txt": "这是更新后的文本内容",
    "tenant_id": 1001
  }'
```

## 9. 部署建议

1. **索引配置**：按照3.1节提供的YAML配置创建索引
2. **资源分配**：根据数据量分配适当的内存和CPU资源
3. **监控指标**：设置查询延迟、索引大小和同步状态的监控
4. **备份策略**：配置QuickWit索引的定期备份机制
5. **访问控制**：为删除API添加适当的认证和权限控制，防止误操作

## 10. API接口总览

本设计方案提供以下HTTP接口，全部基于GET和POST方法：

| 接口 | 方法 | 路径 | 功能 | 安全限制 |
|------|------|------|------|
| 搜索知识库 | GET | `/knowledge/search` | 全文检索知识库内容 | - |
| 数据统计 | POST | `/knowledge/stats` | 获取知识库文档和分段统计信息 | tenant_id为必填参数 |
| 推送数据 | POST | `/knowledge/push` | 推送知识库分段数据到QuickWit | tenant_id为必填参数 |
| 删除数据 | POST | `/knowledge/delete` | 根据条件删除知识库数据 | tenant_id为必填参数，除tenant_id外必须提供至少一个参数，如果只提供一个参数则必须为kb_id，其他参数支持数组形式实现批量删除 |
| 全量清空 | POST | `/knowledge/clear` | 清空所有知识库数据 | 需要特殊权限认证 |
| 更新文本 | POST | `/knowledge/update` | 更新知识库分段的文本内容 | tenant_id为必填参数 |

## 11. 数据安全设计

### 11.1 租户隔离安全机制

1. **tenant_id强制要求**：
   - 除全量清空数据接口外，**所有HTTP接口都必须包含tenant_id参数**
   - 系统会验证请求中的tenant_id与数据归属的租户ID一致性
   - 任何跨租户访问尝试都会被拒绝

2. **删除操作安全机制**：
   - 删除接口要求tenant_id为必填参数，除tenant_id外至少提供一个参数，如果只提供单个参数则必须为`kb_id`
   - 验证tenant_id与待删除数据的租户归属一致性
3. **全量清空保护**：单独提供`/knowledge/clear`接口用于全量清空，与常规删除接口分离
4. **审计日志**：所有操作记录详细的审计日志，包括tenant_id、操作类型、数据范围
5. **权限控制**：建议为全量清空接口添加更严格的权限验证

### 11.2 操作权限建议

1. **常规删除**：仅限有数据管理权限的用户访问
2. **全量清空**：仅限系统管理员在维护窗口期间操作
3. **批量删除**：建议添加二次确认机制

## 12. 总结

本设计方案利用QuickWit的高性能全文搜索能力，针对知识库文档分段内容实现了高效的中文全文检索功能。通过合理的索引设计和API实现，可以支持：
- 跨多知识库的统一搜索
- 精确的知识库/文档/分段级别过滤
- 租户/空间级别的数据隔离
- 高性能的中文分词检索
- 可扩展的架构设计
- 永久数据保存策略
- 灵活的手动数据删除机制
- 基于删除-插入模式的文本更新机制
- 仅使用GET和POST方法的简化接口设计
- 安全的删除操作保护机制
- 完整的数据统计与核对功能，支持排查缺失数据

该方案可以直接集成到现有日志平台中，提供强大的知识库检索能力，并通过API接口实现精细化的数据管理和数据完整性检查。
