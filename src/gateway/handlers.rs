use crate::functions::{FunctionMetadata, InvokeRequest, RegisterFunctionRequest};
use crate::scheduler::{Scheduler, SimpleScheduler};
use serde::{Deserialize, Serialize};
use silent::{Request, Response, Result as SilentResult, StatusCode};
use std::sync::Arc;

/// 从文件加载函数的请求
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadFileRequest {
    pub file_path: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// 从目录加载函数的请求
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadDirectoryRequest {
    pub directory_path: String,
}

/// 通用 API 响应格式
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub message: Option<String>,
}

/// 健康检查
pub async fn health_check(_req: Request) -> SilentResult<Response> {
    let response = ApiResponse {
        success: true,
        data: Some("FluxFaaS is running".to_string()),
        error: None,
        message: Some("Health check passed".to_string()),
    };
    Ok(Response::json(&response))
}

/// 注册函数
pub async fn register_function(mut req: Request) -> SilentResult<Response> {
    // 解析请求体 - 使用 json_parse() 方法
    let register_req: RegisterFunctionRequest = match req.json_parse().await {
        Ok(req) => req,
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Invalid request body: {e}")),
                message: Some("Failed to parse request body".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };
    // 从配置中获取 scheduler
    let scheduler = req.get_config_uncheck::<Arc<SimpleScheduler>>();

    let response = match scheduler
        .registry()
        .register(FunctionMetadata::from_request(register_req.clone()))
        .await
    {
        Ok(_) => ApiResponse {
            success: true,
            data: Some("Function registration received".to_string()),
            error: None,
            message: Some(format!(
                "Function '{}' registration request received",
                register_req.name
            )),
        },
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Register function failed: {e}")),
                message: Some("Failed to register function".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };
    Ok(Response::json(&response))
}

/// 列出所有函数
pub async fn list_functions(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 获取所有函数列表
    let functions = scheduler.registry().list().await;

    // 构建函数列表数据
    let function_list: Vec<_> = functions
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id.to_string(),
                "name": f.name,
                "description": f.description,
                "created_at": f.created_at,
                "timeout_ms": f.timeout_ms
            })
        })
        .collect();

    let response = ApiResponse {
        success: true,
        data: Some(function_list),
        error: None,
        message: Some(format!(
            "Retrieved {} functions successfully",
            functions.len()
        )),
    };
    Ok(Response::json(&response))
}

/// 获取单个函数信息
pub async fn get_function(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 获取路径参数
    let name: String = match req.get_path_params("name") {
        Ok(name) => name,
        Err(_) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Missing function name parameter".to_string()),
                message: Some("Function name is required".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 从注册表获取函数详情
    match scheduler.registry().get(&name).await {
        Ok(function) => {
            let response = ApiResponse {
                success: true,
                data: Some(function),
                error: None,
                message: Some(format!("Function '{name}' details retrieved")),
            };
            Ok(Response::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Function not found: {e}")),
                message: Some(format!("Function '{name}' not found")),
            };
            Ok(Response::json(&response).with_status(StatusCode::NOT_FOUND))
        }
    }
}

/// 删除函数
pub async fn delete_function(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    let name: String = match req.get_path_params("name") {
        Ok(name) => name,
        Err(_) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Missing function name parameter".to_string()),
                message: Some("Function name is required".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 从注册表删除函数
    match scheduler.registry().remove(&name).await {
        Ok(_) => {
            // 同时从缓存中移除函数
            scheduler.runtime().cache().remove(&name).await;

            let response = ApiResponse {
                success: true,
                data: Some(format!("Function '{name}' deleted successfully")),
                error: None,
                message: Some(format!("Function '{name}' deleted successfully")),
            };
            Ok(Response::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Failed to delete function: {e}")),
                message: Some(format!("Function '{name}' not found")),
            };
            Ok(Response::json(&response).with_status(StatusCode::NOT_FOUND))
        }
    }
}

/// 调用函数
pub async fn invoke_function(mut req: Request) -> SilentResult<Response> {
    // 先解析请求体
    let invoke_req: InvokeRequest = match req.json_parse().await {
        Ok(req) => req,
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Invalid request body: {e}")),
                message: Some("Failed to parse request body".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    let name: String = match req.get_path_params("name") {
        Ok(name) => name,
        Err(_) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Missing function name parameter".to_string()),
                message: Some("Function name is required".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 使用调度器执行函数
    match scheduler.schedule(&name, invoke_req).await {
        Ok(invoke_response) => {
            let response = ApiResponse {
                success: true,
                data: Some(invoke_response),
                error: None,
                message: Some(format!("Function '{name}' executed successfully")),
            };
            Ok(Response::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Function execution failed: {e}")),
                message: Some(format!("Failed to execute function '{name}'")),
            };
            Ok(Response::json(&response).with_status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// 获取调度器状态
pub async fn get_scheduler_status(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let _scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 暂时返回模拟状态，因为 SimpleScheduler 还没有这些方法
    let response = ApiResponse {
        success: true,
        data: Some("Scheduler is running".to_string()),
        error: None,
        message: Some("Scheduler status retrieved successfully".to_string()),
    };
    Ok(Response::json(&response))
}

/// 从文件加载函数
pub async fn load_function_from_file(mut req: Request) -> SilentResult<Response> {
    // 先解析请求体
    let load_req: LoadFileRequest = match req.json_parse().await {
        Ok(req) => req,
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Invalid request body: {e}")),
                message: Some("Failed to parse request body".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 使用 FunctionLoader 从文件加载函数
    match scheduler
        .loader()
        .load_function_from_file(
            &load_req.file_path,
            load_req.name,
            load_req.description,
            load_req.timeout_ms,
        )
        .await
    {
        Ok(function_metadata) => {
            let function_name = function_metadata.name.clone();
            // 将函数注册到注册表
            match scheduler
                .registry()
                .register(function_metadata.clone())
                .await
            {
                Ok(_) => {
                    let response = ApiResponse {
                        success: true,
                        data: Some(function_metadata),
                        error: None,
                        message: Some(format!(
                            "Function '{function_name}' loaded successfully from file"
                        )),
                    };
                    Ok(Response::json(&response))
                }
                Err(e) => {
                    let response = ApiResponse::<()> {
                        success: false,
                        data: None,
                        error: Some(format!("Failed to register function: {e}")),
                        message: Some("Function loading failed during registration".to_string()),
                    };
                    Ok(Response::json(&response).with_status(StatusCode::INTERNAL_SERVER_ERROR))
                }
            }
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Failed to load function from file: {e}")),
                message: Some("Function loading failed".to_string()),
            };
            Ok(Response::json(&response).with_status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// 从目录加载函数
pub async fn load_functions_from_directory(mut req: Request) -> SilentResult<Response> {
    // 先解析请求体
    let load_req: LoadDirectoryRequest = match req.json_parse().await {
        Ok(req) => req,
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Invalid request body: {e}")),
                message: Some("Failed to parse request body".to_string()),
            };
            return Ok(Response::json(&response).with_status(StatusCode::BAD_REQUEST));
        }
    };

    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 使用 FunctionLoader 从目录加载函数
    match scheduler
        .loader()
        .load_functions_from_directory(&load_req.directory_path)
        .await
    {
        Ok(functions) => {
            let mut loaded_functions = Vec::new();
            let mut failed_functions = Vec::new();

            // 批量注册函数
            for function_metadata in functions {
                let function_name = function_metadata.name.clone();
                match scheduler
                    .registry()
                    .register(function_metadata.clone())
                    .await
                {
                    Ok(_) => {
                        loaded_functions.push(function_name);
                    }
                    Err(e) => {
                        failed_functions.push(format!("{function_name}: {e}"));
                    }
                }
            }

            if failed_functions.is_empty() {
                let response = ApiResponse {
                    success: true,
                    data: Some(format!(
                        "Loaded {} functions: {:?}",
                        loaded_functions.len(),
                        loaded_functions
                    )),
                    error: None,
                    message: Some(format!(
                        "Successfully loaded {} functions from directory",
                        loaded_functions.len()
                    )),
                };
                Ok(Response::json(&response))
            } else {
                let response = ApiResponse {
                    success: false,
                    data: Some(format!(
                        "Loaded: {loaded_functions:?}, Failed: {failed_functions:?}"
                    )),
                    error: Some(format!(
                        "Failed to register {} functions",
                        failed_functions.len()
                    )),
                    message: Some("Partial success - some functions failed to load".to_string()),
                };
                Ok(Response::json(&response).with_status(StatusCode::PARTIAL_CONTENT))
            }
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Failed to load functions from directory: {e}")),
                message: Some("Directory loading failed".to_string()),
            };
            Ok(Response::json(&response).with_status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// 获取缓存统计
pub async fn get_cache_stats(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 获取缓存统计信息
    let cache_stats = scheduler.runtime().cache().stats().await;
    let hit_rate = scheduler.runtime().cache().hit_rate().await;

    // 构建响应数据
    let stats_data = serde_json::json!({
        "hits": cache_stats.hits,
        "misses": cache_stats.misses,
        "hit_rate": format!("{:.2}%", hit_rate * 100.0),
        "size": cache_stats.size,
        "memory_usage_bytes": cache_stats.memory_usage,
        "memory_usage_mb": cache_stats.memory_usage as f64 / (1024.0 * 1024.0),
        "max_memory_bytes": cache_stats.max_memory,
        "max_memory_mb": cache_stats.max_memory as f64 / (1024.0 * 1024.0),
        "evictions": cache_stats.evictions
    });

    let response = ApiResponse {
        success: true,
        data: Some(stats_data),
        error: None,
        message: Some("Cache statistics retrieved successfully".to_string()),
    };
    Ok(Response::json(&response))
}

/// 获取性能统计
pub async fn get_performance_stats(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 获取性能统计信息
    let performance_report = scheduler.runtime().monitor().generate_report().await;
    let global_stats = performance_report.global_stats;
    let hottest_functions = scheduler.runtime().monitor().get_hottest_functions(5).await;
    let slowest_functions = scheduler.runtime().monitor().get_slowest_functions(5).await;

    // 构建响应数据
    let stats_data = serde_json::json!({
        "global_stats": {
            "total_requests": global_stats.total_requests,
            "total_success": global_stats.total_success,
            "total_failures": global_stats.total_failures,
            "success_rate": if global_stats.total_requests > 0 {
                global_stats.total_success as f64 / global_stats.total_requests as f64 * 100.0
            } else { 0.0 },
            "active_functions": global_stats.active_functions,
            "current_system_memory_bytes": global_stats.current_system_memory,
            "peak_system_memory_bytes": global_stats.peak_system_memory,
            "uptime_seconds": global_stats.start_time.map(|start| start.elapsed().as_secs()).unwrap_or(0)
        },
        "hottest_functions": hottest_functions,
        "slowest_functions": slowest_functions,
        "function_count": performance_report.function_stats.len(),
        "health_status": format!("{:?}", performance_report.health_status),
        "recommendations": performance_report.recommendations
    });

    let response = ApiResponse {
        success: true,
        data: Some(stats_data),
        error: None,
        message: Some("Performance statistics retrieved successfully".to_string()),
    };
    Ok(Response::json(&response))
}

/// 重置调度器
pub async fn reset_scheduler(req: Request) -> SilentResult<Response> {
    // 从配置中获取 scheduler
    let _scheduler: &Arc<SimpleScheduler> = req.get_config()?;

    // 暂时返回成功响应，因为 SimpleScheduler 还没有这些方法
    let response = ApiResponse {
        success: true,
        data: Some("Scheduler reset received".to_string()),
        error: None,
        message: Some("Scheduler reset request received".to_string()),
    };
    Ok(Response::json(&response))
}
