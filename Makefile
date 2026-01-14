.PHONY: help build run stop restart logs clean test dev quickwit-only

# 默认目标
help:
	@echo "Log Platform - Makefile 命令"
	@echo ""
	@echo "使用方法:"
	@echo "  make run              - 构建并启动所有服务（log-platform + quickwit）"
	@echo "  make dev              - 本地开发模式（仅启动 quickwit，本地运行 Rust）"
	@echo "  make build            - 构建 Docker 镜像"
	@echo "  make stop             - 停止所有服务"
	@echo "  make restart          - 重启所有服务"
	@echo "  make logs             - 查看服务日志"
	@echo "  make logs-app         - 查看应用日志"
	@echo "  make logs-quickwit    - 查看 Quickwit 日志"
	@echo "  make clean            - 清理 Docker 资源"
	@echo "  make test             - 运行测试"
	@echo "  make quickwit-only    - 仅启动 Quickwit 服务"
	@echo "  make swagger          - 打开 Swagger UI"
	@echo ""

# 构建 Docker 镜像
build:
	@echo "🔨 构建 Docker 镜像..."
	docker-compose -f docker/docker-compose.yml build

# 启动所有服务
run: build
	@echo "🚀 启动所有服务..."
	docker-compose -f docker/docker-compose.yml up -d
	@echo ""
	@echo "✅ 服务已启动！"
	@echo ""
	@echo "📝 访问地址："
	@echo "  - 应用服务: http://localhost:8098"
	@echo "  - Swagger UI: http://localhost:8098/swagger-ui"
	@echo "  - OpenAPI JSON: http://localhost:8098/api-docs/openapi.json"
	@echo "  - Quickwit UI: http://localhost:7280"
	@echo ""
	@echo "📊 查看日志: make logs"
	@echo "🛑 停止服务: make stop"

# 本地开发模式（仅启动 Quickwit）
dev:
	@echo "🔧 启动开发模式（仅 Quickwit）..."
	docker-compose -f docker/docker-compose.yml up -d quickwit
	@echo ""
	@echo "✅ Quickwit 已启动！"
	@echo ""
	@echo "现在可以本地运行应用："
	@echo "  cargo run"
	@echo ""
	@echo "📝 访问地址："
	@echo "  - Quickwit UI: http://localhost:7280"
	@echo ""

# 仅启动 Quickwit
quickwit-only:
	@echo "🔍 仅启动 Quickwit 服务..."
	docker-compose -f docker/docker-compose.yml up -d quickwit
	@echo "✅ Quickwit 已启动: http://localhost:7280"

# 停止所有服务
stop:
	@echo "🛑 停止所有服务..."
	docker-compose -f docker/docker-compose.yml down
	@echo "✅ 服务已停止"

# 重启所有服务
restart: stop run

# 查看所有服务日志
logs:
	docker-compose -f docker/docker-compose.yml logs -f

# 查看应用日志
logs-app:
	docker-compose -f docker/docker-compose.yml logs -f log-platform

# 查看 Quickwit 日志
logs-quickwit:
	docker-compose -f docker/docker-compose.yml logs -f quickwit

# 清理 Docker 资源
clean:
	@echo "🧹 清理 Docker 资源..."
	docker-compose -f docker/docker-compose.yml down -v
	docker system prune -f
	@echo "✅ 清理完成"

# 运行测试
test:
	@echo "🧪 运行测试..."
	cargo test

# 打开 Swagger UI
swagger:
	@echo "📖 打开 Swagger UI..."
	@command -v open >/dev/null 2>&1 && open http://localhost:8098/swagger-ui || \
	command -v xdg-open >/dev/null 2>&1 && xdg-open http://localhost:8098/swagger-ui || \
	echo "请手动打开: http://localhost:8098/swagger-ui"

# 检查服务状态
status:
	@echo "📊 服务状态："
	@docker-compose -f docker/docker-compose.yml ps

# 进入应用容器
shell-app:
	docker-compose -f docker/docker-compose.yml exec log-platform /bin/bash

# 进入 Quickwit 容器
shell-quickwit:
	docker-compose -f docker/docker-compose.yml exec quickwit /bin/bash

# 本地编译（不使用 Docker）
build-local:
	@echo "🔨 本地编译..."
	cargo build --release
	@echo "✅ 编译完成: target/release/log_platform"

# 本地运行（不使用 Docker）
run-local: build-local
	@echo "🚀 本地运行..."
	./target/release/log_platform

# 格式化代码
fmt:
	cargo fmt

# 代码检查
check:
	cargo check
	cargo clippy

# 查看 Docker 镜像大小
image-size:
	@docker images | grep log-platform || echo "镜像未构建"

# 完整的开发流程
dev-full: dev
	@echo ""
	@echo "等待 Quickwit 启动..."
	@sleep 5
	@echo "🚀 启动应用..."
	cargo run
