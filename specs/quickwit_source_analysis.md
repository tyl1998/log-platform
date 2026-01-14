# QuickWit 源码分析和使用指南

## 1. 项目架构概述

QuickWit 是基于 Rust 构建的云原生搜索引擎，专为云存储优化。主要架构特点：

### 1.1 核心模块
```
quickwit/
├── quickwit-cli              # CLI工具
├── quickwit-serve            # REST API服务
├── quickwit-search           # 搜索服务
├── quickwit-indexing         # 索引服务
├── quickwit-ingest           # 数据摄取
├── quickwit-doc-mapper       # 文档映射和分词
├── quickwit-query            # 查询解析
├── quickwit-config           # 配置管理
├── quickwit-metastore        # 元数据存储
└── quickwit-storage          # 存储抽象层
```

### 1.2 核心技术栈
- **搜索引擎**: 基于 Tantivy (Rust 版 Lucene)
- **HTTP框架**: Warp (轻量级、异步)
- **云存储**: S3、Azure Blob、GCS 等
- **分布式**: 无状态设计，计算与存储分离

## 2. REST API 核心实现

### 2.1 API 目录结构
```
quickwit-serve/src/
├── index_api/         # 索引管理 API
│   ├── rest_handler.rs
│   ├── index_resource.rs
│   └── source_resource.rs
├── search_api/        # 搜索 API
│   ├── rest_handler.rs
│   └── grpc_adapter.rs
├── ingest_api/        # 数据摄取 API
│   └── rest_handler.rs
└── elasticsearch_api/ # ES 兼容 API
```

### 2.2 关键 API 端点

#### 索引管理 API
```http
# 创建索引
POST /api/v1/indexes
Content-Type: application/json

# 获取索引列表
GET /api/v1/indexes

# 删除索引
DELETE /api/v1/indexes/{index_id}

# 清空索引
POST /api/v1/indexes/{index_id}/clear
```

#### 搜索 API
```http
# POST 搜索
POST /api/v1/{index_id}/search
Content-Type: application/json

# GET 搜索
GET /api/v1/{index_id}/search?query=...&max_hits=20
```

#### 数据摄取 API
```http
# 批量摄取 (NDJSON格式)
POST /api/v1/{index_id}/ingest?commit=force
Content-Type: application/json
```

## 3. 索引配置详解

### 3.1 索引配置结构
```rust
// quickwit-config/src/index_config.rs
pub struct IndexConfig {
    pub version: usize,
    pub index_id: String,
    pub doc_mapping: DocMapping,
    pub search_settings: Option<SearchSettings>,
    pub indexing_settings: Option<IndexingSettings>,
    pub retention: Option<RetentionPolicy>,
}
```

### 3.2 文档映射 (DocMapping)
```rust
pub struct DocMapping {
    pub mode: Mode,                    // dynamic, Lenient, Strict
    pub field_mappings: Vec<FieldMapping>,
    pub tag_fields: Vec<String>,       // 用于快速过滤的字段
    pub timestamp_field: Option<String>,
    pub partition_key: Option<String>, // ⭐ 分片键
    pub store_source: bool,
    pub max_num_partitions: usize,     // 默认 200
}
```

### 3.3 字段映射 (FieldMapping)
```rust
pub struct FieldMapping {
    pub name: String,
    pub mapping: FieldMappingEntry,
}

pub enum FieldMappingEntry {
    Text(TextFieldMappingEntry),
    I64(I64FieldMappingEntry),
    U64(U64FieldMappingEntry),
    F64(F64FieldMappingEntry),
    Datetime(DatetimeFieldMappingEntry),
    Bool(BoolFieldMappingEntry),
    Ip(IpFieldMappingEntry),
    Bytes(BytesFieldMappingEntry),
    Json(ObjectFieldMappingEntry),
}
```

### 3.4 文本字段映射详解
```rust
pub struct TextFieldMappingEntry {
    pub description: Option<String>,
    pub stored: bool,
    pub indexed: bool,
    pub tokenizer: String,        // 分词器名称
    pub record: Record,           // basic, freq, position
    pub fieldnorms: bool,         // 是否存储字段长度归一化
    pub fast: FastField,          // 是否为快速字段
}
```

## 4. 分词器 (Tokenizer) 系统

### 4.1 内置分词器
位置：`quickwit-query/src/tokenizers/`

#### 4.1.1 中文分词器 (chinese_compatible)
```rust
// 实现特点：
// - 中文字符按字符分割 (CJK字符范围：U+3400 到 U+9FFF)
// - 英文字母按空格分割
// - 保留字母数字组合
// - 过滤标点符号

char_grouping(c):
- Keep: 字母数字连续组合
- SplitKeep: CJK字符（每个字符作为一个token）
- SplitIgnore: 标点符号（忽略）
```

**测试示例**：
```text
输入: "Hello world, 你好世界, bonjour monde"

输出分词:
- "hello" (0-5)
- "world" (6-12) 
- "你" (13-16)  # CJK字符
- "好" (16-19)
- "世" (19-22)
- "界" (22-25)
- "bonjour" (25-32)
- "monde" (33-38)
```

#### 4.1.2 其他分词器
- `default`: 默认分词器，按空白和标点分割，转小写
- `raw`: 不分词，保留原文（类似ES的keyword类型）
- `raw_lowercase`: 不分词，但转小写
- `en_stem`: 英文词干提取
- `whitespace`: 仅按空白分割
- `lowercase`: 仅转小写，不分割

### 4.2 分词器配置使用
```yaml
field_mappings:
  - name: content
    type: text
    tokenizer: chinese_compatible  # 使用中文分词器
    record: position              # 记录词项位置（支持短语查询）
    fieldnorms: true              # 启用BM25评分
```

## 5. 分片 (Partition) 机制

### 5.1 分片键配置
在索引配置的 `doc_mapping` 中设置：
```yaml
doc_mapping:
  partition_key: tenant_id  # 按租户ID分片
  tag_fields: ["tenant_id", "kb_id", "doc_id"]
```

### 5.2 分片工作原理
- **路由**: 具有相同 `partition_key` 值的文档被路由到相同的分片
- **查询优化**: 当查询包含 `partition_key` 条件时，只搜索相关分片
- **限制**: 最大分片数默认为 200（可通过 `max_num_partitions` 配置）

### 5.3 源码位置
```rust
// quickwit-indexing/src/actors/doc_processor.rs
pub struct DocProcessor {
    partition_key: Option<String>,
    max_num_partitions: usize,
}

// 处理分片逻辑
fn compute_partition_id(doc: &Document, partition_key: &str) -> Option<u64> {
    // 根据partition_key字段值计算分片ID
}
```

## 6. 搜索 API 详解

### 6.1 搜索请求结构
```rust
// quickwit-serve/src/search_api/rest_handler.rs
#[derive(Debug, Default, Deserialize)]
pub struct SearchRequestQueryString {
    pub query: String,                    // 查询字符串
    pub search_fields: Option<Vec<String>>, // 指定搜索字段
    pub max_hits: u64,                    // 最大返回数量
    pub start_offset: u64,                // 偏移量（分页）
    pub start_timestamp: Option<i64>,     // 时间范围过滤
    pub end_timestamp: Option<i64>,
    pub sort_by: Option<String>,          // 排序字段
}
```

### 6.2 查询语法
支持 Tantivy 查询语法：
```text
# 简单搜索
hello

# 字段搜索
title:hello

# 布尔查询
hello AND world
hello OR world
NOT world

# 范围查询
age:[10 TO 20]
date:[2024-01-01 TO 2024-12-31]

# 通配符
name:hello*
name:?ello

# 正则表达式
name:/he.*o/

# 短语搜索
title:"hello world"
```

### 6.3 排序语法
```text
# 按字段降序（默认）
sort_by: created_at

# 按字段升序
sort_by: +created_at

# 按字段降序
sort_by: -created_at

# 多字段排序
sort_by: created_at,score

# 支持的字段类型
- fast: true 的数值字段
- datetime 字段
- text 字段（不支持分词字段）
```

## 7. 数据摄取 API 详解

### 7.1 摄取格式
支持 NDJSON 格式（每行一个JSON文档）：
```json
{"id": 1, "title": "文档1", "content": "这是第一个文档", "tenant_id": 1001}
{"id": 2, "title": "文档2", "content": "这是第二个文档", "tenant_id": 1001}
```

### 7.2 摄取选项
```rust
#[derive(Clone, Debug, Deserialize)]
struct IngestOptions {
    commit_type: CommitTypeV2,  // Auto, Force, WaitFor
    use_legacy_ingest: bool,    // 是否使用v1 API
    detailed_response: bool,    // 是否返回详细响应
}
```

### 7.3 摄取流程
```rust
// quickwit-serve/src/ingest_api/rest_handler.rs
async fn ingest(
    index_id: IndexId,
    body: Body,
    ingest_options: IngestOptions,
    ingest_router: IngestRouterServiceClient,
    ingest_service: IngestServiceClient,
) -> Result<IngestResponse, IngestServiceError> {
    // 1. 解析NDJSON格式数据
    // 2. 路由到对应分片（基于partition_key）
    // 3. 索引文档
    // 4. 提交（commit）
}
```

## 8. 聚合查询 (Aggregations)

### 8.1 聚合类型
支持多种聚合类型：
- `terms`: 词项统计
- `histogram`: 直方图
- `range`: 范围统计
- `date_histogram`: 时间直方图
- `avg`, `sum`, `min`, `max`: 数值统计

### 8.2 聚合查询示例
```json
{
  "query": "tenant_id:1001",
  "aggs": {
    "kb_stats": {
      "terms": {
        "field": "kb_id",
        "size": 1000
      }
    },
    "doc_count": {
      "cardinality": {
        "field": "doc_id"
      }
    }
  }
}
```

## 9. 性能优化建议

### 9.1 索引优化
1. **字段设置**：
   - 过滤字段设置 `fast: true`
   - 排序字段设置 `fast: true`
   - 文本字段启用 `fieldnorms: true` 支持评分

2. **分片策略**：
   - 使用 `partition_key` 按租户或业务维度分片
   - 避免过多分片（建议 < 200）

3. **合并策略**：
   ```yaml
   indexing_settings:
     split_num_docs_target: 1000000  # 每个分片目标文档数
     merge_policy:
       type: "stable_log"
       min_level_num_docs: 100000
       merge_factor: 10
       max_merge_factor: 12
   ```

### 9.2 查询优化
1. **使用过滤字段**：查询时优先使用 `fast` 字段过滤
2. **分页限制**：`max_hits` 不宜过大（建议 < 1000）
3. **时间过滤**：结合 `timestamp_field` 进行时间范围过滤
4. **分片过滤**：查询条件包含 `partition_key` 字段

## 10. 最佳实践

### 10.1 多租户场景
```yaml
# 知识库索引配置示例
version: 0.7
index_id: "knowledge_segments_v1"
doc_mapping:
  mode: dynamic
  timestamp_field: created
  partition_key: tenant_id  # 按租户分片
  tag_fields: ["tenant_id", "kb_id", "doc_id", "space_id"]
  store_source: true
  field_mappings:
    - name: id
      type: u64
      fast: true
    - name: tenant_id
      type: i64
      fast: true
    - name: kb_id
      type: u64
      fast: true
    - name: doc_id
      type: u64
      fast: true
    - name: space_id
      type: i64
      fast: true
    - name: raw_txt
      type: text
      tokenizer: chinese_compatible  # 中文分词
      record: position              # 支持短语查询
      fieldnorms: true              # BM25评分
    - name: created
      type: datetime
      fast: true
      fast_precision: seconds

search_settings:
  default_search_fields: ["raw_txt"]

indexing_settings:
  split_num_docs_target: 1000000
  merge_policy:
    type: "stable_log"
    min_level_num_docs: 100000
    merge_factor: 10
    maturation_period: 48h
```

### 10.2 中文搜索最佳实践
```rust
// 1. 使用中文分词器
field_mappings:
  - name: content
    type: text
    tokenizer: chinese_compatible
    record: position
    fieldnorms: true

// 2. 支持多种查询类型
// 关键词搜索: "机器学习"
// 短语搜索: "人工智能技术"
// 模糊搜索: "机器~" (支持编辑距离)
// 通配符搜索: "机器*" 

// 3. 组合查询
{
  "query": "(机器学习 OR 深度学习) AND AI技术",
  "search_fields": ["title", "content"]
}
```

### 10.3 安全和权限
```rust
// 1. 所有查询强制包含tenant_id
let query = format!("tenant_id:{} AND raw_txt:{}", tenant_id, keyword);

// 2. 验证参数
if tenant_id.is_empty() {
    return Err(AppError::BadRequest("tenant_id is required".to_string()));
}

// 3. 敏感操作审计
log::warn!("User {} performed dangerous operation clear_all for tenant {}", user_id, tenant_id);
```

## 11. 常见问题

### 11.1 分片相关
**Q: 如何控制分片数量？**
A: 设置 `max_num_partitions` 参数（默认200），避免过多租户导致分片爆炸。

**Q: 分片不均匀怎么办？**
A: 检查 `partition_key` 分布，选择更均匀的字段作为分片键。

### 11.2 中文搜索相关
**Q: 中文分词效果不理想？**
A: `chinese_compatible` 分词器按字符分割，适合短文本。对于长文本，可以考虑自定义分词器。

**Q: 如何支持自定义词典？**
A: QuickWit 暂不支持自定义词典，如需支持需要修改分词器源码。

### 11.3 性能相关
**Q: 查询慢怎么办？**
A: 检查：1) 是否使用 `fast` 字段过滤；2) 是否包含 `partition_key` 条件；3) 分片数量是否过多。

**Q: 索引速度慢？**
A: 调整 `commit_timeout_secs`、批量摄取、使用 `commit=force` 参数。

## 12. 与现有工程集成

### 12.1 复用现有架构
你的工程已实现完整的 agent_logs 模块，可直接复制模式：

```
src/
├── models/
│   ├── agent_logs_model.rs       → 参考 → knowledge_logs_model.rs
├── services/
│   ├── agent_log_quickwit.rs     → 参考 → knowledge_quickwit.rs
├── api/
│   ├── agent_logs.rs             → 参考 → knowledge_logs.rs
├── index_define/
│   ├── agent_index_define.rs     → 参考 → knowledge_index_define.rs
```

### 12.2 关键差异点
1. **分词器**: 使用 `chinese_compatible` 而非 `default`
2. **分片键**: 使用 `tenant_id` 而非其他字段
3. **查询语法**: 支持中文全文检索
4. **字段类型**: 适配知识库业务场景（kb_id, doc_id, space_id）

## 总结

QuickWit 提供了完整的 REST API 和强大的搜索功能，特别适合：
- 云原生场景
- 多租户应用
- 日志和文档搜索
- 中文全文检索

通过合理配置 `partition_key`、`分词器` 和 `fast` 字段，可以实现高性能的多租户全文检索系统。
