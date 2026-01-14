mod api;
mod config;
mod index_define;
mod macros;
mod middlewares;
mod migration;
mod models;
mod my_error;
mod services;
mod storage;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use crate::my_error::AppError;
use anyhow::Result;
use config::Config;
use log::{error, info};
use migration::AgentLogMigrationManager;
use models::AppStates;
use services::{AgentLogQuickwitService, KnowledgeQuickwitService, RecordCommonLogQuickwitService};
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

// 索引创建重试次数
const MAX_INDEX_CREATE_RETRIES: u8 = 5;
// 重试间隔（秒）
const RETRY_INTERVAL_SECONDS: u64 = 3;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载配置
    let config = Config::new()?;

    let log_path = config.server.log_path.clone();

    // 初始化日志
    // 解析 RUST_LOG 环境变量
    let log_level = "info";

    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // 使用 tracing-subscriber 初始化日志记录器
    let console_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(std::io::stdout)
        .with_filter(console_filter);
    // 日志写入到文件
    let file_appender = tracing_appender::rolling::daily(log_path, "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let log_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_filter(log_filter);

    // 初始化 tracing 订阅器
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    info!("服务器端口配置: {}", config.server_addr());

    let app_states = Arc::new(AppStates::new(config.quickwit.clone()));
    info!("QuickWit服务初始化成功");

    // 异步创建索引，带重试机制
    tokio::spawn(create_indexes_with_retry(app_states.clone()));

    // 启动Web服务器
    let app = api::init_routes(app_states.clone());
    let addr = config.server_addr();
    info!("服务器启动于 {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 异步创建索引，带重试机制
async fn create_indexes_with_retry(app_states: Arc<AppStates>) {
    // 创建通用日志索引
    create_index_with_retry("通用日志索引", MAX_INDEX_CREATE_RETRIES, || async {
        let service = RecordCommonLogQuickwitService::new(app_states.clone());
        // 先检查索引是否存在
        if !check_index_exists(service.get_url(), service.get_index_name()).await {
            info!("通用日志索引不存在，开始创建");
            service.create_index().await
        } else {
            info!("通用日志索引已存在，无需创建");
            Ok(())
        }
    })
    .await;

    // 创建知识库索引（优先创建，因为不涉及数据迁移）
    create_index_with_retry("知识库索引", MAX_INDEX_CREATE_RETRIES, || async {
        let service = KnowledgeQuickwitService::new(app_states.clone());
        let index_name = service.get_knowledge_index_name();

        // 检查索引是否存在
        if !check_index_exists(service.get_url(), index_name).await {
            info!("知识库索引 {} 不存在，开始创建", index_name);
            service.ensure_knowledge_index_exists().await
        } else {
            info!("知识库索引 {} 已存在，无需创建", index_name);
            Ok(())
        }
    })
    .await;

    // 创建智能体日志索引（不包含数据迁移）
    create_index_with_retry("智能体日志索引", MAX_INDEX_CREATE_RETRIES, || async {
        let service = AgentLogQuickwitService::new(app_states.clone());
        let v2_index_name = service.get_agent_index_name();

        // 检查新版本索引是否存在
        if !check_index_exists(service.get_url(), v2_index_name).await {
            info!("新版本智能体日志索引 {} 不存在，开始创建", v2_index_name);
            service.create_agent_index().await
        } else {
            info!("新版本智能体日志索引 {} 已存在，无需创建", v2_index_name);
            Ok(())
        }
    })
    .await;

    // 在独立的任务中处理数据迁移（避免阻塞服务启动）
    let app_states_for_migration = app_states.clone();
    tokio::spawn(async move {
        info!("🔍 开始智能体日志数据迁移检查");

        // 使用默认存储路径创建迁移管理器
        let mut migration_manager =
            match AgentLogMigrationManager::new_with_default_storage(app_states_for_migration) {
                Ok(manager) => manager,
                Err(e) => {
                    error!("❌ 创建迁移管理器失败: {}", e);
                    return;
                }
            };

        // 直接调用 migrate，它内部会检查状态并决定是否需要迁移
        match migration_manager.migrate().await {
            Ok(_) => {
                info!("✅ 数据迁移流程执行完成（可能已完成或无需迁移）");
            }
            Err(e) => {
                error!("❌ 数据迁移执行失败: {}", e);
                error!("💡 提示：可以稍后手动调用 /api/agent/log/migrateData 接口重试迁移");
            }
        }
    });
}

/// 检查索引是否存在
async fn check_index_exists(quickwit_url: &str, index_name: &str) -> bool {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/indexes/{}", quickwit_url, index_name);

    info!("检查索引是否存在: {}", url);

    match client.get(&url).send().await {
        Ok(response) => {
            let status = response.status();
            info!("索引检查响应状态: {}", status);
            status.is_success()
        }
        Err(e) => {
            info!("索引检查请求失败: {}", e);
            false
        }
    }
}

/// 带重试机制的索引创建函数
async fn create_index_with_retry<F, Fut>(index_name: &str, max_retries: u8, create_fn: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<(), AppError>>,
{
    let mut retry_count = 0;

    loop {
        match create_fn().await {
            Ok(_) => {
                info!("{}创建成功或已存在", index_name);
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    info!("{}创建失败，已达到最大重试次数: {}", index_name, e);
                    break;
                } else {
                    info!(
                        "{}创建失败，将在 {} 秒后进行第 {}/{} 次重试: {}",
                        index_name, RETRY_INTERVAL_SECONDS, retry_count, max_retries, e
                    );
                    sleep(Duration::from_secs(RETRY_INTERVAL_SECONDS)).await;
                }
            }
        }
    }
}
