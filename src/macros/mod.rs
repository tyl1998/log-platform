/// 为结构体实现搜索参数处理的宏
/// 自动处理字符串和数组类型字段
#[macro_export]
macro_rules! impl_searchable_params {
    (
        $struct_name:ident,
        string_fields: [ $($string_field:ident),* ],
        array_fields: [ $($array_field:ident => $operator:expr),* ]
    ) => {
        impl $struct_name {
            /// 生成查询参数列表
            pub fn build_query_parts(&self) -> Vec<String> {
                let mut query_parts = Vec::new();

                // 处理字符串字段
                $(
                    if let Some(ref value) = self.$string_field {
                        if !value.trim().is_empty() {
                            query_parts.push(format!("{}:{}", stringify!($string_field), value));
                        }
                    }
                )*

                // 处理数组字段
                $(
                    if let Some(ref values) = self.$array_field {
                        let valid_values: Vec<String> = values
                            .iter()
                            .filter(|v| !v.trim().is_empty())
                            .map(|v| format!("{}:{}", stringify!($array_field), v.trim()))
                            .collect();

                        if !valid_values.is_empty() {
                            if valid_values.len() == 1 {
                                query_parts.push(valid_values[0].clone());
                            } else {
                                query_parts.push(format!("({})", valid_values.join($operator)));
                            }
                        }
                    }
                )*

                query_parts
            }
        }
    };
}
