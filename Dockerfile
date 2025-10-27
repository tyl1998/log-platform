# 构建阶段
FROM rust:1.85-slim as builder

WORKDIR /usr/src/app
COPY . .

RUN apt-get update && apt-get install -y pkg-config libssl-dev
RUN cargo build --release

# 运行阶段
FROM debian:bullseye-slim

WORKDIR /app

# 复制构建好的二进制文件
COPY --from=builder /usr/src/app/target/release/log_platform /app/

# 设置环境变量
ENV RUST_LOG=info

# 暴露端口
EXPOSE 3000

# 运行应用
CMD ["./log_platform"] 