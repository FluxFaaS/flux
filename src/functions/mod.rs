#![allow(dead_code)]
use chrono::{DateTime, Utc};
use scru128::Scru128Id;
use serde::{Deserialize, Serialize};

pub mod registry;
pub mod storage;
pub mod watcher;

/// 函数调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    pub input: serde_json::Value,
}

/// 函数调用响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    pub output: serde_json::Value,
    pub execution_time_ms: u64,
    pub status: ExecutionStatus,
}

/// 函数执行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Success,
    Error(String),

    Timeout,
}

/// 函数元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetadata {
    pub id: Scru128Id,
    pub name: String,
    pub description: String,
    pub code: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub timeout_ms: u64,
    /// 第二阶段新增：版本信息
    pub version: String,
    /// 第二阶段新增：依赖列表
    pub dependencies: Vec<String>,
    /// 第二阶段新增：函数参数信息
    pub parameters: Vec<FunctionParameter>,
    /// 第二阶段新增：返回类型
    pub return_type: String,
}

/// 函数参数信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub name: String,
    pub param_type: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<String>,
}

/// 函数注册请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFunctionRequest {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub timeout_ms: Option<u64>,
    /// 第二阶段新增：版本信息
    pub version: Option<String>,
    /// 第二阶段新增：依赖列表
    pub dependencies: Option<Vec<String>>,
    /// 第二阶段新增：函数参数信息
    pub parameters: Option<Vec<FunctionParameter>>,
    /// 第二阶段新增：返回类型
    pub return_type: Option<String>,
}

/// 系统错误类型
#[derive(Debug, thiserror::Error)]
pub enum FluxError {
    #[error("Function not found: {name}")]
    FunctionNotFound { name: String },

    #[error("Function already exists: {name}")]
    FunctionAlreadyExists { name: String },

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Execution timeout")]
    Timeout,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    // 第二阶段新增错误类型
    #[error("Function compilation failed: {0}")]
    CompilationError(String),

    #[error("Function validation failed: {reason}")]
    ValidationError { reason: String },

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

pub type Result<T> = std::result::Result<T, FluxError>;

impl FunctionMetadata {
    pub fn new(name: String, code: String) -> Self {
        let now = Utc::now();
        Self {
            id: scru128::new(),
            name,
            description: String::new(),
            code,
            created_at: now,
            updated_at: now,
            timeout_ms: 5000, // 默认 5 秒超时
            version: "1.0.0".to_string(),
            dependencies: Vec::new(),
            parameters: Vec::new(),
            return_type: "serde_json::Value".to_string(),
        }
    }

    /// 创建带版本的函数元数据
    pub fn new_with_version(name: String, code: String, version: String) -> Self {
        let mut metadata = Self::new(name, code);
        metadata.version = version;
        metadata
    }

    /// 创建带依赖的函数元数据
    pub fn new_with_dependencies(name: String, code: String, dependencies: Vec<String>) -> Self {
        let mut metadata = Self::new(name, code);
        metadata.dependencies = dependencies;
        metadata
    }

    /// 更新版本
    pub fn update_version(&mut self, new_version: String) {
        self.version = new_version;
        self.updated_at = Utc::now();
    }

    /// 添加依赖
    pub fn add_dependency(&mut self, dependency: String) {
        if !self.dependencies.contains(&dependency) {
            self.dependencies.push(dependency);
            self.updated_at = Utc::now();
        }
    }

    /// 移除依赖
    pub fn remove_dependency(&mut self, dependency: &str) {
        if let Some(pos) = self.dependencies.iter().position(|x| x == dependency) {
            self.dependencies.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// 设置参数信息
    pub fn set_parameters(&mut self, parameters: Vec<FunctionParameter>) {
        self.parameters = parameters;
        self.updated_at = Utc::now();
    }

    /// 设置返回类型
    pub fn set_return_type(&mut self, return_type: String) {
        self.return_type = return_type;
        self.updated_at = Utc::now();
    }

    pub fn from_request(req: RegisterFunctionRequest) -> Self {
        let now = Utc::now();
        Self {
            id: scru128::new(),
            name: req.name,
            description: req.description.unwrap_or_default(),
            code: req.code,
            created_at: now,
            updated_at: now,
            timeout_ms: req.timeout_ms.unwrap_or(5000),
            version: req.version.unwrap_or_else(|| "1.0.0".to_string()),
            dependencies: req.dependencies.unwrap_or_default(),
            parameters: req.parameters.unwrap_or_default(),
            return_type: req
                .return_type
                .unwrap_or_else(|| "serde_json::Value".to_string()),
        }
    }
}
