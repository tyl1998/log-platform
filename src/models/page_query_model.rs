use serde::{Deserialize, Serialize};

/// 日志分页结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PageQuery<T>
where
    T: Serialize,
{
    /// 查询条件
    #[serde(rename = "queryFilter")]
    pub query_filter: Option<T>,
    /// 每页显示条数，默认10
    #[serde(rename = "pageSize")]
    #[serde(default = "default_size")]
    pub page_size: i64,
    /// 当前页
    #[serde(default = "default_current")]
    pub current: i64,

    /// 排序字段,没有则不要求排序
    pub orders: Option<Vec<OrderItem>>,
}

impl<T> PageQuery<T>
where
    T: Serialize,
{
    /// 获取偏移量
    pub fn get_offset(&self) -> i64 {
        (self.current - 1) * self.page_size
    }

    /// 获取最大条数
    pub fn get_max_hits(&self) -> i64 {
        self.page_size
    }
}

/// 排序字段定义
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OrderItem {
    // 排序字段
    pub column: String,
    // 是否升序
    pub asc: bool,
}

fn default_size() -> i64 {
    10
}

fn default_current() -> i64 {
    1
}
