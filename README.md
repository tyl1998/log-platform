# Log Platform

A lightweight log platform built with Rust, using QuickWit as the full-text search engine.

[中文文档](./README.zh-CN.md) | [English](./README.md)

## Features

- **Agent Log Management**: Store, search, and manage AI agent interaction logs
- **Knowledge Base**: Full-text search and segment management for knowledge documents
- **Common Log API**: Universal log ingestion and search capabilities
- **REST API**: Simple and intuitive RESTful interface
- **Full-text Search**: Powered by QuickWit for efficient log searching
- **Flexible Configuration**: Multiple configuration loading methods
- **High Performance**: Built with Rust for optimal performance

## Quick Start

### Prerequisites

- Rust 1.70+
- Docker and Docker Compose (recommended)
- Make (optional, for simplified commands)

### Option 1: Using Docker (Recommended)

The simplest way to start all services:

```bash
# Start all services (application + Quickwit)
make run
```

Access URLs:
- **Application**: http://localhost:8098
- **Swagger UI**: http://localhost:8098/swagger-ui
- **OpenAPI JSON**: http://localhost:8098/api-docs/openapi.json
- **Quickwit UI**: http://localhost:7280

View logs:
```bash
make logs
```

Stop services:
```bash
make stop
```

### Option 2: Local Development

1. Clone the repository

```bash
git clone https://github.com/nuwax-ai/log-platform.git
cd log-platform
```

2. Start Quickwit (using Docker)

```bash
make dev
```

3. Run the application

```bash
cargo run
```

The application starts on `http://127.0.0.1:8098` by default.

### Configuration

The project supports multiple configuration loading methods, in the following priority order:

1. Environment variable `LOG_PLATFORM_CONFIG` - specify a custom config file path
2. Container path `/app/config.yml` - for Docker environments
3. Current directory `config.yml` - for local development
4. Built-in default configuration

Configuration file format (YAML):

```yaml
server:
  port: 8098
  log_path: logs

quickwit:
  url: http://127.0.0.1:7280
```

Specify configuration file via environment variable:

```bash
LOG_PLATFORM_CONFIG=/path/to/config.yml cargo run
```

## API Documentation

### Swagger UI

After starting the service, access the interactive API documentation:

**http://localhost:8098/swagger-ui**

### API Overview

#### Health Check

```bash
GET /health
GET /ready
```

#### Agent Log APIs

Manage AI agent interaction logs:

```bash
# Create index
GET /api/agent/log/createIndex

# Add single log
POST /api/agent/log/add

# Batch add logs
POST /api/agent/log/batch

# Search logs
POST /api/agent/log/search

# Query log detail
POST /api/agent/log/detail

# Delete index
DELETE /api/agent/log/delete/{index_name}
```

#### Knowledge Base APIs

Manage knowledge documents:

```bash
# Create index
GET /api/knowledge/createIndex

# Search knowledge
POST /api/knowledge/search

# Push segments
POST /api/knowledge/push

# Update segment
POST /api/knowledge/update

# Delete segments
POST /api/knowledge/delete
POST /api/knowledge/delete-async

# Clear all segments
POST /api/knowledge/clear

# Get statistics
POST /api/knowledge/stats

# Query segment IDs
POST /api/knowledge/segment-ids

# Delete task management
GET /api/knowledge/delete-tasks
GET /api/knowledge/delete-tasks/{task_id}
GET /api/knowledge/delete-tasks/{task_id}/status
```

#### Common Log APIs

Universal log operations:

```bash
# Ingest single log
POST /api/logs

# Batch ingest logs
POST /api/logs/batch

# Search logs
GET /api/logs/search
```

## Development

### Project Structure

```
log-platform/
├── src/
│   ├── api/           # API handlers
│   ├── config/        # Configuration management
│   ├── index_define/  # QuickWit index definitions
│   ├── migration/     # Data migration
│   ├── models/        # Data models
│   ├── services/      # Business logic
│   ├── storage/       # Storage layer
│   └── middlewares/   # HTTP middlewares
├── docker/            # Docker configuration
├── config.yml         # Configuration file
└── Makefile           # Build commands
```

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## License

This project is dual-licensed under either:

- **MIT License** - see [LICENSE-MIT](LICENSE-MIT) for details
- **Apache License 2.0** - see [LICENSE-APACHE](LICENSE-APACHE) for details

You may choose to use this project under either license.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
