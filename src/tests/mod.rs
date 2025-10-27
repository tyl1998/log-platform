#[cfg(test)]
pub mod tests {
    use crate::config::Config;
    use log::info;
    use std::sync::Once;

    // 确保日志系统只初始化一次
    static INIT: Once = Once::new();

    // 初始化测试环境
    pub fn setup() {
        INIT.call_once(|| {
            env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .init();
            info!("测试环境初始化完成");
        });
    }

    // 获取测试配置
    pub fn obtain_test_config() -> Config {
        let config = Config::new().unwrap();
        config
    }
}

// 导入测试模块
#[cfg(test)]
pub mod agent_logs_api_tests;

#[cfg(test)]
pub mod route_tests;

#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
pub mod performance_tests;

#[cfg(test)]
pub mod integration_tests;
