# Quickwit 索引更新方案（新增字段 biz_type）

## 背景
- 现状：`agent_logs_v2` 索引已在创建配置中显式包含 `biz_type` 字段，并设置为 `tokenizer: raw`、`indexed: true`、`stored: true`、`fast: true`，且加入 `tag_fields`（见 `src/index_define/agent_index_define.rs:175-181`）。
- 历史：旧索引 `agent_logs` 未包含该字段，当前方案通过“迁移到新索引”完成字段补齐（代码位于迁移管理器）。
- 诉求：希望使用 Quickwit 的“Update an index”端点（`PUT /api/v1/indexes/<index_id>`）在无需完整迁移的情况下，为现有索引新增可检索字段 `biz_type`。

## 官方能力与约束
- 根据官方文档：索引配置可通过 Update 端点更新（参考 `https://quickwit.io/docs/main-branch/configuration/index-config` 与 REST API 文档）。
- 可更新项：`doc_mapping`（含 `field_mappings`/`tag_fields`/`mode`/`dynamic_mapping`）、`search_settings`、`retention`、`indexing_settings` 等。
- 关键限制：Update 端点更新的是“索引的配置元数据”，不会对“已存在的分片（splits）”进行重写或回填。新增字段的索引效果只会应用于“更新后新摄取的数据”。历史数据不会自动具备新字段的索引/fast/tag 特性。

## 可行性结论
- 可以：通过 `PUT /api/v1/indexes/<index_id>` 为现有索引添加 `biz_type` 的 `field_mappings` 与（可选）将其纳入 `tag_fields`。此后，新的摄取数据可按 `biz_type` 过滤/聚合。
- 不足：历史数据不会被自动回填该字段的索引属性；因此：
  - 不能对历史数据基于 `biz_type` 进行稳定的过滤/聚合/（依赖 fast 的）排序。
  - 若业务需要“历史+增量”数据一致支持 `biz_type` 检索能力，仍需执行重摄取/迁移（即重新索引）。

## 与当前代码的匹配
- 当前创建配置已包含 `biz_type` 字段（`src/index_define/agent_index_define.rs:175-181`），并将其纳入 `tag_fields`（`src/index_define/agent_index_define.rs:183-185`）。
- 服务层对排序字段的选择遵循 Quickwit 只支持 fast 字段排序的约束（主要对数值/时间字段，参见 `src/services/agent_log_quickwit.rs:448-456`）。`biz_type` 作为 text/raw 字段用于过滤/聚合更合适。

## 操作步骤（Update 示例）
1. 准备更新后的索引配置（JSON/YAML）。示例（JSON）：
   ```json
   {
     "version": "0.8",
     "index_id": "agent_logs_v2",
     "doc_mapping": {
       "mode": "dynamic",
       "dynamic_mapping": {"indexed": true, "stored": true, "tokenizer": "default", "record": "basic"},
       "field_mappings": [
         {"name": "biz_type", "type": "text", "tokenizer": "raw", "stored": true, "indexed": true, "fast": true}
       ],
       "tag_fields": ["tenant_id", "user_uid", "biz_type"],
       "timestamp_field": "request_start_time",
       "store_source": false
     },
     "search_settings": {"default_search_fields": ["user_input", "output"]},
     "retention": {"period": "90 days", "schedule": "daily"},
     "indexing_settings": {"commit_timeout_secs": 3}
   }
   ```
   注：实际更新时应与现有配置“合并”，避免覆盖掉其他既有字段/设置。建议先 `GET /api/v1/indexes/<index_id>` 拉取当前配置并在此基础上增量修改。

2. 调用 Update 端点：
   ```bash
   curl -X PUT \
     -H "Content-Type: application/json" \
     -d @updated_index_config.json \
     "http://127.0.0.1:7280/api/v1/indexes/agent_logs_v2"
   ```

3. 验证：
   - 新摄取一批含 `biz_type` 的 NDJSON（应用端统一使用 `application/x-ndjson`，见服务层实现）：
     - 智能体日志批量摄取：`src/services/agent_log_quickwit.rs:355-381`
   - 搜索 `biz_type:xxx`，确认新数据命中；历史数据可能不具备该字段的索引命中。

## 影响与限制详解
- 历史数据：Update 不回填旧分片的字段索引信息；若要让旧数据具备 `biz_type` 的过滤/聚合能力，需要“重摄取”。
- 排序能力：Quickwit 仅允许按 fast 字段或 `_score` 排序；`biz_type` 作为 text/raw 字段，通常用于过滤/聚合，不用于排序。
- Tag 元数据：`tag_fields` 变更不会为旧分片补写标签；新分片会按新的配置生成标签。

## 推荐策略
- 仅需增量生效：使用 Update 端点即可，成本最低，立刻对新数据生效。
- 需历史一致：继续采用“新索引 + 迁移”或“在线重摄取”，以保证历史与增量数据在 `biz_type` 上的一致检索能力。
- 折中方案：
  - 立即 Update 以支持增量；
  - 后台异步重摄取历史数据（已有迁移能力，参见 `src/migration/agent_log_migration.rs` 与服务层摄取实现）。

## 风险与注意事项
- 更新配置需确保与现有 `doc_mapping` 一致，不要更改已有字段类型；否则会导致后续摄取/查询异常。
- 批量更新期间建议暂停或控制摄取节奏，避免在配置加载/分片切换瞬间产生意外行为。
- 若需要对旧数据进行 `biz_type` 过滤/聚合，必须配合迁移或重摄取；仅 Update 不足以实现“历史生效”。

## 结论
- 使用 Update 端点新增 `biz_type` 字段：可行且安全，能满足“增量数据”的检索需求。
- 若业务要求“历史+增量一致”的 `biz_type` 能力，仍需执行迁移/重摄取。当前仓库已具备迁移与 NDJSON 摄取能力，可直接复用。