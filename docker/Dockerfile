# 构建阶段
FROM rust:1.90-slim as builder

WORKDIR /usr/src/app

# 安装构建依赖
# 注意：utoipa-swagger-ui 需要 curl 来下载 Swagger UI 资源
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev curl && \
    rm -rf /var/lib/apt/lists/*

# 复制依赖文件
COPY Cargo.toml Cargo.lock ./


# 复制实际的源代码
COPY src ./src
COPY assets ./assets

# 构建实际的应用（依赖已缓存，只编译项目代码）
RUN cargo build --release

# 运行阶段
# 使用 Debian Bookworm 以匹配 Rust 1.85 的 OpenSSL 3.x
FROM debian:bookworm-slim

WORKDIR /app

# 安装运行时依赖
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/*

# 复制构建好的二进制文件
COPY --from=builder /usr/src/app/target/release/log_platform /app/

# 复制配置文件（Docker 环境专用）
COPY config.docker.yml /app/config.yml

# 创建日志目录
RUN mkdir -p /app/logs

# 设置环境变量
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# 暴露端口
EXPOSE 8098

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8098/health || exit 1

# 运行应用
CMD ["./log_platform"] 