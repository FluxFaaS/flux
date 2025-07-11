use crate::functions::{FunctionMetadata, InvokeRequest, RegisterFunctionRequest};
use crate::scheduler::{SimpleScheduler, Scheduler};
use silent::{Request, Response, Result as SilentResult};
use std::sync::Arc;

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
