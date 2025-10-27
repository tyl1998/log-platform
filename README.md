# 日志平台

一个基于Rust的轻量级日志平台，使用QuickWit作为全文检索引擎。

## 功能特点

- 提供REST接口用于日志写入和查询
- 支持单条日志和批量日志写入
- 支持全文检索日志内容
- 支持按时间范围、日志级别等过滤
- 使用QuickWit作为后端存储和搜索引擎
- 灵活的配置系统，支持多种配置加载方式

## 快速开始

### 前提条件

- Rust 1.70+
- Docker (可选，用于容器化部署)
- 运行中的QuickWit实例 (默认连接到 `http://127.0.0.1:7280`)

### 本地开发

1. 克隆仓库

```bash
git clone https://github.com/yourusername/log_platform.git
cd log_platform
```

2. 构建和运行

```bash
cargo run
```

应用默认在 `http://127.0.0.1:3000` 上启动。

### 配置系统

项目支持多种方式加载配置，按以下优先级顺序：

1. 环境变量 `LOG_PLATFORM_CONFIG` 指定的配置文件
2. 容器路径 `/app/config.yml`
3. 当前目录下的 `config.yml`
4. 默认内置配置

配置文件使用YAML格式，示例如下：

```yaml
server:
  host: 127.0.0.1
  port: 3000

quickwit:
  url: http://127.0.0.1:7280
  default_index: logs
```

使用环境变量指定配置文件：

```bash
LOG_PLATFORM_CONFIG=/path/to/config.yml cargo run
```

### Docker部署

1. 构建Docker镜像

```bash
docker build -t log_platform:latest .
```

2. 运行容器

```bash
docker run -p 3000:3000 -e QUICKWIT_URL=http://host.docker.internal:7280 -v $(pwd)/config.yml:/app/config.yml log_platform:latest
```

也可以通过环境变量指定配置文件路径：

```bash
docker run -p 3000:3000 -e LOG_PLATFORM_CONFIG=/path/to/config.yml -v $(pwd)/config.yml:/path/to/config.yml log_platform:latest
```

## API接口

### 健康检查

```
GET /health
```

返回HTTP 200表示服务运行正常。

### 写入单条日志

```
POST /api/logs
Content-Type: application/json

{
  "timestamp": "2023-07-01T12:00:00Z",
  "level": "info",
  "message": "这是一条测试日志",
  "service": "api-service",
  "trace_id": "abc123",
  "metadata": {
    "user_id": "user123",
    "request_id": "req456"
  }
}
```

### 批量写入日志

```
POST /api/logs/batch
Content-Type: application/json

[
  {
    "timestamp": "2023-07-01T12:00:00Z",
    "level": "info",
    "message": "日志消息1"
  },
  {
    "timestamp": "2023-07-01T12:01:00Z",
    "level": "error",
    "message": "错误消息"
  }
]
```

### 搜索日志

```
GET /api/logs/search?query=错误&start_time=2023-07-01T00:00:00Z&end_time=2023-07-02T00:00:00Z&limit=10
```

参数说明：
- `query`: 搜索关键词，支持QuickWit查询语法（可选，默认为*匹配所有）
- `start_time`: 开始时间，ISO8601格式（可选）
- `end_time`: 结束时间，ISO8601格式（可选）
- `offset`: 分页偏移量（可选，默认为0）
- `limit`: 返回结果数量限制（可选，默认为20）

## 许可证

MIT
