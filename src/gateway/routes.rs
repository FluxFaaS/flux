use super::handlers;
use crate::scheduler::SimpleScheduler;
use silent::prelude::*;
use std::sync::Arc;

/// 构建所有路由
pub fn build_routes(scheduler: Arc<SimpleScheduler>) -> Vec<Route> {
    vec![
        // 健康检查
        route::get("/health").to(handlers::health_check),

        // 函数管理
        route::post("/functions").to({
            let scheduler = scheduler.clone();
            move |req| handlers::register_function(req, scheduler.clone())
        }),

        route::get("/functions").to({
            let scheduler = scheduler.clone();
            move |req| handlers::list_functions(req, scheduler.clone())
        }),

        route::get("/functions/:name").to({
            let scheduler = scheduler.clone();
            move |req| handlers::get_function(req, scheduler.clone())
        }),

        route::delete("/functions/:name").to({
            let scheduler = scheduler.clone();
            move |req| handlers::delete_function(req, scheduler.clone())
        }),

        // 函数调用
        route::post("/invoke/:name").to({
            let scheduler = scheduler.clone();
            move |req| handlers::invoke_function(req, scheduler.clone())
        }),

        // 系统信息
        route::get("/status").to({
            let scheduler = scheduler.clone();
            move |req| handlers::get_status(req, scheduler.clone())
        }),
    ]
}
