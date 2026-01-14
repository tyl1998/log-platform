# 日志平台

一个基于 Rust 的轻量级日志平台，使用 QuickWit 作为全文检索引擎。

[中文文档](./README.zh-CN.md) | [English](./README.md)

## 功能特点

- **Agent 日志管理**：存储、搜索和管理 AI 智能体交互日志
- **知识库管理**：知识文档的全文检索和分段管理
- **通用日志 API**：支持通用的日志写入和查询
- **REST API**：简单直观的 RESTful 接口
- **全文检索**：基于 QuickWit 的高效日志搜索
- **灵活配置**：支持多种配置加载方式
- **高性能**：使用 Rust 构建，性能优异

## 快速开始

### 前提条件

- Rust 1.70+
- Docker 和 Docker Compose（推荐）
- Make（可选，用于简化命令）

### 方式一：使用 Docker（推荐）

最简单的方式，一键启动所有服务：

```bash
# 启动所有服务（应用 + Quickwit）
make run
```

访问地址：
- **应用服务**: http://localhost:8098
- **Swagger UI**: http://localhost:8098/swagger-ui
- **OpenAPI JSON**: http://localhost:8098/api-docs/openapi.json
- **Quickwit UI**: http://localhost:7280

查看日志：
```bash
make logs
```

停止服务：
```bash
make stop
```

### 方式二：本地开发

1. 克隆仓库

```bash
git clone https://github.com/nuwax-ai/log-platform.git
cd log-platform
```

2. 启动 Quickwit（使用 Docker）

```bash
make dev
```

3. 运行应用

```bash
cargo run
```

应用默认在 `http://127.0.0.1:8098` 上启动。

### 配置说明

项目支持多种方式加载配置，按以下优先级顺序：

1. 环境变量 `LOG_PLATFORM_CONFIG` - 指定自定义配置文件路径
2. 容器路径 `/app/config.yml` - 用于 Docker 环境
3. 当前目录 `config.yml` - 用于本地开发
4. 内置默认配置

配置文件格式（YAML）：

```yaml
server:
  port: 8098
  log_path: logs

quickwit:
  url: http://127.0.0.1:7280
```

通过环境变量指定配置文件：

```bash
LOG_PLATFORM_CONFIG=/path/to/config.yml cargo run
```

## API 文档

### Swagger UI

启动服务后，访问交互式 API 文档：

**http://localhost:8098/swagger-ui**

### API 概览

#### 健康检查

```bash
GET /health
GET /ready
```

#### Agent 日志 API

管理 AI 智能体交互日志：

```bash
# 创建索引
GET /api/agent/log/createIndex

# 添加单条日志
POST /api/agent/log/add

# 批量添加日志
POST /api/agent/log/batch

# 搜索日志
POST /api/agent/log/search

# 查询日志详情
POST /api/agent/log/detail

# 删除索引
DELETE /api/agent/log/delete/{index_name}
```

#### 知识库 API

管理知识文档：

```bash
# 创建索引
GET /api/knowledge/createIndex

# 搜索知识
POST /api/knowledge/search

# 推送分段
POST /api/knowledge/push

# 更新分段
POST /api/knowledge/update

# 删除分段
POST /api/knowledge/delete
POST /api/knowledge/delete-async

# 清空所有分段
POST /api/knowledge/clear

# 获取统计信息
POST /api/knowledge/stats

# 查询分段 ID
POST /api/knowledge/segment-ids

# 删除任务管理
GET /api/knowledge/delete-tasks
GET /api/knowledge/delete-tasks/{task_id}
GET /api/knowledge/delete-tasks/{task_id}/status
```

#### 通用日志 API

通用日志操作：

```bash
# 写入单条日志
POST /api/logs

# 批量写入日志
POST /api/logs/batch

# 搜索日志
GET /api/logs/search
```

## 开发

### 项目结构

```
log-platform/
├── src/
│   ├── api/           # API 处理器
│   ├── config/        # 配置管理
│   ├── index_define/  # QuickWit 索引定义
│   ├── migration/     # 数据迁移
│   ├── models/        # 数据模型
│   ├── services/      # 业务逻辑
│   ├── storage/       # 存储层
│   └── middlewares/   # HTTP 中间件
├── docker/            # Docker 配置
├── config.yml         # 配置文件
└── Makefile           # 构建命令
```

### 运行测试

```bash
cargo test
```

### 代码格式化

```bash
cargo fmt
```

### 代码检查

```bash
cargo clippy
```

## 许可证

本项目采用双协议授权，您可以选择以下任一许可：

- **MIT License** - 详见 [LICENSE-MIT](LICENSE-MIT)
- **Apache License 2.0** - 详见 [LICENSE-APACHE](LICENSE-APACHE)

## 贡献

欢迎贡献代码！请随时提交 Pull Request。
