use crate::functions::{FluxError, FunctionMetadata, RegisterFunctionRequest, Result, ScriptType};
use crate::runtime::validator::FunctionValidator;
use std::path::Path;
use tokio::fs;

/// 动态函数加载器
#[derive(Debug, Clone)]

pub struct FunctionLoader {
    validator: FunctionValidator,
}

impl FunctionLoader {
    /// 创建新的函数加载器
    pub fn new() -> Self {
        Self {
            validator: FunctionValidator::new(),
        }
    }

    /// 创建带自定义验证器的函数加载器
    pub fn with_validator(validator: FunctionValidator) -> Self {
        Self { validator }
    }

    /// 从文件路径加载函数代码
    pub async fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let path = path.as_ref();

        // 检查文件是否存在
        if !path.exists() {
            return Err(FluxError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Function file not found: {}", path.display()),
            )));
        }

        // 检查文件扩展名
        if let Some(ext) = path.extension() {
            if ext != "rs" {
                return Err(FluxError::ValidationError {
                    reason: format!("Only .rs files are supported, got: {}", path.display()),
                });
            }
        } else {
            return Err(FluxError::ValidationError {
                reason: format!("File must have .rs extension: {}", path.display()),
            });
        }

        // 读取文件内容
        let content = fs::read_to_string(path).await?;

        tracing::info!("Loaded function code from file: {}", path.display());
        Ok(content)
    }

    /// 从文件路径创建函数元数据
    pub async fn load_function_from_file<P: AsRef<Path>>(
        &self,
        path: P,
        name: Option<String>,
        description: Option<String>,
        timeout_ms: Option<u64>,
    ) -> Result<FunctionMetadata> {
        let path = path.as_ref();
        let code = self.load_from_file(path).await?;

        // 验证函数代码
        self.validate_function_code(&code).await?;

        // 如果没有提供名称，从文件名推断
        let function_name = name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        // 根据文件扩展名推断脚本类型
        let script_type = path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(ScriptType::from_extension)
            .unwrap_or(ScriptType::Rust); // 默认为 Rust

        let req = RegisterFunctionRequest {
            name: function_name,
            description,
            code,
            timeout_ms,
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
            script_type,
        };

        Ok(FunctionMetadata::from_request(req))
    }

    /// 从多个文件路径批量加载函数
    pub async fn load_functions_from_directory<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Vec<FunctionMetadata>> {
        let dir_path = dir_path.as_ref();

        if !dir_path.exists() {
            return Err(FluxError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {}", dir_path.display()),
            )));
        }

        if !dir_path.is_dir() {
            return Err(FluxError::ValidationError {
                reason: format!("Path is not a directory: {}", dir_path.display()),
            });
        }

        let mut functions = Vec::new();
        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // 只处理 .rs 文件
            if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                match self.load_function_from_file(&path, None, None, None).await {
                    Ok(function) => {
                        functions.push(function);
                        tracing::info!("Loaded function from: {}", path.display());
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load function from {}: {}", path.display(), e);
                        // 继续处理其他文件，不中断整个过程
                    }
                }
            }
        }

        tracing::info!(
            "Loaded {} functions from directory: {}",
            functions.len(),
            dir_path.display()
        );
        Ok(functions)
    }

    /// 验证函数代码
    pub async fn validate_function_code(&self, code: &str) -> Result<()> {
        self.validator.validate(code).await
    }
}

impl Default for FunctionLoader {
    fn default() -> Self {
        Self::new()
    }
}
