use anyhow::Result;
use serde::Deserialize;
use std::env;
use std::fs;
use std::net::SocketAddr;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub quickwit: QuickwitConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub log_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QuickwitConfig {
    pub url: String,
}

impl Config {
    pub fn new() -> Result<Self> {
        Self::load_config()
    }

    /// 从多个位置加载配置文件:
    /// 1. /app/config.yml (容器环境)
    /// 2. 当前目录下的config.yml
    /// 3. LOG_PLATFORM_CONFIG环境变量指定的文件
    /// 4. 默认配置
    pub fn load_config() -> Result<Self> {
        // 先检查环境变量
        let env_path = env::var("LOG_PLATFORM_CONFIG").ok();
        if let Some(path) = &env_path {
            if let Ok(file) = fs::File::open(path) {
                return match serde_yaml::from_reader(file) {
                    Ok(config) => Ok(config),
                    Err(e) => Err(anyhow::anyhow!("从环境变量指定的配置文件加载失败: {}", e)),
                };
            }
        }

        // 尝试从固定位置加载
        if let Ok(file) = fs::File::open("/app/config.yml") {
            return match serde_yaml::from_reader(file) {
                Ok(config) => Ok(config),
                Err(e) => Err(anyhow::anyhow!("从 /app/config.yml 加载失败: {}", e)),
            };
        }

        // 尝试从当前目录加载
        if let Ok(file) = fs::File::open("config.yml") {
            return match serde_yaml::from_reader(file) {
                Ok(config) => Ok(config),
                Err(e) => Err(anyhow::anyhow!("从当前目录的 config.yml 加载失败: {}", e)),
            };
        }

        // 使用默认配置
        Err(anyhow::anyhow!("未找到配置文件，使用默认配置"))
    }

    pub fn server_addr(&self) -> SocketAddr {
        format!("0.0.0.0:{}", self.server.port)
            .parse()
            .expect("无效的服务器地址")
    }
}
