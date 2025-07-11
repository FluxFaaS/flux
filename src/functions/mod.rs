use chrono::{DateTime, Utc};
use scru128::Scru128Id;
use serde::{Deserialize, Serialize};

pub mod registry;

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
    #[allow(dead_code)]
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
}

/// 函数注册请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFunctionRequest {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub timeout_ms: Option<u64>,
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
    #[allow(dead_code)]
    Timeout,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, FluxError>;

impl FunctionMetadata {
    #[allow(dead_code)]
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
        }
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
        }
    }
}
