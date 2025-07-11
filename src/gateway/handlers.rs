use crate::functions::{FunctionMetadata, InvokeRequest, RegisterFunctionRequest};
use crate::scheduler::{SimpleScheduler, Scheduler};
use silent::{Request, Response, Result as SilentResult};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// 从文件加载函数的请求
#[derive(Debug, Deserialize)]
pub struct LoadFileRequest {
    pub file_path: String,
    pub function_name: Option<String>,
    pub description: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// 从目录加载函数的请求
#[derive(Debug, Deserialize)]
pub struct LoadDirectoryRequest {
    pub directory_path: String,
}

/// API响应的通用格式
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub message: Option<String>,
}

/// 健康检查
pub async fn health_check(_req: Request) -> SilentResult<Response> {
    let response = serde_json::json!({
        "status": "healthy",
        "service": "FluxFaaS",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Response::json(response))
}

/// 注册函数
pub async fn register_function(
    mut req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let register_req: RegisterFunctionRequest = req.json().await?;

    tracing::info!("Registering function: {}", register_req.name);

    let function = FunctionMetadata::from_request(register_req);

    match scheduler.registry().register(function.clone()).await {
        Ok(_) => {
            tracing::info!("Function {} registered successfully", function.name);
            let response = serde_json::json!({
                "success": true,
                "message": format!("Function '{}' registered successfully", function.name),
                "function": function
            });
            Ok(Response::json(response))
        }
        Err(e) => {
            tracing::error!("Failed to register function {}: {}", function.name, e);
            let response = serde_json::json!({
                "success": false,
                "error": e.to_string()
            });
            Ok(Response::json(response).with_status(400))
        }
    }
}

/// 列出所有函数
pub async fn list_functions(
    _req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let functions = scheduler.registry().list().await;

    let response = serde_json::json!({
        "functions": functions,
        "count": functions.len()
    });

    Ok(Response::json(response))
}

/// 获取单个函数信息
pub async fn get_function(
    req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let function_name = req.param("name").unwrap_or("");

    match scheduler.registry().get(function_name).await {
        Ok(function) => {
            Ok(Response::json(function))
        }
        Err(e) => {
            let response = serde_json::json!({
                "error": e.to_string()
            });
            Ok(Response::json(response).with_status(404))
        }
    }
}

/// 删除函数
pub async fn delete_function(
    req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let function_name = req.param("name").unwrap_or("");

    match scheduler.registry().remove(function_name).await {
        Ok(_) => {
            let response = serde_json::json!({
                "success": true,
                "message": format!("Function '{}' deleted successfully", function_name)
            });
            Ok(Response::json(response))
        }
        Err(e) => {
            let response = serde_json::json!({
                "error": e.to_string()
            });
            Ok(Response::json(response).with_status(404))
        }
    }
}

/// 调用函数
pub async fn invoke_function(
    mut req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let function_name = req.param("name").unwrap_or("");

    if function_name.is_empty() {
        let response = serde_json::json!({
            "error": "Function name is required"
        });
        return Ok(Response::json(response).with_status(400));
    }

    let invoke_req: InvokeRequest = req.json().await?;

    tracing::info!("Invoking function: {}", function_name);

    match scheduler.schedule(function_name, invoke_req).await {
        Ok(response_data) => {
            tracing::info!("Function {} invoked successfully", function_name);
            Ok(Response::json(response_data))
        }
        Err(e) => {
            tracing::error!("Failed to invoke function {}: {}", function_name, e);
            let response = serde_json::json!({
                "error": e.to_string()
            });

            let status_code = match &e {
                crate::functions::FluxError::FunctionNotFound { .. } => 404,
                crate::functions::FluxError::Timeout => 408,
                _ => 500,
            };

            Ok(Response::json(response).with_status(status_code))
        }
    }
}

/// 获取系统状态
pub async fn get_status(
    _req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let function_count = scheduler.registry().count().await;

    let response = serde_json::json!({
        "service": "FluxFaaS",
        "version": "0.1.0",
        "status": "running",
        "functions": {
            "total": function_count,
        },
        "runtime": {
            "type": "SimpleRuntime",
            "isolation": "process"
        },
        "scheduler": {
            "type": "SimpleScheduler",
            "strategy": "immediate"
        },
        "uptime": "N/A", // TODO: 实现运行时间计算
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Response::json(response))
}

/// 从文件加载函数
pub async fn load_function_from_file(
    mut req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let load_req: LoadFileRequest = req.json().await?;

    tracing::info!("Loading function from file: {}", load_req.file_path);

    match scheduler.registry().register_from_file(
        &load_req.file_path,
        load_req.function_name,
        load_req.description,
        load_req.timeout_ms,
    ).await {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "file_path": load_req.file_path,
                    "message": "Function loaded successfully"
                })),
                error: None,
                message: Some("Function loaded and registered successfully".to_string()),
            };
            Ok(Response::json(response))
        }
        Err(e) => {
            tracing::error!("Failed to load function from file {}: {}", load_req.file_path, e);
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
                message: Some("Failed to load function from file".to_string()),
            };
            Ok(Response::json(response).with_status(400))
        }
    }
}

/// 从目录批量加载函数
pub async fn load_functions_from_directory(
    mut req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let load_req: LoadDirectoryRequest = req.json().await?;

    tracing::info!("Loading functions from directory: {}", load_req.directory_path);

    match scheduler.registry().register_from_directory(&load_req.directory_path).await {
        Ok(count) => {
            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "directory_path": load_req.directory_path,
                    "functions_loaded": count,
                    "message": format!("Successfully loaded {} functions", count)
                })),
                error: None,
                message: Some(format!("Successfully loaded {} functions from directory", count)),
            };
            Ok(Response::json(response))
        }
        Err(e) => {
            tracing::error!("Failed to load functions from directory {}: {}", load_req.directory_path, e);
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
                message: Some("Failed to load functions from directory".to_string()),
            };
            Ok(Response::json(response).with_status(400))
        }
    }
}

/// 获取缓存统计
pub async fn get_cache_stats(
    _req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let cache = scheduler.runtime().cache();
    let stats = cache.stats().await;
    let hit_rate = cache.hit_rate().await;

    let usage_percent = if stats.max_memory > 0 {
        (stats.memory_usage as f64 / stats.max_memory as f64) * 100.0
    } else {
        0.0
    };

    let response_data = serde_json::json!({
        "basic_stats": {
            "hit_rate": hit_rate,
            "hits": stats.hits,
            "misses": stats.misses,
            "size": stats.size,
            "evictions": stats.evictions
        },
        "memory_usage": {
            "current_bytes": stats.memory_usage,
            "current_kb": stats.memory_usage as f64 / 1024.0,
            "max_bytes": stats.max_memory,
            "max_mb": stats.max_memory as f64 / (1024.0 * 1024.0),
            "usage_percent": usage_percent
        },
        "config": {
            "cache_type": "LRU",
            "max_capacity": 100,
            "expiry_hours": 1,
            "status": "active"
        }
    });

    let response = ApiResponse {
        success: true,
        data: Some(response_data),
        error: None,
        message: Some("Cache statistics retrieved successfully".to_string()),
    };

    Ok(Response::json(response))
}

/// 获取性能监控数据
pub async fn get_performance_monitor(
    _req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let monitor = scheduler.runtime().monitor();
    let report = monitor.generate_report().await;

    // 获取热点函数
    let hottest = monitor.get_hottest_functions(5).await;
    let slowest = monitor.get_slowest_functions(5).await;
    let error_prone = monitor.get_error_prone_functions(5).await;

    let health_status = match report.health_status {
        crate::runtime::monitor::HealthStatus::Healthy => "healthy",
        crate::runtime::monitor::HealthStatus::Warning => "warning",
        crate::runtime::monitor::HealthStatus::Critical => "critical",
    };

    let uptime_seconds = if let Some(start_time) = report.global_stats.start_time {
        report.generated_at.duration_since(start_time).as_secs_f64()
    } else {
        0.0
    };

    let success_rate = if report.global_stats.total_requests > 0 {
        (report.global_stats.total_success as f64 / report.global_stats.total_requests as f64) * 100.0
    } else {
        0.0
    };

    let response_data = serde_json::json!({
        "health_status": health_status,
        "global_stats": {
            "total_requests": report.global_stats.total_requests,
            "total_success": report.global_stats.total_success,
            "total_failures": report.global_stats.total_failures,
            "success_rate": success_rate,
            "uptime_seconds": uptime_seconds,
            "peak_memory_kb": report.global_stats.peak_system_memory as f64 / 1024.0,
            "current_memory_kb": report.global_stats.current_system_memory as f64 / 1024.0
        },
        "function_stats": report.function_stats,
        "hottest_functions": hottest.into_iter().map(|(name, calls)| {
            serde_json::json!({"name": name, "calls": calls})
        }).collect::<Vec<_>>(),
        "slowest_functions": slowest.into_iter().map(|(name, duration)| {
            serde_json::json!({"name": name, "avg_duration_ms": duration.as_millis()})
        }).collect::<Vec<_>>(),
        "error_prone_functions": error_prone.into_iter().map(|(name, error_rate)| {
            serde_json::json!({"name": name, "error_rate": error_rate * 100.0})
        }).collect::<Vec<_>>(),
        "recommendations": report.recommendations
    });

    let response = ApiResponse {
        success: true,
        data: Some(response_data),
        error: None,
        message: Some("Performance monitoring data retrieved successfully".to_string()),
    };

    Ok(Response::json(response))
}

/// 重置性能监控数据
pub async fn reset_performance_data(
    _req: Request,
    scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    let monitor = scheduler.runtime().monitor();

    match monitor.reset_stats().await {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "message": "Performance monitoring data has been reset",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                error: None,
                message: Some("Performance monitoring data reset successfully".to_string()),
            };
            Ok(Response::json(response))
        }
        Err(e) => {
            tracing::error!("Failed to reset performance data: {}", e);
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
                message: Some("Failed to reset performance monitoring data".to_string()),
            };
            Ok(Response::json(response).with_status(500))
        }
    }
}

/// SCRU128功能演示
pub async fn demonstrate_scru128(
    _req: Request,
    _scheduler: Arc<SimpleScheduler>,
) -> SilentResult<Response> {
    use crate::functions::{FunctionMetadata, RegisterFunctionRequest};

    // 生成一些示例SCRU128 ID和函数
    let mut generated_functions = Vec::new();

    for i in 1..=5 {
        let request = RegisterFunctionRequest {
            name: format!("demo_function_{}", i),
            description: Some(format!("Demo function {} for SCRU128 showcase", i)),
            code: format!("return \"Demo function {} result\"", i),
            timeout_ms: Some(5000),
        };

        let metadata = FunctionMetadata::from_request(request);
        generated_functions.push(serde_json::json!({
            "name": metadata.name,
            "id": metadata.id.to_string(),
            "created_at": metadata.created_at
        }));

        // 短暂延迟确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    // 验证排序特性
    let id_strings: Vec<String> = generated_functions
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect();
    let mut sorted_ids = id_strings.clone();
    sorted_ids.sort();

    let is_time_ordered = id_strings == sorted_ids;

    let response_data = serde_json::json!({
        "generated_functions": generated_functions,
        "analysis": {
            "id_length": if !id_strings.is_empty() { id_strings[0].len() } else { 0 },
            "encoding": "Base32",
            "contains_timestamp": true,
            "supports_sorting": true,
            "time_ordered": is_time_ordered
        },
        "advantages": [
            "比 UUID 更短（25 vs 36 字符）",
            "时间有序，数据库索引友好",
            "分布式环境安全",
            "URL 友好，无需转义"
        ],
        "note": "这些是演示函数，不会实际注册到系统中"
    });

    let response = ApiResponse {
        success: true,
        data: Some(response_data),
        error: None,
        message: Some("SCRU128 demonstration completed successfully".to_string()),
    };

    Ok(Response::json(response))
}
