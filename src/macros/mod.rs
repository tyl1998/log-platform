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

                // 处理字符串字段 - 支持 String 和 Option<String>
                $(
                    {
                        let ref_value = &self.$string_field;
                        match ref_value {
                            Some(v) => {
                                if !v.trim().is_empty() {
                                    query_parts.push(format!("{}:{}", stringify!($string_field), v));
                                }
                            }
                            None => {}
                        }
                    }
                )*

                // 处理数组字段
                $(
                    if let Some(ref values) = self.$array_field {
                        let valid_values: Vec<String> = values
                            .iter()
                            .filter(|v| {
                                // 对于字符串类型，检查trim后是否为空
                                // 对于数值类型，只要不是0就认为是有效值
                                match v.to_string().as_str() {
                                    s if s.chars().all(|c| c.is_whitespace() || c.is_digit(10)) => {
                                        // 纯数字或空格，转换为数字检查是否为0
                                        v.to_string().trim().parse::<i64>().unwrap_or(0) != 0
                                    }
                                    _ => {
                                        // 包含其他字符，使用trim检查
                                        !v.to_string().trim().is_empty()
                                    }
                                }
                            })
                            .map(|v| format!("{}:{}", stringify!($array_field), v))
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

    // 重载处理 String 类型（非 Option）
    (
        $struct_name:ident,
        scalar_fields: [ $($scalar_field:ident),* ],
        array_fields: [ $($array_field:ident => $operator:expr),* ]
    ) => {
        impl $struct_name {
            /// 生成查询参数列表
            pub fn build_query_parts(&self) -> Vec<String> {
                let mut query_parts = Vec::new();

                // 处理标量字段 - 支持 String 和数值类型
                $(
                    let value = &self.$scalar_field;
                    let value_str = format!("{}", value);
                    // 检查字段是否有效（字符串非空，数值非零）
                    if !value_str.trim().is_empty() && value_str != "0" {
                        query_parts.push(format!("{}:{}", stringify!($scalar_field), value_str));
                    }
                )*

                // 处理数组字段
                $(
                    if let Some(ref values) = self.$array_field {
                        let valid_values: Vec<String> = values
                            .iter()
                            .filter(|v| {
                                // 对于数值类型，检查是否为0；对于字符串类型，检查trim后是否为空
                                let v_str = v.to_string();
                                if v_str.chars().all(|c| c.is_whitespace() || c.is_digit(10)) {
                                    // 纯数字，检查是否为0
                                    v_str.trim().parse::<i64>().unwrap_or(0) != 0
                                } else {
                                    // 包含其他字符，使用trim检查
                                    !v_str.trim().is_empty()
                                }
                            })
                            .map(|v| format!("{}:{}", stringify!($array_field), v))
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
