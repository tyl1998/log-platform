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
        Self {
            client: Client::new(),
            config: Arc::new(config),
        }
    }
}
