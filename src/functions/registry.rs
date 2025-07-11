use super::{FluxError, FunctionMetadata, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 函数注册表 - 内存中存储函数元数据
#[derive(Debug, Clone)]
pub struct FunctionRegistry {
    functions: Arc<RwLock<HashMap<String, FunctionMetadata>>>,
}

impl FunctionRegistry {
    /// 创建新的函数注册表
    pub fn new() -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册函数
    pub async fn register(&self, function: FunctionMetadata) -> Result<()> {
        let mut functions = self.functions.write().await;

        if functions.contains_key(&function.name) {
            return Err(FluxError::FunctionAlreadyExists {
                name: function.name.clone(),
            });
        }

        tracing::info!("Registering function: {}", function.name);
        functions.insert(function.name.clone(), function);
        Ok(())
    }

    /// 获取函数
    pub async fn get(&self, name: &str) -> Result<FunctionMetadata> {
        let functions = self.functions.read().await;
        functions
            .get(name)
            .cloned()
            .ok_or_else(|| FluxError::FunctionNotFound {
                name: name.to_string(),
            })
    }

    /// 列出所有函数
    pub async fn list(&self) -> Vec<FunctionMetadata> {
        let functions = self.functions.read().await;
        functions.values().cloned().collect()
    }

    /// 删除函数
    #[allow(dead_code)]
    pub async fn remove(&self, name: &str) -> Result<()> {
        let mut functions = self.functions.write().await;
        functions
            .remove(name)
            .ok_or_else(|| FluxError::FunctionNotFound {
                name: name.to_string(),
            })?;

        tracing::info!("Removed function: {}", name);
        Ok(())
    }

    /// 函数是否存在
    #[allow(dead_code)]
    pub async fn exists(&self, name: &str) -> bool {
        let functions = self.functions.read().await;
        functions.contains_key(name)
    }

    /// 获取函数数量
    #[allow(dead_code)]
    pub async fn count(&self) -> usize {
        let functions = self.functions.read().await;
        functions.len()
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
