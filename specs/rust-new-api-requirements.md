# Rust 新增接口需求 - 数据一致性修复

## 概述

基于现有的 OpenAPI 文档（`specs/openapi-log.json`），所有基础接口都已实现：
- ✅ `/api/knowledge/push` - 推送数据
- ✅ `/api/knowledge/search` - 全文检索
- ✅ `/api/knowledge/delete` - 删除数据
- ✅ `/api/knowledge/update` - 更新分段
- ✅ `/api/knowledge/stats` - 获取统计
- ✅ `/api/knowledge/clear` - 清空数据
- ✅ `/api/knowledge/createIndex` - 创建索引

但是为了实现 Java 端的 `repairKnowledgeBaseConsistency` 方法（数据一致性修复功能），需要新增 **1个接口**。

---

## 新增接口需求

### 1. 查询分段ID列表

#### 接口信息
- **路径**: `POST /api/knowledge/segment-ids`
- **功能**: 查询 Quickwit 中指定知识库的所有分段ID列表
- **用途**: 数据一致性检查和修复
- **标签**: `知识库`
- **操作ID**: `knowledge_query_segment_ids`

#### 请求体 (SegmentIdsParams)
```json
{
  "tenant_id": "1",    // 租户ID（必填，多租户隔离）
  "kb_id": "41",       // 知识库ID（必填）
  "space_id": "10"     // 空间ID（可选，额外过滤条件）
}
```

**字段说明**:
- `tenant_id`: **必填**，用于多租户隔离，防止跨租户访问
- `kb_id`: **必填**，指定要查询的知识库
- `space_id`: **可选**，如果提供则作为额外的过滤条件

#### 响应体 (HttpResult<SegmentIdsResult>)
```json
{
  "code": "0000",
  "message": "操作成功",
  "data": {
    "tenant_id": "1",                    // 租户ID
    "kb_id": "41",                       // 知识库ID
    "space_id": "10",                    // 空间ID（如果提供）
    "total_count": 3000,                 // 总分段数量
    "segment_ids": [                     // 分段ID列表
      "1", "2", "3", "5", "7", "8",
      "10", "11", "12", "15", "..."
    ],
    "query_time": "2025-03-31T10:00:00.123Z"  // 查询时间
  },
  "tid": "uuid-v4"
}
```

**字段说明**:
- `segment_ids`: 返回该知识库在 Quickwit 中的所有 `raw_id` 列表
- `total_count`: 分段总数，用于快速验证
- `query_time`: 查询时间戳

#### 性能要求
- 响应时间: < 2s（即使有10万个分段）
- 内存优化: 如果数据量太大，考虑分页返回或使用流式响应

#### 安全要求
- 必须验证 `tenant_id`，防止跨租户数据泄露
- 只返回 `raw_id`，不返回敏感的文本内容

---

## Java 端使用场景

### 数据一致性修复流程

```java
public long repairKnowledgeBaseConsistency(Long kbId, Long tenantId, Long spaceId) {
    // 1. 查询 MySQL 中的所有分段ID
    List<Long> mysqlSegmentIds = rawSegmentRepository.queryAllSegmentIds(kbId);
    
    // 2. 🆕 调用新接口查询 Quickwit 中的所有分段ID
    List<Long> quickwitSegmentIds = fullTextSearchDomainService.queryAllSegmentIds(kbId, tenantId, spaceId);
    
    // 3. 找出 MySQL 有但 Quickwit 没有的（需要补推）
    Set<Long> quickwitSet = new HashSet<>(quickwitSegmentIds);
    List<Long> missingInQuickwit = mysqlSegmentIds.stream()
        .filter(id -> !quickwitSet.contains(id))
        .collect(Collectors.toList());
    
    // 4. 找出 Quickwit 有但 MySQL 没有的（需要删除）
    Set<Long> mysqlSet = new HashSet<>(mysqlSegmentIds);
    List<Long> extraInQuickwit = quickwitSegmentIds.stream()
        .filter(id -> !mysqlSet.contains(id))
        .collect(Collectors.toList());
    
    long repairedCount = 0;
    
    // 5. 补推缺失的分段
    if (!missingInQuickwit.isEmpty()) {
        log.info("发现 {} 个缺失的分段，开始补推", missingInQuickwit.size());
        
        List<KnowledgeRawSegmentModel> segments = 
            rawSegmentRepository.queryListInfoByIds(missingInQuickwit);
        
        // 转换并推送
        List<RawSegmentFullTextModel> fullTextModels = segments.stream()
            .map(segment -> translator.toFullTextModel(segment, tenantId, spaceId))
            .collect(Collectors.toList());
        
        fullTextSearchDomainService.pushSegments(fullTextModels);
        repairedCount += missingInQuickwit.size();
    }
    
    // 6. 删除多余的分段
    if (!extraInQuickwit.isEmpty()) {
        log.info("发现 {} 个多余的分段，开始删除", extraInQuickwit.size());
        
        fullTextSearchDomainService.deleteByRawIds(extraInQuickwit, tenantId);
        repairedCount += extraInQuickwit.size();
    }
    
    log.info("数据一致性修复完成: 补推={}, 删除={}, 总修复={}", 
        missingInQuickwit.size(), extraInQuickwit.size(), repairedCount);
    
    return repairedCount;
}
```

---

## Rust 端实现建议

### 1. Quickwit 查询实现

```rust
// 使用 Quickwit 的查询 API，只返回 raw_id 字段
let query = json!({
    "query": {
        "bool": {
            "must": [
                {"term": {"tenant_id": tenant_id}},
                {"term": {"kb_id": kb_id}}
            ]
        }
    },
    "_source": ["raw_id"],  // 只返回 raw_id，节省带宽
    "size": 10000,          // 根据实际情况调整
    "sort": [{"raw_id": "asc"}]
});
```

### 2. 大数据量处理

如果单次返回数据量太大，可以考虑：

**方案A: 使用 Quickwit 的 scroll API**
```rust
// 分批获取数据
let mut all_segment_ids = Vec::new();
let mut scroll_id = None;

loop {
    let response = quickwit_client.scroll(scroll_id).await?;
    all_segment_ids.extend(response.segment_ids);
    
    if !response.has_more {
        break;
    }
    scroll_id = Some(response.scroll_id);
}
```

**方案B: 分页返回（如果需要）**
```json
{
  "data": {
    "segment_ids": ["1", "2", "3", "..."],
    "total_count": 100000,
    "has_more": true,
    "next_cursor": "cursor_value"
  }
}
```

### 3. 性能优化建议

- 使用 Quickwit 的聚合查询或 `_source` 过滤，只返回 `raw_id`
- 考虑添加缓存（如果查询频繁）
- 使用异步处理，避免阻塞
- 对于超大数据量（>10万），建议使用流式响应

---

## 错误处理

### 1. 参数错误
```json
{
  "code": "4000",
  "message": "参数错误: tenant_id 不能为空",
  "data": null,
  "tid": "uuid-v4"
}
```

### 2. 知识库不存在
```json
{
  "code": "0000",
  "message": "操作成功",
  "data": {
    "tenant_id": "1",
    "kb_id": "999",
    "total_count": 0,
    "segment_ids": [],
    "query_time": "2025-03-31T10:00:00.123Z"
  },
  "tid": "uuid-v4"
}
```

### 3. 租户验证失败
```json
{
  "code": "4001",
  "message": "租户ID验证失败",
  "data": null,
  "tid": "uuid-v4"
}
```

### 4. Quickwit 查询失败
```json
{
  "code": "5001",
  "message": "Quickwit 查询失败: 具体错误信息",
  "data": null,
  "tid": "uuid-v4"
}
```

---

## 测试用例

### 1. 正常查询
```bash
curl -X POST http://localhost:8097/api/knowledge/segment-ids \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": "1",
    "kb_id": "41",
    "space_id": "10"
  }'
```

**期望响应**:
```json
{
  "code": "0000",
  "message": "操作成功",
  "data": {
    "tenant_id": "1",
    "kb_id": "41",
    "space_id": "10",
    "total_count": 3000,
    "segment_ids": ["1", "2", "3", "..."],
    "query_time": "2025-03-31T10:00:00.123Z"
  },
  "tid": "uuid-v4"
}
```

### 2. 知识库不存在
```bash
curl -X POST http://localhost:8097/api/knowledge/segment-ids \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": "1",
    "kb_id": "999"
  }'
```

**期望响应**:
```json
{
  "code": "0000",
  "message": "操作成功",
  "data": {
    "tenant_id": "1",
    "kb_id": "999",
    "total_count": 0,
    "segment_ids": [],
    "query_time": "2025-03-31T10:00:00.123Z"
  },
  "tid": "uuid-v4"
}
```

### 3. 参数错误
```bash
curl -X POST http://localhost:8097/api/knowledge/segment-ids \
  -H "Content-Type: application/json" \
  -d '{
    "kb_id": "41"
  }'
```

**期望响应**:
```json
{
  "code": "4000",
  "message": "参数错误: tenant_id 不能为空",
  "data": null,
  "tid": "uuid-v4"
}
```

### 4. 大数据量测试
```bash
# 测试 10 万个分段的查询性能
curl -X POST http://localhost:8097/api/knowledge/segment-ids \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": "1",
    "kb_id": "100"
  }' \
  -w "\nTime: %{time_total}s\n"
```

**期望**: 响应时间 < 2s

---

## OpenAPI Schema 定义

### SegmentIdsParams
```json
{
  "type": "object",
  "description": "查询分段ID列表请求参数",
  "required": ["tenant_id", "kb_id"],
  "properties": {
    "tenant_id": {
      "type": "string",
      "description": "租户ID（必填，多租户隔离）"
    },
    "kb_id": {
      "type": "string",
      "description": "知识库ID（必填）"
    },
    "space_id": {
      "type": ["string", "null"],
      "description": "空间ID（可选，额外过滤条件）"
    }
  }
}
```

### SegmentIdsResult
```json
{
  "type": "object",
  "description": "查询分段ID列表响应结果",
  "required": ["tenant_id", "kb_id", "total_count", "segment_ids", "query_time"],
  "properties": {
    "tenant_id": {
      "type": "string",
      "description": "租户ID"
    },
    "kb_id": {
      "type": "string",
      "description": "知识库ID"
    },
    "space_id": {
      "type": ["string", "null"],
      "description": "空间ID"
    },
    "total_count": {
      "type": "integer",
      "format": "int64",
      "description": "总分段数量",
      "minimum": 0
    },
    "segment_ids": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "分段ID列表"
    },
    "query_time": {
      "type": "string",
      "format": "date-time",
      "description": "查询时间"
    }
  }
}
```

---

## 总结

### 需要新增的接口

**只需要 1 个新接口**:
- `POST /api/knowledge/segment-ids` - 查询分段ID列表

### 接口用途

专门用于 Java 端的 `repairKnowledgeBaseConsistency` 方法，实现：
1. 🔍 **数据一致性检查** - 对比 MySQL 和 Quickwit 的分段ID
2. 🔧 **自动修复** - 补推缺失的分段，删除多余的分段
3. 📊 **修复报告** - 返回修复的分段数量

### 实现优先级

- **高优先级**: 基础功能（查询分段ID列表）
- **中优先级**: 性能优化（大数据量处理）
- **低优先级**: 分页支持（如果单次查询性能可接受）

### 关键技术点

1. **只返回 raw_id**: 不返回文本内容，节省带宽和内存
2. **多租户隔离**: 必须验证 tenant_id
3. **性能优化**: 使用 Quickwit 的 `_source` 过滤和排序
4. **错误处理**: 完善的错误码和错误信息

---

**文档版本**: 1.0  
**创建时间**: 2025-03-31  
**依赖**: 现有的 7 个接口都已实现  
**新增**: 仅需 1 个接口
