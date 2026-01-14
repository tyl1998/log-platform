# 知识库全文检索开发任务列表

## 1. 项目初始化和环境准备

### 1.1 依赖管理
- [✅] quickwit客户端依赖已存在（reqwest）
- [✅] 已有基础依赖：serde、chrono、tracing、axum、tower等
- [ ] 确认是否需要添加新的依赖（如需要）

### 1.2 目录结构设计
- [ ] 创建知识库模块目录结构：
  - [ ] `src/knowledge/` - 知识库检索主模块
  - [ ] `src/knowledge/mod.rs` - 模块入口
  - [ ] `✅ src/index_define/` - 已存在，可添加知识库索引定义
  - [ ] `✅ src/models/` - 已存在，可添加知识库数据模型
  - [ ] `✅ src/services/` - 已存在，可添加知识库服务
  - [ ] `✅ src/api/` - 已存在，可添加知识库API路由

**参考现有架构**：
- 现有 agent_logs 模块结构完整，可作为参考
- 知识库模块应复用相同的架构模式

### 1.3 配置管理
- [✅] QuickWit配置已存在 (src/config/mod.rs)
- [✅] AppStates结构已存在 (src/models/app_state_model.rs)
- [ ] 可复用现有配置，无需额外配置

## 2. 数据模型定义 (src/models/)

### 2.1 核心数据结构
**参考现有模型**：`src/models/agent_logs_model.rs`
- [ ] 定义 `KnowledgeRawSegment` 结构体（参考AgentLogEntry）
- [ ] 定义 `KnowledgeSearchRequest` 结构体（参考AgentLogSearchParams）
- [ ] 定义 `KnowledgeSearchResult` 结构体（参考AgentLogSearchResult）

### 2.2 API请求/响应模型
- [ ] 定义 `StatsParams` 结构体
- [ ] 定义 `DocumentStats` 结构体
- [ ] 定义 `KnowledgeStatsResult` 结构体
- [ ] 定义 `PushRequest` 结构体
- [ ] 定义 `PushResult` 结构体
- [ ] 定义 `UpdateRequest` 结构体
- [ ] 定义 `UpdateResult` 结构体
- [ ] 定义 `DeleteParams` 结构体
- [ ] 定义 `DeleteResult` 结构体
- [ ] 定义 `ClearResult` 结构体

### 2.3 知识库模型字段设计
```rust
// 核心字段参考agent_logs_model.rs，但适配知识库场景
pub struct KnowledgeRawSegment {
    pub id: u64,                    // 分段ID（主键）
    pub raw_id: Option<u64>,        // 所属原始分段ID
    pub kb_id: u64,                 // 知识库ID（必填）
    pub doc_id: u64,                // 文档ID（必填）
    pub raw_txt: String,            // 原始文本（全文检索核心）
    pub sort_index: i64,            // 排序索引
    pub tenant_id: i64,             // 租户ID（必填，用于隔离）
    pub space_id: i64,              // 空间ID（必填）
    pub created: DateTime<Utc>,     // 创建时间（时间戳字段）
}

// 搜索请求参数
pub struct KnowledgeSearchParams {
    pub query: String,                    // 搜索关键词（全文检索）
    pub kb_ids: Option<Vec<u64>>,         // 知识库ID列表（OR关系）
    pub doc_ids: Option<Vec<u64>>,        // 文档ID列表（OR关系）
    pub raw_ids: Option<Vec<u64>>,        // 原始分段ID列表（OR关系）
    pub tenant_id: i64,                   // 租户ID（必填）
    pub space_id: Option<i64>,            // 空间ID（可选）
    pub offset: Option<u64>,              // 分页偏移
    pub limit: Option<u64>,               // 返回数量限制
    pub sort_by: Option<String>,          // 排序字段
    pub sort_order: Option<String>,       // 排序方向
}
```

### 2.4 复用现有宏和特征
- [ ] 参考 `impl_searchable_params!` 宏的使用方式
- [ ] 为KnowledgeSearchParams实现搜索参数构建
- [ ] 确保所有结构体实现 `Serialize`、`Deserialize`、`Debug`、`Clone` 特征
- [ ] 复用现有的PageQuery分页模型

## 3. QuickWit服务实现 (src/services/)

### 3.1 复用现有架构
**参考现有实现**：`src/services/agent_log_quickwit.rs`
- [ ] 创建 `KnowledgeQuickwitService` 结构体（复制AgentLogQuickwitService架构）
- [ ] 复用 `AppStates` 和 `Arc<AppStates>` 模式

### 3.2 核心方法实现
- [ ] 实现 `new()` - 创建服务实例
- [ ] 实现 `create_knowledge_index()` - 创建知识库索引
- [ ] 实现 `ensure_knowledge_index_exists()` - 确保索引存在
- [ ] 实现 `batch_ingest_knowledge_segments()` - 批量摄取数据
- [ ] 实现 `search_knowledge_segments()` - 搜索文档（核心功能）

### 3.3 扩展功能
- [ ] 实现 `get_knowledge_stats()` - 统计数据
- [ ] 实现 `delete_knowledge_segments()` - 删除数据
- [ ] 实现 `update_knowledge_segment()` - 更新文本
- [ ] 实现 `clear_all_knowledge_segments()` - 全量清空

### 3.4 复用现有模式
- [ ] 复用AgentLogQuickwitService的查询构建方式
- [ ] 复用时间戳处理逻辑
- [ ] 复用NDJSON格式处理
- [ ] 复用错误处理模式

## 4. 索引定义 (src/index_define/)

### 4.1 知识库索引配置
**参考现有配置**：`src/index_define/agent_index_define.rs`
- [ ] 创建 `get_knowledge_index_config()` 函数
- [ ] 复制agent索引配置结构，适配知识库字段

### 4.2 索引配置细节
```yaml
version: 0.7
index_id: "knowledge_segments_v1"
doc_mapping:
  mode: dynamic
  timestamp_field: created
  partition_key: tenant_id  # 租户级分片
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
      tokenizer: chinese_compatible  # 中文分词
      record: position              # 支持短语查询
      fieldnorms: true              # BM25评分
    - name: sort_index
      type: i64
      fast: true
    - name: created
      type: datetime
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
```

### 4.3 模块注册
- [ ] 在 `src/index_define/mod.rs` 中导出新函数
- [ ] 添加常量定义 `DEFAULT_KNOWLEDGE_INDEX`

## 5. HTTP API接口实现 (src/api/)

### 5.1 知识库API模块创建
**参考现有实现**：`src/api/agent_logs.rs`
- [ ] 创建 `src/api/knowledge_logs.rs` 文件
- [ ] 复制agent_logs.rs的架构和模式

### 5.2 核心API接口
**参考路由定义**：`src/api/routes.rs`

#### 5.2.1 搜索接口
- [ ] 实现 `knowledge_search_logs` 函数
  - [ ] 路由：`POST /api/knowledge/search`
  - [ ] 参数：`KnowledgeSearchParams` (JSON)
  - [ ] 验证tenant_id必填
  - [ ] 调用 `KnowledgeQuickwitService.search_knowledge_segments()`
  - [ ] 返回 `HttpResult<KnowledgeSearchResult>`

#### 5.2.2 数据推送接口
- [ ] 实现 `knowledge_push_segments` 函数
  - [ ] 路由：`POST /api/knowledge/push`
  - [ ] 参数：`Vec<KnowledgeRawSegment>` (JSON)
  - [ ] 验证tenant_id一致性
  - [ ] 调用 `batch_ingest_knowledge_segments()`
  - [ ] 返回 `HttpResult<PushResult>`

#### 5.2.3 数据删除接口
- [ ] 实现 `knowledge_delete_segments` 函数
  - [ ] 路由：`POST /api/knowledge/delete`
  - [ ] 参数：`DeleteParams` (JSON)
  - [ ] 验证tenant_id必填，除tenant_id外至少一个参数
  - [ ] 调用 `delete_knowledge_segments()`
  - [ ] 返回 `HttpResult<DeleteResult>`

#### 5.2.4 全量清空接口
- [ ] 实现 `knowledge_clear_all_segments` 函数
  - [ ] 路由：`POST /api/knowledge/clear`
  - [ ] 危险操作，需要特殊权限验证
  - [ ] 调用 `clear_all_knowledge_segments()`
  - [ ] 返回 `HttpResult<ClearResult>`

#### 5.2.5 统计接口
- [ ] 实现 `knowledge_get_stats` 函数
  - [ ] 路由：`POST /api/knowledge/stats`
  - [ ] 参数：`StatsParams` (JSON)
  - [ ] 验证tenant_id
  - [ ] 调用 `get_knowledge_stats()`
  - [ ] 返回 `HttpResult<KnowledgeStatsResult>`

#### 5.2.6 文本更新接口
- [ ] 实现 `knowledge_update_segment` 函数
  - [ ] 路由：`POST /api/knowledge/update`
  - [ ] 参数：`UpdateRequest` (JSON)
  - [ ] 验证id或raw_id必填其一
  - [ ] 调用 `update_knowledge_segment()`
  - [ ] 返回 `HttpResult<UpdateResult>`

#### 5.2.7 索引创建接口
- [ ] 实现 `knowledge_create_index` 函数
  - [ ] 路由：`GET /api/knowledge/createIndex`
  - [ ] 调用 `create_knowledge_index()`
  - [ ] 返回 `HttpResult<()>`

### 5.3 路由注册
- [ ] 在 `src/api/routes.rs` 中添加知识库路由：
```rust
.route("/api/knowledge/createIndex", get(knowledge_create_index))
.route("/api/knowledge/search", post(knowledge_search_logs))
.route("/api/knowledge/push", post(knowledge_push_segments))
.route("/api/knowledge/delete", post(knowledge_delete_segments))
.route("/api/knowledge/clear", post(knowledge_clear_all_segments))
.route("/api/knowledge/stats", post(knowledge_get_stats))
.route("/api/knowledge/update", post(knowledge_update_segment))
```

### 5.4 复用现有模式
- [ ] 复用 `HttpResult` 响应格式
- [ ] 复用错误处理方式
- [ ] 复用日志记录模式
- [ ] 复用状态提取方式：`State(app_states): State<Arc<AppStates>>`

## 6. 模块注册和导出

### 6.1 模型模块
- [ ] 在 `src/models/mod.rs` 中添加知识库模型导出：
```rust
pub mod agent_logs_model;
// 添加
pub mod knowledge_logs_model;
```
- [ ] 导出所有知识库相关结构体

### 6.2 服务模块
- [ ] 在 `src/services/mod.rs` 中添加服务导出：
```rust
pub mod agent_log_quickwit;
// 添加
pub mod knowledge_quickwit;
```
- [ ] 导出 `KnowledgeQuickwitService`

### 6.3 API模块
- [ ] 在 `src/api/mod.rs` 中添加导出：
```rust
pub mod agent_logs;
// 添加
pub mod knowledge_logs;
pub mod routes;
```

### 6.4 索引定义模块
- [ ] 在 `src/index_define/mod.rs` 中添加导出：
```rust
pub mod agent_index_define;
// 添加
pub mod knowledge_index_define;
```

## 7. 测试

### 7.1 复用现有测试架构
**参考现有测试**：`src/tests/`
- [ ] 在 `src/tests/` 下创建知识库测试文件
- [ ] 复用现有测试模式

### 7.2 单元测试
- [ ] 为 `KnowledgeQuickwitService` 编写测试
- [ ] 测试索引创建方法
- [ ] 测试批量摄取方法
- [ ] 测试搜索功能
- [ ] 测试错误处理

### 7.3 API集成测试
- [ ] 创建 `knowledge_api_tests.rs`
- [ ] 测试所有知识库API接口
- [ ] 验证tenant_id隔离效果
- [ ] 测试参数验证逻辑

### 7.4 性能测试
- [ ] 创建 `knowledge_performance_tests.rs`
- [ ] 测试搜索性能（延迟、吞吐量）
- [ ] 测试索引性能
- [ ] 内存使用测试

## 8. 主程序集成

### 8.1 在main.rs中初始化
**参考现有实现**：`src/main.rs`
- [ ] 在main.rs中加载QuickWit配置
- [ ] 创建AppStates实例
- [ ] 注册知识库路由

### 8.2 配置更新
- [ ] 检查 `config.yml` 是否包含知识库相关配置
- [ ] 如果需要，添加知识库索引配置参数

## 9. 开发流程建议

### 第一阶段（核心功能，1-2周）
1. **数据模型实现**
   - 定义所有知识库结构体
   - 实现序列化/反序列化

2. **索引配置**
   - 创建索引定义函数
   - 测试索引创建

3. **核心服务**
   - 实现KnowledgeQuickwitService
   - 实现搜索功能
   - 实现数据推送功能

4. **基础API**
   - 实现搜索API
   - 实现推送API
   - 实现索引创建API

### 第二阶段（扩展功能，1周）
1. **删除功能**
   - 实现删除API
   - 实现统计API
   - 实现更新API

2. **安全强化**
   - 完善tenant_id验证
   - 添加操作日志

### 第三阶段（优化完善，3-5天）
1. **性能优化**
   - 查询性能调优
   - 批量操作优化

2. **测试完善**
   - 补充集成测试
   - 性能测试

3. **文档**
   - API文档
   - 部署文档

## 10. 关键注意事项

### 10.1 安全要求
- ✅ **所有接口必须验证tenant_id**（除清空接口外）
- ✅ **租户隔离**：使用partition_key: tenant_id实现物理隔离
- ✅ **参数验证**：严格验证所有必填参数
- ✅ **敏感操作**：全量清空需要特殊权限

### 10.2 技术要点
- ✅ **中文分词**：使用 `chinese_compatible` 分词器
- ✅ **短语查询**：设置 `record: position`
- ✅ **评分排序**：启用 `fieldnorms: true` 支持BM25
- ✅ **快速字段**：所有过滤字段设置为 `fast: true`

### 10.3 架构复用
- ✅ **复用现有模式**：agent_logs模块提供完整参考
- ✅ **一致性**：保持与现有代码风格一致
- ✅ **可扩展性**：便于后续添加新功能

## 11. 验收标准

### 11.1 功能验收
- [ ] 所有7个API接口正常工作
- [ ] 搜索功能支持中文分词
- [ ] 多租户数据隔离有效
- [ ] 租户级分片正常工作
- [ ] 错误处理完善

### 11.2 性能验收
- [ ] 搜索响应时间 < 200ms (95%分位)
- [ ] 支持并发查询 > 100 QPS
- [ ] 索引性能 > 1000 docs/s
- [ ] 内存使用合理

### 11.3 安全验收
- [ ] 租户隔离测试通过
- [ ] 权限控制测试通过
- [ ] 无跨租户数据泄露

---

**总结**：
基于对现有工程架构的深度分析，知识库全文检索模块可以完全复用现有的成熟架构和模式。开发过程中应严格参考agent_logs模块的实现方式，确保代码风格和架构的一致性。所有接口必须强制tenant_id参数，确保多租户数据安全隔离。

**核心要点**：
- ✅ **现有架构完整**：agent_logs模块提供最佳实践参考
- ✅ **快速开发**：只需复制现有模式并适配字段
- ✅ **安全隔离**：租户级分片 + 参数验证双重保障
- ✅ **中文支持**：chinese_compatible分词器处理文本
- ✅ **高性能**：快速字段 + BM25评分优化查询
