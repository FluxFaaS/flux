use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::functions::{FluxError, FunctionMetadata};

/// 函数存储记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRecord {
    pub metadata: FunctionMetadata,
    pub source_code: String,
    pub source_path: Option<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub version: String,
    pub dependencies: Vec<String>,
    pub checksum: String,
}

impl FunctionRecord {
    pub fn new(
        metadata: FunctionMetadata,
        source_code: String,
        source_path: Option<PathBuf>,
        version: String,
        dependencies: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        let checksum = format!("{:x}", md5::compute(&source_code));

        Self {
            metadata,
            source_code,
            source_path,
            created_at: now,
            updated_at: now,
            version,
            dependencies,
            checksum,
        }
    }

    pub fn update_code(&mut self, new_code: String) {
        self.source_code = new_code;
        self.updated_at = chrono::Utc::now();
        self.checksum = format!("{:x}", md5::compute(&self.source_code));
    }

    pub fn update_version(&mut self, new_version: String) {
        self.version = new_version;
        self.updated_at = chrono::Utc::now();
    }
}

/// 函数存储后端接口
#[async_trait::async_trait]
pub trait FunctionStorage: Send + Sync {
    async fn store(&self, name: &str, record: FunctionRecord) -> Result<()>;
    async fn load(&self, name: &str) -> Result<Option<FunctionRecord>>;
    async fn delete(&self, name: &str) -> Result<bool>;
    async fn list(&self) -> Result<Vec<String>>;
    async fn exists(&self, name: &str) -> Result<bool>;
    async fn get_version(&self, name: &str) -> Result<Option<String>>;
    async fn list_versions(&self, name: &str) -> Result<Vec<String>>;
    async fn backup(&self) -> Result<PathBuf>;
    async fn restore(&self, backup_path: &Path) -> Result<()>;
}

/// 文件系统存储实现
pub struct FileSystemStorage {
    storage_dir: PathBuf,
    functions: Arc<RwLock<HashMap<String, FunctionRecord>>>,
}

impl FileSystemStorage {
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        // 确保存储目录存在
        fs::create_dir_all(&storage_dir)
            .with_context(|| format!("Failed to create storage directory: {storage_dir:?}"))?;

        let storage = Self {
            storage_dir,
            functions: Arc::new(RwLock::new(HashMap::new())),
        };

        Ok(storage)
    }

    fn get_function_file_path(&self, name: &str) -> PathBuf {
        self.storage_dir.join(format!("{name}.json"))
    }

    fn get_backup_path(&self) -> PathBuf {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        self.storage_dir.join(format!("backup_{timestamp}.json"))
    }

    async fn load_from_disk(&self) -> Result<()> {
        let mut functions = self.functions.write().await;

        if !self.storage_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.storage_dir)
            .with_context(|| format!("Failed to read storage directory: {:?}", self.storage_dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if name.starts_with("backup_") {
                        continue; // 跳过备份文件
                    }

                    match self.load_function_from_file(&path).await {
                        Ok(record) => {
                            functions.insert(name.to_string(), record);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load function {} from disk: {}", name, e);
                        }
                    }
                }
            }
        }

        tracing::info!("Loaded {} functions from disk", functions.len());
        Ok(())
    }

    async fn load_function_from_file(&self, path: &Path) -> Result<FunctionRecord> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read function file: {path:?}"))?;

        let record: FunctionRecord = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse function file: {path:?}"))?;

        Ok(record)
    }

    async fn save_function_to_file(&self, name: &str, record: &FunctionRecord) -> Result<()> {
        let path = self.get_function_file_path(name);
        let content = serde_json::to_string_pretty(record)
            .with_context(|| format!("Failed to serialize function: {name}"))?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write function file: {path:?}"))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl FunctionStorage for FileSystemStorage {
    async fn store(&self, name: &str, record: FunctionRecord) -> Result<()> {
        // 保存到内存
        {
            let mut functions = self.functions.write().await;
            functions.insert(name.to_string(), record.clone());
        }

        // 持久化到磁盘
        self.save_function_to_file(name, &record).await?;

        tracing::debug!("Stored function: {}", name);
        Ok(())
    }

    async fn load(&self, name: &str) -> Result<Option<FunctionRecord>> {
        // 先从内存加载
        {
            let functions = self.functions.read().await;
            if let Some(record) = functions.get(name) {
                return Ok(Some(record.clone()));
            }
        }

        // 从磁盘加载
        let path = self.get_function_file_path(name);
        if path.exists() {
            match self.load_function_from_file(&path).await {
                Ok(record) => {
                    // 加载到内存
                    {
                        let mut functions = self.functions.write().await;
                        functions.insert(name.to_string(), record.clone());
                    }
                    Ok(Some(record))
                }
                Err(e) => {
                    tracing::warn!("Failed to load function {} from disk: {}", name, e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, name: &str) -> Result<bool> {
        // 从内存删除
        let existed = {
            let mut functions = self.functions.write().await;
            functions.remove(name).is_some()
        };

        // 从磁盘删除
        let path = self.get_function_file_path(name);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete function file: {path:?}"))?;
        }

        if existed {
            tracing::debug!("Deleted function: {}", name);
        }

        Ok(existed)
    }

    async fn list(&self) -> Result<Vec<String>> {
        // 先确保从磁盘加载所有函数
        self.load_from_disk().await?;

        let functions = self.functions.read().await;
        Ok(functions.keys().cloned().collect())
    }

    async fn exists(&self, name: &str) -> Result<bool> {
        // 检查内存
        {
            let functions = self.functions.read().await;
            if functions.contains_key(name) {
                return Ok(true);
            }
        }

        // 检查磁盘
        let path = self.get_function_file_path(name);
        Ok(path.exists())
    }

    async fn get_version(&self, name: &str) -> Result<Option<String>> {
        if let Some(record) = self.load(name).await? {
            Ok(Some(record.version))
        } else {
            Ok(None)
        }
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>> {
        // 简单实现：只返回当前版本
        // 在更复杂的实现中，可以支持多版本存储
        if let Some(version) = self.get_version(name).await? {
            Ok(vec![version])
        } else {
            Ok(vec![])
        }
    }

    async fn backup(&self) -> Result<PathBuf> {
        let backup_path = self.get_backup_path();
        let functions = self.functions.read().await;

        let backup_data =
            serde_json::to_string_pretty(&*functions).context("Failed to serialize backup data")?;

        fs::write(&backup_path, backup_data)
            .with_context(|| format!("Failed to write backup file: {backup_path:?}"))?;

        tracing::info!("Created backup at: {:?}", backup_path);
        Ok(backup_path)
    }

    async fn restore(&self, backup_path: &Path) -> Result<()> {
        let content = fs::read_to_string(backup_path)
            .with_context(|| format!("Failed to read backup file: {backup_path:?}"))?;

        let backup_functions: HashMap<String, FunctionRecord> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse backup file: {backup_path:?}"))?;

        // 恢复到内存
        {
            let mut functions = self.functions.write().await;
            *functions = backup_functions.clone();
        }

        // 恢复到磁盘
        for (name, record) in backup_functions {
            self.save_function_to_file(&name, &record).await?;
        }

        tracing::info!("Restored from backup: {:?}", backup_path);
        Ok(())
    }
}

/// 内存存储实现（用于测试和临时存储）
pub struct MemoryStorage {
    functions: Arc<RwLock<HashMap<String, FunctionRecord>>>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl FunctionStorage for MemoryStorage {
    async fn store(&self, name: &str, record: FunctionRecord) -> Result<()> {
        let mut functions = self.functions.write().await;
        functions.insert(name.to_string(), record);
        Ok(())
    }

    async fn load(&self, name: &str) -> Result<Option<FunctionRecord>> {
        let functions = self.functions.read().await;
        Ok(functions.get(name).cloned())
    }

    async fn delete(&self, name: &str) -> Result<bool> {
        let mut functions = self.functions.write().await;
        Ok(functions.remove(name).is_some())
    }

    async fn list(&self) -> Result<Vec<String>> {
        let functions = self.functions.read().await;
        Ok(functions.keys().cloned().collect())
    }

    async fn exists(&self, name: &str) -> Result<bool> {
        let functions = self.functions.read().await;
        Ok(functions.contains_key(name))
    }

    async fn get_version(&self, name: &str) -> Result<Option<String>> {
        let functions = self.functions.read().await;
        Ok(functions.get(name).map(|r| r.version.clone()))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>> {
        if let Some(version) = self.get_version(name).await? {
            Ok(vec![version])
        } else {
            Ok(vec![])
        }
    }

    async fn backup(&self) -> Result<PathBuf> {
        // 内存存储不支持备份到文件
        Err(
            FluxError::StorageError("Memory storage does not support file backup".to_string())
                .into(),
        )
    }

    async fn restore(&self, _backup_path: &Path) -> Result<()> {
        // 内存存储不支持从文件恢复
        Err(
            FluxError::StorageError("Memory storage does not support file restore".to_string())
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::new();
        let metadata = FunctionMetadata::new(
            "test".to_string(),
            "fn test() -> String { \"Hello\".to_string() }".to_string(),
            None,
        );

        let record = FunctionRecord::new(
            metadata,
            "fn test() -> String { \"Hello\".to_string() }".to_string(),
            None,
            "1.0.0".to_string(),
            vec![],
        );

        // 测试存储
        storage.store("test", record.clone()).await.unwrap();

        // 测试加载
        let loaded = storage.load("test").await.unwrap().unwrap();
        assert_eq!(loaded.metadata.name, "test");

        // 测试存在性检查
        assert!(storage.exists("test").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // 测试删除
        assert!(storage.delete("test").await.unwrap());
        assert!(!storage.exists("test").await.unwrap());
    }

    #[tokio::test]
    async fn test_filesystem_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let metadata = FunctionMetadata::new(
            "test".to_string(),
            "fn test() -> String { \"Hello\".to_string() }".to_string(),
            None,
        );

        let record = FunctionRecord::new(
            metadata,
            "fn test() -> String { \"Hello\".to_string() }".to_string(),
            None,
            "1.0.0".to_string(),
            vec![],
        );

        // 测试存储和持久化
        storage.store("test", record.clone()).await.unwrap();

        // 创建新的存储实例来测试持久化
        let storage2 = FileSystemStorage::new(temp_dir.path().to_path_buf()).unwrap();
        let loaded = storage2.load("test").await.unwrap().unwrap();
        assert_eq!(loaded.metadata.name, "test");
    }
}
