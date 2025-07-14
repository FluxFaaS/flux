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

        // 扩展功能 - 从文件加载函数
        route::post("/functions/load-file").to({
            let scheduler = scheduler.clone();
            move |req| handlers::load_function_from_file(req, scheduler.clone())
        }),

        // 从目录批量加载函数
        route::post("/functions/load-directory").to({
            let scheduler = scheduler.clone();
            move |req| handlers::load_functions_from_directory(req, scheduler.clone())
        }),

        // 缓存统计
        route::get("/cache/stats").to({
            let scheduler = scheduler.clone();
            move |req| handlers::get_cache_stats(req, scheduler.clone())
        }),

        // 性能监控
        route::get("/monitor/performance").to({
            let scheduler = scheduler.clone();
            move |req| handlers::get_performance_monitor(req, scheduler.clone())
        }),

        // 重置监控数据
        route::post("/monitor/reset").to({
            let scheduler = scheduler.clone();
            move |req| handlers::reset_performance_data(req, scheduler.clone())
        }),


    ]
}
