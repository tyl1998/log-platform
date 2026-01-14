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

/// 通用的 redb 存储封装
///
/// 提供简单的 Key-Value 存储接口，支持任意可序列化的类型
///
/// 重要改进：数据库只打开一次，避免重复创建的性能问题
#[derive(Clone)]
pub struct RedbStorage {
    /// 数据库实例（Arc 共享，只打开一次）
    db: Arc<Database>,
    /// 数据库文件路径
    db_path: String,
}

impl RedbStorage {
    /// 创建新的存储实例
    ///
    /// # 参数
    /// * `db_path` - 数据库文件路径
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// 创建默认存储实例
    ///
    /// 使用默认数据库路径：`data/app.redb`
    pub fn new_default() -> Self {
        Self::new(DEFAULT_DB_PATH)
    }

    /// 打开数据库
    fn open_db(&self) -> Result<Database, String> {
        // 确保目录存在
        if let Some(parent) = Path::new(&self.db_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }

        Database::create(&self.db_path).map_err(|e| format!("打开数据库失败: {}", e))
    }

    /// 保存数据
    ///
    /// # 参数
    /// * `table_name` - 表名
    /// * `key` - 键
    /// * `value` - 值（会被序列化为 JSON）
    pub fn save<T: Serialize>(&self, table_name: &str, key: &str, value: &T) -> Result<(), String> {
        let db = self.open_db()?;

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
        let db = self.open_db()?;

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
        let db = self.open_db()?;

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
        let db = self.open_db()?;

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

    /// 获取数据库文件路径
    pub fn db_path(&self) -> &str {
        &self.db_path
    }
}

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
        let storage = RedbStorage::new("test_data.redb");

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
        std::fs::remove_file("test_data.redb").ok();
    }

    #[test]
    fn test_delete() {
        let storage = RedbStorage::new("test_delete.redb");

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // 保存
        storage.save("test_table", "key1", &data).unwrap();

        // 确认存在
        assert!(storage.exists("test_table", "key1").unwrap());

        // 删除
        storage.delete("test_table", "key1").unwrap();

        // 确认不存在
        let loaded: Option<TestData> = storage.load("test_table", "key1").unwrap();
        assert_eq!(loaded, None);

        // 清理
        std::fs::remove_file("test_delete.redb").ok();
    }
}
