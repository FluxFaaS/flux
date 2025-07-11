#![allow(clippy::uninlined_format_args)]

use crate::functions::{FluxError, Result};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// 函数代码验证器
#[derive(Debug, Clone)]
pub struct FunctionValidator {
    /// 允许的函数名模式
    #[allow(dead_code)]
    allowed_patterns: Vec<String>,
    /// 禁止的关键字
    forbidden_keywords: HashSet<String>,
    /// 禁止的模块路径
    forbidden_modules: HashSet<String>,
    /// 最大代码长度（字符数）
    max_code_length: usize,
    /// 最大函数复杂度
    max_complexity: usize,
    /// 验证超时时间
    validation_timeout: Duration,
}

/// 函数复杂度分析结果
#[derive(Debug, Clone)]
pub struct ComplexityAnalysis {
    /// 函数数量
    #[allow(dead_code)]
    pub function_count: usize,
    /// 循环数量
    #[allow(dead_code)]
    pub loop_count: usize,
    /// 条件分支数量
    #[allow(dead_code)]
    pub branch_count: usize,
    /// 嵌套深度
    #[allow(dead_code)]
    pub nesting_depth: usize,
    /// 总复杂度分数
    pub complexity_score: usize,
}

/// 安全检查结果
#[derive(Debug, Clone)]
pub struct SecurityAnalysis {
    /// 发现的安全问题
    pub security_issues: Vec<SecurityIssue>,
    /// 风险等级
    pub risk_level: RiskLevel,
    /// 是否通过安全检查
    pub is_safe: bool,
}

/// 安全问题类型
#[derive(Debug, Clone)]
pub struct SecurityIssue {
    /// 问题类型
    pub issue_type: SecurityIssueType,
    /// 问题描述
    #[allow(dead_code)]
    pub description: String,
    /// 发现位置（行号）
    #[allow(dead_code)]
    pub line_number: Option<usize>,
    /// 问题代码片段
    #[allow(dead_code)]
    pub code_snippet: Option<String>,
}

/// 安全问题类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityIssueType {
    /// 危险系统调用
    DangerousSystemCall,
    /// 文件系统访问
    FileSystemAccess,
    /// 网络访问
    NetworkAccess,
    /// 不安全代码块
    UnsafeCode,
    /// 外部命令执行
    CommandExecution,
    /// 内存操作
    MemoryOperation,
    /// 动态代码执行
    DynamicExecution,
}

/// 风险等级
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum RiskLevel {
    /// 低风险
    Low,
    /// 中等风险
    Medium,
    /// 高风险
    High,
    /// 极高风险
    Critical,
}

impl FunctionValidator {
    /// 创建新的函数验证器
    pub fn new() -> Self {
        let mut forbidden_keywords = HashSet::new();

        // 添加危险的关键字
        let dangerous_keywords = [
            "unsafe",
            "std::process",
            "std::fs::remove",
            "std::fs::write",
            "std::fs::create_dir",
            "std::env",
            "std::ptr",
            "libc::",
            "winapi::",
            "std::ffi",
            "std::os",
            "std::mem::transmute",
            "std::process::Command",
            "std::process::Stdio",
            "tokio::process",
            "std::net",
            "tokio::net",
            "std::thread::spawn",
            "std::sync::mpsc",
            "std::fs::File::create",
            "std::fs::File::open",
            "std::fs::OpenOptions",
            "std::fs::copy",
            "std::fs::rename",
            "std::path::Path::new",
            "include_str!",
            "include_bytes!",
            "include!",
            "concat!",
            "std::arch",
            "core::arch",
            "asm!",
            "global_asm!",
        ];

        for keyword in &dangerous_keywords {
            forbidden_keywords.insert(keyword.to_string());
        }

        let mut forbidden_modules = HashSet::new();
        let dangerous_modules = [
            "std::process",
            "std::fs",
            "std::env",
            "std::ptr",
            "std::mem",
            "std::ffi",
            "std::os",
            "std::net",
            "std::thread",
            "std::sync",
            "std::arch",
            "core::arch",
            "libc",
            "winapi",
            "nix",
            "tokio::process",
            "tokio::net",
            "tokio::fs",
        ];

        for module in &dangerous_modules {
            forbidden_modules.insert(module.to_string());
        }

        Self {
            allowed_patterns: vec!["fn ".to_string()],
            forbidden_keywords,
            forbidden_modules,
            max_code_length: 10000, // 10KB max
            max_complexity: 50,
            validation_timeout: Duration::from_secs(5),
        }
    }

    /// 创建自定义配置的验证器
    #[allow(dead_code)]
    pub fn with_config(
        max_code_length: usize,
        max_complexity: usize,
        validation_timeout_secs: u64,
    ) -> Self {
        let mut validator = Self::new();
        validator.max_code_length = max_code_length;
        validator.max_complexity = max_complexity;
        validator.validation_timeout = Duration::from_secs(validation_timeout_secs);
        validator
    }

    /// 完整验证函数代码
    pub async fn validate(&self, code: &str) -> Result<()> {
        let start_time = Instant::now();

        // 基础检查
        self.basic_validation(code)?;

        // 安全性检查
        let security_analysis = self.security_analysis(code)?;
        if !security_analysis.is_safe {
            return Err(FluxError::ValidationError {
                reason: format!(
                    "Security validation failed: {} issues found, risk level: {:?}",
                    security_analysis.security_issues.len(),
                    security_analysis.risk_level
                ),
            });
        }

        // 复杂度分析
        let complexity = self.complexity_analysis(code)?;
        if complexity.complexity_score > self.max_complexity {
            return Err(FluxError::ValidationError {
                reason: format!(
                    "Code complexity too high: {} (max: {})",
                    complexity.complexity_score, self.max_complexity
                ),
            });
        }

        // 检查验证时间
        if start_time.elapsed() > self.validation_timeout {
            return Err(FluxError::ValidationError {
                reason: "Validation timeout".to_string(),
            });
        }

        tracing::info!(
            "Function validation passed - Security: {:?}, Complexity: {}, Time: {:?}",
            security_analysis.risk_level,
            complexity.complexity_score,
            start_time.elapsed()
        );

        Ok(())
    }

    /// 基础代码验证
    fn basic_validation(&self, code: &str) -> Result<()> {
        // 检查代码长度
        if code.len() > self.max_code_length {
            return Err(FluxError::ValidationError {
                reason: format!(
                    "Code too long: {} characters (max: {})",
                    code.len(),
                    self.max_code_length
                ),
            });
        }

        // 检查是否为空
        if code.trim().is_empty() {
            return Err(FluxError::ValidationError {
                reason: "Function code cannot be empty".to_string(),
            });
        }

        // 检查是否包含函数定义
        if !code.contains("fn ") {
            return Err(FluxError::ValidationError {
                reason: "Code must contain at least one function definition".to_string(),
            });
        }

        // 检查括号匹配
        self.check_bracket_balance(code)?;

        Ok(())
    }

    /// 安全性分析
    fn security_analysis(&self, code: &str) -> Result<SecurityAnalysis> {
        let mut security_issues = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        for (line_number, line) in lines.iter().enumerate() {
            let line_trimmed = line.trim();

            // 检查禁用关键字
            for forbidden in &self.forbidden_keywords {
                if line_trimmed.contains(forbidden) {
                    security_issues.push(SecurityIssue {
                        issue_type: self.classify_security_issue(forbidden),
                        description: format!("Forbidden keyword detected: {}", forbidden),
                        line_number: Some(line_number + 1),
                        code_snippet: Some(line_trimmed.to_string()),
                    });
                }
            }

            // 检查禁用模块
            for module in &self.forbidden_modules {
                if line_trimmed.contains(&format!("use {};", module))
                    || line_trimmed.contains(&format!("{}::", module))
                {
                    security_issues.push(SecurityIssue {
                        issue_type: SecurityIssueType::DangerousSystemCall,
                        description: format!("Forbidden module usage: {}", module),
                        line_number: Some(line_number + 1),
                        code_snippet: Some(line_trimmed.to_string()),
                    });
                }
            }

            // 检查特殊模式
            self.check_special_patterns(line_trimmed, line_number + 1, &mut security_issues);
        }

        let risk_level = self.assess_risk_level(&security_issues);
        let is_safe = matches!(risk_level, RiskLevel::Low) && security_issues.len() < 3;

        Ok(SecurityAnalysis {
            security_issues,
            risk_level,
            is_safe,
        })
    }

    /// 复杂度分析
    fn complexity_analysis(&self, code: &str) -> Result<ComplexityAnalysis> {
        let lines: Vec<&str> = code.lines().collect();
        let mut function_count = 0;
        let mut loop_count = 0;
        let mut branch_count = 0;
        let mut max_nesting = 0;
        let mut current_nesting = 0;

        for line in lines {
            let line_trimmed = line.trim();

            // 统计函数
            if line_trimmed.starts_with("fn ") || line_trimmed.contains("fn ") {
                function_count += 1;
            }

            // 统计循环
            if line_trimmed.starts_with("for ")
                || line_trimmed.starts_with("while ")
                || line_trimmed.starts_with("loop")
            {
                loop_count += 1;
                current_nesting += 1;
            }

            // 统计分支
            if line_trimmed.starts_with("if ")
                || line_trimmed.starts_with("match ")
                || line_trimmed.contains("else")
            {
                branch_count += 1;
                current_nesting += 1;
            }

            // 统计嵌套深度
            let open_braces = line_trimmed.chars().filter(|&c| c == '{').count();
            let close_braces = line_trimmed.chars().filter(|&c| c == '}').count();
            current_nesting += open_braces;
            current_nesting = current_nesting.saturating_sub(close_braces);
            max_nesting = max_nesting.max(current_nesting);
        }

        // 计算复杂度分数
        let complexity_score = function_count * 2 + loop_count * 3 + branch_count * 2 + max_nesting;

        Ok(ComplexityAnalysis {
            function_count,
            loop_count,
            branch_count,
            nesting_depth: max_nesting,
            complexity_score,
        })
    }

    /// 检查括号匹配
    fn check_bracket_balance(&self, code: &str) -> Result<()> {
        let mut stack = Vec::new();
        let pairs = [('(', ')'), ('[', ']'), ('{', '}')];

        for (i, ch) in code.chars().enumerate() {
            match ch {
                '(' | '[' | '{' => stack.push((ch, i)),
                ')' | ']' | '}' => {
                    if let Some((open, _)) = stack.pop() {
                        let expected = pairs
                            .iter()
                            .find(|(o, _)| *o == open)
                            .map(|(_, c)| *c)
                            .unwrap();
                        if ch != expected {
                            return Err(FluxError::ValidationError {
                                reason: format!("Mismatched brackets at position {}", i),
                            });
                        }
                    } else {
                        return Err(FluxError::ValidationError {
                            reason: format!("Unmatched closing bracket at position {}", i),
                        });
                    }
                }
                _ => {}
            }
        }

        if !stack.is_empty() {
            return Err(FluxError::ValidationError {
                reason: format!("Unclosed brackets: {} remaining", stack.len()),
            });
        }

        Ok(())
    }

    /// 分类安全问题
    fn classify_security_issue(&self, keyword: &str) -> SecurityIssueType {
        match keyword {
            s if s.contains("process") => SecurityIssueType::CommandExecution,
            s if s.contains("fs") || s.contains("File") => SecurityIssueType::FileSystemAccess,
            s if s.contains("net") => SecurityIssueType::NetworkAccess,
            "unsafe" => SecurityIssueType::UnsafeCode,
            s if s.contains("ptr") || s.contains("mem") => SecurityIssueType::MemoryOperation,
            _ => SecurityIssueType::DangerousSystemCall,
        }
    }

    /// 检查特殊模式
    fn check_special_patterns(
        &self,
        line: &str,
        line_number: usize,
        issues: &mut Vec<SecurityIssue>,
    ) {
        // 检查动态执行模式
        if line.contains("eval(") || line.contains("exec(") {
            issues.push(SecurityIssue {
                issue_type: SecurityIssueType::DynamicExecution,
                description: "Dynamic code execution detected".to_string(),
                line_number: Some(line_number),
                code_snippet: Some(line.to_string()),
            });
        }

        // 检查外部命令执行
        if line.contains("system(") || line.contains("exec(") {
            issues.push(SecurityIssue {
                issue_type: SecurityIssueType::CommandExecution,
                description: "External command execution detected".to_string(),
                line_number: Some(line_number),
                code_snippet: Some(line.to_string()),
            });
        }
    }

    /// 评估风险等级
    fn assess_risk_level(&self, issues: &[SecurityIssue]) -> RiskLevel {
        if issues.is_empty() {
            return RiskLevel::Low;
        }

        let critical_issues = issues
            .iter()
            .filter(|issue| {
                matches!(
                    issue.issue_type,
                    SecurityIssueType::UnsafeCode
                        | SecurityIssueType::CommandExecution
                        | SecurityIssueType::DynamicExecution
                )
            })
            .count();

        let high_risk_issues = issues
            .iter()
            .filter(|issue| {
                matches!(
                    issue.issue_type,
                    SecurityIssueType::FileSystemAccess
                        | SecurityIssueType::NetworkAccess
                        | SecurityIssueType::MemoryOperation
                )
            })
            .count();

        if critical_issues > 0 {
            RiskLevel::Critical
        } else if high_risk_issues > 2 {
            RiskLevel::High
        } else if issues.len() > 5 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

impl Default for FunctionValidator {
    fn default() -> Self {
        Self::new()
    }
}
