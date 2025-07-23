#![allow(dead_code)]
use chrono::{DateTime, Utc};
use scru128::Scru128Id;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod registry;
pub mod storage;
pub mod watcher;

/// 脚本类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScriptType {
    /// Rust 代码
    #[default]
    Rust,
    /// Python 代码
    Python,
    /// JavaScript 代码
    JavaScript,
    /// TypeScript 代码
    TypeScript,
}

/// 函数返回类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReturnType {
    /// 字符串类型
    String,
    /// 整数类型
    Integer,
    /// 浮点数类型
    Float,
    /// 布尔类型
    Boolean,
    /// JSON 对象类型
    Object,
    /// JSON 数组类型
    Array,
    /// 无返回值
    Void,
    /// 任意 JSON 值（兼容性保留）
    #[default]
    Any,
}

impl fmt::Display for ReturnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReturnType::String => write!(f, "string"),
            ReturnType::Integer => write!(f, "integer"),
            ReturnType::Float => write!(f, "float"),
            ReturnType::Boolean => write!(f, "boolean"),
            ReturnType::Object => write!(f, "object"),
            ReturnType::Array => write!(f, "array"),
            ReturnType::Void => write!(f, "void"),
            ReturnType::Any => write!(f, "any"),
        }
    }
}


impl ReturnType {
    /// 从字符串解析返回类型
    pub fn parse_from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "string" | "str" => Some(ReturnType::String),
            "integer" | "int" | "i32" | "i64" => Some(ReturnType::Integer),
            "float" | "f32" | "f64" | "number" => Some(ReturnType::Float),
            "boolean" | "bool" => Some(ReturnType::Boolean),
            "object" | "json" => Some(ReturnType::Object),
            "array" | "list" => Some(ReturnType::Array),
            "void" | "unit" | "()" => Some(ReturnType::Void),
            "any" | "serde_json::value" => Some(ReturnType::Any),
            _ => None,
        }
    }

    /// 获取 Rust 类型字符串表示
    pub fn to_rust_type(&self) -> &'static str {
        match self {
            ReturnType::String => "String",
            ReturnType::Integer => "i64",
            ReturnType::Float => "f64",
            ReturnType::Boolean => "bool",
            ReturnType::Object => "serde_json::Map<String, serde_json::Value>",
            ReturnType::Array => "Vec<serde_json::Value>",
            ReturnType::Void => "()",
            ReturnType::Any => "serde_json::Value",
        }
    }
}

impl fmt::Display for ScriptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptType::Rust => write!(f, "rust"),
            ScriptType::Python => write!(f, "python"),
            ScriptType::JavaScript => write!(f, "javascript"),
            ScriptType::TypeScript => write!(f, "typescript"),
        }
    }
}


impl ScriptType {
    /// 从文件扩展名推断脚本类型
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(ScriptType::Rust),
            "py" => Some(ScriptType::Python),
            "js" => Some(ScriptType::JavaScript),
            "ts" => Some(ScriptType::TypeScript),
            _ => None,
        }
    }

    /// 从字符串解析脚本类型
    pub fn parse_from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(ScriptType::Rust),
            "python" | "py" => Some(ScriptType::Python),
            "javascript" | "js" => Some(ScriptType::JavaScript),
            "typescript" | "ts" => Some(ScriptType::TypeScript),
            _ => None,
        }
    }

    /// 获取文件扩展名
    pub fn file_extension(&self) -> &'static str {
        match self {
            ScriptType::Rust => "rs",
            ScriptType::Python => "py",
            ScriptType::JavaScript => "js",
            ScriptType::TypeScript => "ts",
        }
    }

    /// 检查是否支持编译
    pub fn supports_compilation(&self) -> bool {
        matches!(self, ScriptType::Rust | ScriptType::TypeScript)
    }

    /// 检查是否需要外部运行时
    pub fn requires_external_runtime(&self) -> bool {
        matches!(self, ScriptType::Python | ScriptType::JavaScript | ScriptType::TypeScript)
    }
}

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
    /// 执行完成（第三阶段新增）
    Completed,
    Error(String),
    /// 执行失败（第三阶段新增）
    Failed,
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
    pub return_type: ReturnType,
    /// 第四阶段新增：脚本类型
    pub script_type: ScriptType,
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
    pub return_type: Option<ReturnType>,
    /// 第四阶段新增：脚本类型（必填）
    pub script_type: ScriptType,
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
    pub fn new(name: String, code: String, script_type: Option<ScriptType>) -> Self {
        let now = Utc::now();
        let detected_script_type = script_type.unwrap_or_else(|| Self::detect_script_type_from_code(&code));

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
            return_type: ReturnType::default(),
            script_type: detected_script_type,
        }
    }

    /// 从代码内容自动检测脚本类型
    pub fn detect_script_type_from_code(code: &str) -> ScriptType {
        let code_lower = code.to_lowercase();

        // JavaScript 特征
        if code_lower.contains("function") ||
           code_lower.contains("const ") ||
           code_lower.contains("let ") ||
           code_lower.contains("var ") ||
           code_lower.contains("json.parse") ||
           code_lower.contains("=>") {
            return ScriptType::JavaScript;
        }

        // Python 特征
        if code_lower.contains("def ") ||
           code_lower.contains("import ") ||
           code_lower.contains("print(") ||
           (code.contains("    ") && code_lower.contains(":")) { // 缩进 + 冒号
            return ScriptType::Python;
        }

        // TypeScript 特征 (更复杂的类型声明)
        if code_lower.contains(": string") ||
           code_lower.contains(": number") ||
           code_lower.contains("interface ") ||
           code_lower.contains("type ") {
            return ScriptType::TypeScript;
        }

        // Rust 特征
        if code_lower.contains("fn ") ||
           code_lower.contains("let mut") ||
           code_lower.contains("match ") ||
           code_lower.contains("impl ") {
            return ScriptType::Rust;
        }

        // 默认为 JavaScript（用于简单表达式）
        ScriptType::JavaScript
    }

    /// 创建指定脚本类型的函数元数据
    pub fn new_with_script_type(name: String, code: String, script_type: ScriptType) -> Self {
        Self::new(name, code, Some(script_type))
    }

    /// 创建带版本的函数元数据
    pub fn new_with_version(name: String, code: String, version: String) -> Self {
        let mut metadata = Self::new(name, code, None);
        metadata.version = version;
        metadata
    }

    /// 创建带依赖的函数元数据
    pub fn new_with_dependencies(name: String, code: String, dependencies: Vec<String>) -> Self {
        let mut metadata = Self::new(name, code, None);
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
    pub fn set_return_type(&mut self, return_type: ReturnType) {
        self.return_type = return_type;
        self.updated_at = Utc::now();
    }

    /// 设置脚本类型
    pub fn set_script_type(&mut self, script_type: ScriptType) {
        self.script_type = script_type;
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
            return_type: req.return_type.unwrap_or_default(),
            script_type: req.script_type,
        }
    }
}
