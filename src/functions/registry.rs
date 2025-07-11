use super::{FluxError, FunctionMetadata, Result};
use crate::runtime::loader::FunctionLoader;
use std::collections::HashMap;
use std::path::Path;
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

    /// 从文件注册函数
    pub async fn register_from_file<P: AsRef<Path>>(
        &self,
        path: P,
        name: Option<String>,
        description: Option<String>,
        timeout_ms: Option<u64>,
    ) -> Result<()> {
        let loader = FunctionLoader::new();
        let function = loader
            .load_function_from_file(path, name, description, timeout_ms)
            .await?;

        // 验证函数代码
        loader.validate_function_code(&function.code)?;

        self.register(function).await
    }

    /// 从目录批量注册函数
    pub async fn register_from_directory<P: AsRef<Path>>(&self, dir_path: P) -> Result<usize> {
        let loader = FunctionLoader::new();
        let functions = loader.load_functions_from_directory(dir_path).await?;

        let mut registered_count = 0;
        for function in functions {
            // 验证函数代码
            if let Err(e) = loader.validate_function_code(&function.code) {
                tracing::warn!(
                    "Skipping function {} due to validation error: {}",
                    function.name,
                    e
                );
                continue;
            }

            // 尝试注册函数，跳过已存在的函数
            match self.register(function).await {
                Ok(_) => {
                    registered_count += 1;
                }
                Err(FluxError::FunctionAlreadyExists { name }) => {
                    tracing::warn!("Function {} already exists, skipping", name);
                }
                Err(e) => {
                    tracing::error!("Failed to register function: {}", e);
                }
            }
        }

        tracing::info!("Registered {} functions from directory", registered_count);
        Ok(registered_count)
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
