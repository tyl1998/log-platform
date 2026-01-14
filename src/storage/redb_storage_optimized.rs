use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// 默认数据库文件路径
pub const DEFAULT_DB_PATH: &str = "data/app.redb";

/// 表名常量
pub mod tables {
    /// 迁移状态表
    pub const MIGRATION_STATUS: &str = "migration_status";

    /// 已迁移的 request_id 集合表
    pub const MIGRATED_IDS: &str = "migrated_request_ids";
}

/// 优化的 redb 存储封装
///
/// 重要改进：
/// 1. 数据库只打开一次，避免重复创建的性能问题
/// 2. 支持批量操作，减少事务数量
/// 3. 使用 Arc 共享数据库实例
/// 4. 为大量数据提供特殊优化
#[derive(Clone)]
pub struct RedbStorageOptimized {
    /// 数据库实例（Arc 共享，只打开一次）
    db: Arc<Database>,
    /// 数据库文件路径
    db_path: String,
}

impl RedbStorageOptimized {
    /// 创建新的存储实例
    ///
    /// # 参数
    /// * `db_path` - 数据库文件路径
    pub fn new(db_path: impl Into<String>) -> Result<Self, String> {
        let db_path = db_path.into();

        // 确保目录存在
        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }

        // 只打开一次数据库，如果存在则打开，不存在则创建
        let db = if Path::new(&db_path).exists() {
            Database::open(&db_path).map_err(|e| format!("打开现有数据库失败: {}", e))?
        } else {
            Database::create(&db_path).map_err(|e| format!("创建数据库失败: {}", e))?
        };

        Ok(Self {
            db: Arc::new(db),
            db_path,
        })
    }

    /// 创建默认存储实例
    ///
    /// 使用默认数据库路径：`data/app.redb`
    pub fn new_default() -> Result<Self, String> {
        Self::new(DEFAULT_DB_PATH)
    }

    /// 获取数据库引用（避免重复打开）
    fn get_db(&self) -> &Database {
        &self.db
    }

    /// 保存单个数据（保持向后兼容）
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `key` - 键
    /// * `value` - 值（会被序列化为 JSON）
    pub fn save<T: Serialize>(&self, table_name: &str, key: &str, value: &T) -> Result<(), String> {
        let db = self.get_db();

        let write_txn = db
            .begin_write()
            .map_err(|e| format!("开始写事务失败: {}", e))?;

        {
            let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| format!("打开表失败: {}", e))?;

            let json = serde_json::to_vec(value).map_err(|e| format!("序列化失败: {}", e))?;

            table
                .insert(key, json.as_slice())
                .map_err(|e| format!("写入失败: {}", e))?;
        }

        write_txn
            .commit()
            .map_err(|e| format!("提交事务失败: {}", e))?;

        Ok(())
    }

    /// 批量保存数据（性能优化版本）
    ///
    /// 专门用于大量数据的高效批量插入，使用单个事务处理多个键值对
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `items` - 键值对数组
    pub fn save_batch<T: Serialize>(
        &self,
        table_name: &str,
        items: &[(String, T)],
    ) -> Result<(), String> {
        if items.is_empty() {
            return Ok(());
        }

        let db = self.get_db();

        let write_txn = db
            .begin_write()
            .map_err(|e| format!("开始批量写事务失败: {}", e))?;

        {
            let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| format!("打开表失败: {}", e))?;

            for (key, value) in items {
                let json = serde_json::to_vec(value)
                    .map_err(|e| format!("序列化失败，键: {}, 错误: {}", key, e))?;

                table
                    .insert(key.as_str(), json.as_slice())
                    .map_err(|e| format!("写入失败，键: {}, 错误: {}", key, e))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| format!("提交批量事务失败: {}", e))?;

        Ok(())
    }

    /// 批量标记字符串（专门用于迁移 ID 的高效存储）
    ///
    /// 使用单字节值替代 JSON 序列化，大幅提升性能
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `ids` - ID 数组
    pub fn mark_ids_batch(&self, table_name: &str, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let db = self.get_db();

        let write_txn = db
            .begin_write()
            .map_err(|e| format!("开始标记ID事务失败: {}", e))?;

        {
            let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| format!("打开表失败: {}", e))?;

            // 使用单字节值 [1] 替代 JSON 序列化，大幅提升性能
            let value = [1u8];
            for id in ids {
                table
                    .insert(id.as_str(), &value)
                    .map_err(|e| format!("标记ID失败: {}, 错误: {}", id, e))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| format!("提交标记事务失败: {}", e))?;

        Ok(())
    }

    /// 加载数据
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `key` - 键
    ///
    /// # 返回
    /// * `Ok(Some(T))` - 找到数据
    /// * `Ok(None)` - 未找到数据
    /// * `Err(String)` - 发生错误
    pub fn load<T: DeserializeOwned>(
        &self,
        table_name: &str,
        key: &str,
    ) -> Result<Option<T>, String> {
        let db = self.get_db();

        let read_txn = db
            .begin_read()
            .map_err(|e| format!("开始读事务失败: {}", e))?;

        let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| format!("打开表失败: {}", e))?;

        let value = match table.get(key) {
            Ok(Some(v)) => v,
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("读取失败: {}", e)),
        };

        let data: T =
            serde_json::from_slice(value.value()).map_err(|e| format!("反序列化失败: {}", e))?;

        Ok(Some(data))
    }

    /// 删除数据
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `key` - 键
    pub fn delete(&self, table_name: &str, key: &str) -> Result<(), String> {
        let db = self.get_db();

        let write_txn = db
            .begin_write()
            .map_err(|e| format!("开始写事务失败: {}", e))?;

        {
            let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| format!("打开表失败: {}", e))?;

            table.remove(key).map_err(|e| format!("删除失败: {}", e))?;
        }

        write_txn
            .commit()
            .map_err(|e| format!("提交事务失败: {}", e))?;

        Ok(())
    }

    /// 检查键是否存在
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `key` - 键
    pub fn exists(&self, table_name: &str, key: &str) -> Result<bool, String> {
        let db = self.get_db();

        let read_txn = db
            .begin_read()
            .map_err(|e| format!("开始读事务失败: {}", e))?;

        let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| format!("打开表失败: {}", e))?;

        let exists = table
            .get(key)
            .map_err(|e| format!("检查失败: {}", e))?
            .is_some();

        Ok(exists)
    }

    /// 批量检查键是否存在（性能优化版本）
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `keys` - 键数组
    ///
    /// # 返回
    /// * 存在的键数组
    pub fn exists_batch(&self, table_name: &str, keys: &[String]) -> Result<Vec<String>, String> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let db = self.get_db();

        let read_txn = db
            .begin_read()
            .map_err(|e| format!("开始批量读事务失败: {}", e))?;

        let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| format!("打开表失败: {}", e))?;

        let mut existing_keys = Vec::new();
        for key in keys {
            if table
                .get(key)
                .map_err(|e| format!("检查键失败: {}, 错误: {}", key, e))?
                .is_some()
            {
                existing_keys.push(key.clone());
            }
        }

        Ok(existing_keys)
    }

    /// 获取数据库文件路径
    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// 获取表中的键数量（用于统计）
    ///
    /// # 参数
    /// * `table_name` - 表名
    pub fn count_keys(&self, table_name: &str) -> Result<usize, String> {
        let db = self.get_db();

        let read_txn = db
            .begin_read()
            .map_err(|e| format!("开始计数事务失败: {}", e))?;

        let table_def: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| format!("打开表失败: {}", e))?;

        let count = table.len().map_err(|e| format!("获取表长度失败: {}", e))?;

        Ok(count)
    }
}

// 为了向后兼容，提供原有 RedbStorage 的别名
pub type RedbStorage = RedbStorageOptimized;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_save_and_load() {
        let storage = RedbStorageOptimized::new("test_data_optimized.redb").unwrap();

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // 保存
        storage.save("test_table", "key1", &data).unwrap();

        // 加载
        let loaded: Option<TestData> = storage.load("test_table", "key1").unwrap();
        assert_eq!(loaded, Some(data));

        // 清理
        std::fs::remove_file("test_data_optimized.redb").ok();
    }

    #[test]
    fn test_batch_operations() {
        let storage = RedbStorageOptimized::new("test_batch.redb").unwrap();

        let items = vec![
            (
                "key1".to_string(),
                TestData {
                    name: "test1".to_string(),
                    value: 1,
                },
            ),
            (
                "key2".to_string(),
                TestData {
                    name: "test2".to_string(),
                    value: 2,
                },
            ),
            (
                "key3".to_string(),
                TestData {
                    name: "test3".to_string(),
                    value: 3,
                },
            ),
        ];

        // 批量保存
        storage.save_batch("batch_table", &items).unwrap();

        // 验证数据
        for (key, expected_data) in &items {
            let loaded: Option<TestData> = storage.load("batch_table", key).unwrap();
            assert_eq!(loaded, Some(expected_data.clone()));
        }

        // 批量存在性检查
        let keys = vec!["key1".to_string(), "key2".to_string(), "key4".to_string()];
        let existing = storage.exists_batch("batch_table", &keys).unwrap();
        assert_eq!(existing.len(), 2); // key4 不存在

        // 清理
        std::fs::remove_file("test_batch.redb").ok();
    }

    #[test]
    fn test_id_marking() {
        let storage = RedbStorageOptimized::new("test_ids.redb").unwrap();

        let ids = vec!["id1".to_string(), "id2".to_string(), "id3".to_string()];

        // 标记ID
        storage.mark_ids_batch("id_table", &ids).unwrap();

        // 检查存在性
        assert!(storage.exists("id_table", "id1").unwrap());
        assert!(storage.exists("id_table", "id2").unwrap());
        assert!(storage.exists("id_table", "id3").unwrap());
        assert!(!storage.exists("id_table", "id4").unwrap());

        // 检查数量
        let count = storage.count_keys("id_table").unwrap();
        assert_eq!(count, 3);

        // 清理
        std::fs::remove_file("test_ids.redb").ok();
    }
}
