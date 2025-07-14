use super::handlers;
use silent::prelude::*;

pub fn build_routes() -> RootRoute {
    let mut root = RootRoute::new();

    // 健康检查路由
    let health_route = Route::new("health").get(handlers::health_check);
    root.push(health_route);

    // 函数管理路由
    let functions_route = Route::new("functions")
        .post(handlers::register_function)
        .get(handlers::list_functions);
    root.push(functions_route);

    // 单个函数操作路由
    let function_route = Route::new("functions/<name>")
        .get(handlers::get_function)
        .delete(handlers::delete_function);
    root.push(function_route);

    // 函数调用路由
    let invoke_route = Route::new("invoke/<name>").post(handlers::invoke_function);
    root.push(invoke_route);

    // 调度器状态路由
    let status_route = Route::new("status").get(handlers::get_scheduler_status);
    root.push(status_route);

    // 文件加载路由
    let load_file_route = Route::new("load/file").post(handlers::load_function_from_file);
    root.push(load_file_route);

    // 目录加载路由
    let load_dir_route = Route::new("load/directory").post(handlers::load_functions_from_directory);
    root.push(load_dir_route);

    // 缓存统计路由
    let cache_route = Route::new("cache/stats").get(handlers::get_cache_stats);
    root.push(cache_route);

    // 性能统计路由
    let perf_route = Route::new("performance/stats").get(handlers::get_performance_stats);
    root.push(perf_route);

    // 重置路由
    let reset_route = Route::new("reset").post(handlers::reset_scheduler);
    root.push(reset_route);

    root
}
