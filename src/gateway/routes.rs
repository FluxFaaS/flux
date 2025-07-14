use super::handlers;
use crate::scheduler::SimpleScheduler;
use silent::prelude::*;
use std::sync::Arc;

pub fn build_routes(scheduler: Arc<SimpleScheduler>) -> RootRoute {
    let mut root = RootRoute::new();

    // 健康检查路由
    let health_route = Route::new("health").get(handlers::health_check);
    root.push(health_route);

    // 函数管理路由
    let functions_route = Route::new("functions")
        .post({
            let scheduler = scheduler.clone();
            move |req| handlers::register_function(req, scheduler.clone())
        })
        .get({
            let scheduler = scheduler.clone();
            move |req| handlers::list_functions(req, scheduler.clone())
        });
    root.push(functions_route);

    // 单个函数操作路由
    let function_route = Route::new("functions/:name")
        .get({
            let scheduler = scheduler.clone();
            move |req| handlers::get_function(req, scheduler.clone())
        })
        .delete({
            let scheduler = scheduler.clone();
            move |req| handlers::delete_function(req, scheduler.clone())
        });
    root.push(function_route);

    // 函数调用路由
    let invoke_route = Route::new("invoke/:name").post({
        let scheduler = scheduler.clone();
        move |req| handlers::invoke_function(req, scheduler.clone())
    });
    root.push(invoke_route);

    // 调度器状态路由
    let status_route = Route::new("status").get({
        let scheduler = scheduler.clone();
        move |req| handlers::get_scheduler_status(req, scheduler.clone())
    });
    root.push(status_route);

    // 文件加载路由
    let load_file_route = Route::new("load/file").post({
        let scheduler = scheduler.clone();
        move |req| handlers::load_function_from_file(req, scheduler.clone())
    });
    root.push(load_file_route);

    // 目录加载路由
    let load_dir_route = Route::new("load/directory").post({
        let scheduler = scheduler.clone();
        move |req| handlers::load_functions_from_directory(req, scheduler.clone())
    });
    root.push(load_dir_route);

    // 缓存统计路由
    let cache_route = Route::new("cache/stats").get({
        let scheduler = scheduler.clone();
        move |req| handlers::get_cache_stats(req, scheduler.clone())
    });
    root.push(cache_route);

    // 性能统计路由
    let perf_route = Route::new("performance/stats").get({
        let scheduler = scheduler.clone();
        move |req| handlers::get_performance_stats(req, scheduler.clone())
    });
    root.push(perf_route);

    // 重置路由
    let reset_route = Route::new("reset").post({
        let scheduler = scheduler.clone();
        move |req| handlers::reset_scheduler(req, scheduler.clone())
    });
    root.push(reset_route);

    root
}
