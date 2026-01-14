use std::sync::Arc;

use reqwest::Client;

use crate::config::QuickwitConfig;

#[derive(Clone)]
pub struct AppStates {
    pub client: Client,
    pub config: Arc<QuickwitConfig>,
}

impl AppStates {
    pub fn new(config: QuickwitConfig) -> Self {
        // 配置 HTTP 客户端超时时间，避免请求无限阻塞
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30)) // 总超时30秒
            .connect_timeout(std::time::Duration::from_secs(10)) // 连接超时10秒
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config: Arc::new(config),
        }
    }
}
