use std::time::Instant;
use tokio::time::{Duration, sleep};

use flux::scheduler::balancer::{
    CircuitBreakerState, LoadBalanceStrategy, LoadBalanceTarget, LoadBalancer, LoadBalancerConfig,
    PerformanceStats,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FluxFaaS 负载均衡器测试");
    println!("{}", "=".repeat(50));

    // 测试1: 基本功能测试
    println!("\n📊 测试1: 负载均衡器基本功能");
    test_basic_functionality().await?;

    // 测试2: 轮询策略测试
    println!("\n🔄 测试2: 轮询负载均衡策略");
    test_round_robin_strategy().await?;

    // 测试3: 加权轮询策略测试
    println!("\n⚖️ 测试3: 加权轮询负载均衡策略");
    test_weighted_round_robin_strategy().await?;

    // 测试4: 最少连接策略测试
    println!("\n🔗 测试4: 最少连接负载均衡策略");
    test_least_connections_strategy().await?;

    // 测试5: 响应时间策略测试
    println!("\n⚡ 测试5: 响应时间最短策略");
    test_fastest_response_strategy().await?;

    // 测试6: 断路器功能测试
    println!("\n🔌 测试6: 断路器功能");
    test_circuit_breaker().await?;

    // 测试7: 一致性哈希策略测试
    println!("\n🔀 测试7: 一致性哈希策略");
    test_consistent_hash_strategy().await?;

    // 测试8: 自适应策略测试
    println!("\n🧠 测试8: 自适应负载均衡策略");
    test_adaptive_strategy().await?;

    // 测试9: 性能监控测试
    println!("\n📈 测试9: 性能监控功能");
    test_performance_monitoring().await?;

    // 测试10: 故障转移测试
    println!("\n🔄 测试10: 故障转移功能");
    test_failover().await?;

    println!("\n✅ 所有测试完成!");
    Ok(())
}

/// 测试基本功能
async fn test_basic_functionality() -> anyhow::Result<()> {
    let config = LoadBalancerConfig::default();
    let balancer = LoadBalancer::new(config);

    // 初始状态检查
    let stats = balancer.get_statistics().await;
    println!("  - 初始目标数: {}", stats.total_targets);
    println!("  - 初始健康目标数: {}", stats.healthy_targets);
    assert_eq!(stats.total_targets, 0);
    assert_eq!(stats.healthy_targets, 0);

    // 添加目标
    let target1 = create_test_target("target1", "Test Target 1", 100, 0.3, 5, 50.0, true);
    let target2 = create_test_target("target2", "Test Target 2", 150, 0.5, 8, 75.0, true);
    let target3 = create_test_target("target3", "Test Target 3", 80, 0.2, 3, 30.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    let stats = balancer.get_statistics().await;
    println!("  - 添加后目标数: {}", stats.total_targets);
    println!("  - 添加后健康目标数: {}", stats.healthy_targets);
    assert_eq!(stats.total_targets, 3);
    assert_eq!(stats.healthy_targets, 3);

    // 移除目标
    balancer.remove_target("target2").await?;
    let stats = balancer.get_statistics().await;
    println!("  - 移除后目标数: {}", stats.total_targets);
    assert_eq!(stats.total_targets, 2);

    println!("✅ 基本功能测试通过");
    Ok(())
}

/// 测试轮询策略
async fn test_round_robin_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::RoundRobin,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加测试目标
    for i in 1..=3 {
        let target = create_test_target(
            &format!("target{i}"),
            &format!("Test Target {i}"),
            100,
            0.3,
            5,
            50.0,
            true,
        );
        balancer.add_target(target).await?;
    }

    // 测试轮询选择
    let mut selections = Vec::new();
    for _ in 0..9 {
        let result = balancer.select_target(None).await?;
        selections.push(result.target_id);
    }

    println!("  - 轮询选择结果: {selections:?}");

    // 验证轮询模式
    assert_eq!(selections[0], selections[3]);
    assert_eq!(selections[1], selections[4]);
    assert_eq!(selections[2], selections[5]);

    println!("✅ 轮询策略测试通过");
    Ok(())
}

/// 测试加权轮询策略
async fn test_weighted_round_robin_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::WeightedRoundRobin,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加不同权重的目标
    let target1 = create_test_target("target1", "High Weight", 300, 0.3, 5, 50.0, true);
    let target2 = create_test_target("target2", "Medium Weight", 200, 0.4, 7, 60.0, true);
    let target3 = create_test_target("target3", "Low Weight", 100, 0.2, 3, 40.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    // 测试加权轮询选择
    let mut selections = Vec::new();
    for _ in 0..12 {
        let result = balancer.select_target(None).await?;
        selections.push(result.target_id);
    }

    println!("  - 加权轮询选择结果: {selections:?}");

    // 统计选择次数
    let mut counts = std::collections::HashMap::new();
    for selection in selections {
        *counts.entry(selection).or_insert(0) += 1;
    }

    println!("  - 选择次数统计: {counts:?}");

    // 验证高权重目标被选择更多次
    assert!(counts.get("target1").unwrap_or(&0) > counts.get("target3").unwrap_or(&0));

    println!("✅ 加权轮询策略测试通过");
    Ok(())
}

/// 测试最少连接策略
async fn test_least_connections_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::LeastConnections,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加不同连接数的目标
    let target1 = create_test_target("target1", "High Connections", 100, 0.3, 20, 50.0, true);
    let target2 = create_test_target("target2", "Medium Connections", 100, 0.4, 10, 60.0, true);
    let target3 = create_test_target("target3", "Low Connections", 100, 0.2, 5, 40.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    // 测试最少连接选择
    for _ in 0..5 {
        let result = balancer.select_target(None).await?;
        println!("  - 选择目标: {} (连接数最少)", result.target_id);
        assert_eq!(result.target_id, "target3"); // 应该始终选择连接数最少的
    }

    println!("✅ 最少连接策略测试通过");
    Ok(())
}

/// 测试响应时间最短策略
async fn test_fastest_response_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::FastestResponse,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加不同响应时间的目标
    let target1 = create_test_target("target1", "Slow Response", 100, 0.3, 5, 100.0, true);
    let target2 = create_test_target("target2", "Medium Response", 100, 0.4, 7, 50.0, true);
    let target3 = create_test_target("target3", "Fast Response", 100, 0.2, 3, 20.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    // 测试响应时间最短选择
    for _ in 0..5 {
        let result = balancer.select_target(None).await?;
        println!("  - 选择目标: {} (响应时间最短)", result.target_id);
        assert_eq!(result.target_id, "target3"); // 应该始终选择响应时间最短的
    }

    println!("✅ 响应时间最短策略测试通过");
    Ok(())
}

/// 测试断路器功能
async fn test_circuit_breaker() -> anyhow::Result<()> {
    let mut config = LoadBalancerConfig::default();
    config.failover_config.failure_threshold = 3;
    config.failover_config.success_threshold = 2;
    let balancer = LoadBalancer::new(config);

    let target = create_test_target("target1", "Test Target", 100, 0.3, 5, 50.0, true);
    balancer.add_target(target).await?;

    // 模拟连续失败
    println!("  - 模拟连续失败...");
    for i in 1..=5 {
        balancer
            .update_target_status("target1", false, 0.8, 20, 200.0)
            .await?;
        println!("    失败次数: {i}");
    }

    let targets = balancer.get_targets().await;
    println!("  - 断路器状态: {:?}", targets[0].circuit_breaker_state);
    assert_eq!(targets[0].circuit_breaker_state, CircuitBreakerState::Open);

    // 等待恢复时间
    println!("  - 等待断路器恢复...");
    sleep(Duration::from_millis(100)).await;

    // 手动设置恢复时间
    balancer
        .update_target_status("target1", true, 0.3, 5, 50.0)
        .await?;
    sleep(Duration::from_millis(100)).await;

    println!("✅ 断路器功能测试通过");
    Ok(())
}

/// 测试一致性哈希策略
async fn test_consistent_hash_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::ConsistentHash,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加测试目标
    for i in 1..=3 {
        let target = create_test_target(
            &format!("target{i}"),
            &format!("Test Target {i}"),
            100,
            0.3,
            5,
            50.0,
            true,
        );
        balancer.add_target(target).await?;
    }

    // 测试一致性哈希选择
    let test_keys = vec!["user1", "user2", "user3", "user1", "user2", "user3"];
    let mut selections = Vec::new();

    for key in &test_keys {
        let result = balancer.select_target(Some(key)).await?;
        selections.push((key.to_string(), result.target_id));
    }

    println!("  - 一致性哈希选择结果: {selections:?}");

    // 验证相同key选择相同目标
    assert_eq!(selections[0].1, selections[3].1); // user1
    assert_eq!(selections[1].1, selections[4].1); // user2
    assert_eq!(selections[2].1, selections[5].1); // user3

    println!("✅ 一致性哈希策略测试通过");
    Ok(())
}

/// 测试自适应策略
async fn test_adaptive_strategy() -> anyhow::Result<()> {
    let config = LoadBalancerConfig {
        strategy: LoadBalanceStrategy::Adaptive,
        ..Default::default()
    };
    let balancer = LoadBalancer::new(config);

    // 添加不同性能特征的目标
    let target1 = create_test_target("target1", "High Load", 100, 0.9, 50, 200.0, true);
    let target2 = create_test_target("target2", "Medium Load", 100, 0.5, 20, 100.0, true);
    let target3 = create_test_target("target3", "Low Load", 100, 0.1, 5, 30.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    // 测试自适应选择
    let mut selections = Vec::new();
    for _ in 0..10 {
        let result = balancer.select_target(None).await?;
        selections.push(result.target_id);
    }

    println!("  - 自适应选择结果: {selections:?}");

    // 统计选择次数
    let mut counts = std::collections::HashMap::new();
    for selection in selections {
        *counts.entry(selection).or_insert(0) += 1;
    }

    println!("  - 选择次数统计: {counts:?}");

    // 验证低负载目标被选择更多次
    assert!(counts.get("target3").unwrap_or(&0) > counts.get("target1").unwrap_or(&0));

    println!("✅ 自适应策略测试通过");
    Ok(())
}

/// 测试性能监控
async fn test_performance_monitoring() -> anyhow::Result<()> {
    let config = LoadBalancerConfig::default();
    let balancer = LoadBalancer::new(config);

    let target = create_test_target("target1", "Test Target", 100, 0.3, 5, 50.0, true);
    balancer.add_target(target).await?;

    // 模拟一些请求
    for i in 1..=10 {
        let success = i % 3 != 0; // 大部分成功，少部分失败
        let response_time = if success { 50.0 } else { 200.0 };
        balancer
            .update_target_status("target1", success, 0.3, 5, response_time)
            .await?;
    }

    let stats = balancer.get_statistics().await;
    println!("  - 总请求数: {}", stats.total_requests);
    println!("  - 成功请求数: {}", stats.successful_requests);
    println!("  - 失败请求数: {}", stats.failed_requests);
    println!("  - 成功率: {:.2}%", stats.success_rate * 100.0);
    println!("  - 平均响应时间: {:.2}ms", stats.avg_response_time_ms);

    assert!(stats.total_requests > 0);
    assert!(stats.success_rate > 0.0);

    println!("✅ 性能监控测试通过");
    Ok(())
}

/// 测试故障转移
async fn test_failover() -> anyhow::Result<()> {
    let config = LoadBalancerConfig::default();
    let balancer = LoadBalancer::new(config);

    // 添加多个目标
    let target1 = create_test_target("target1", "Primary", 100, 0.3, 5, 50.0, true);
    let target2 = create_test_target("target2", "Secondary", 100, 0.4, 7, 60.0, true);
    let target3 = create_test_target("target3", "Tertiary", 100, 0.2, 3, 40.0, true);

    balancer.add_target(target1).await?;
    balancer.add_target(target2).await?;
    balancer.add_target(target3).await?;

    // 正常情况下的选择
    println!("  - 正常情况下的负载均衡:");
    for _ in 0..3 {
        let result = balancer.select_target(None).await?;
        println!("    选择目标: {}", result.target_id);
    }

    // 模拟target1故障
    println!("  - 模拟target1故障:");
    balancer
        .update_target_status("target1", false, 0.9, 50, 500.0)
        .await?;

    // 故障后的选择应该避开不健康的目标
    println!("  - 故障后的负载均衡:");
    for _ in 0..4 {
        let result = balancer.select_target(None).await?;
        println!("    选择目标: {}", result.target_id);
        assert_ne!(result.target_id, "target1"); // 不应该选择故障目标
    }

    println!("✅ 故障转移测试通过");
    Ok(())
}

/// 创建测试目标
fn create_test_target(
    id: &str,
    name: &str,
    weight: u32,
    load: f64,
    connections: u32,
    response_time: f64,
    healthy: bool,
) -> LoadBalanceTarget {
    LoadBalanceTarget {
        id: id.to_string(),
        name: name.to_string(),
        weight,
        current_load: load,
        active_connections: connections,
        avg_response_time_ms: response_time,
        is_healthy: healthy,
        last_activity: Instant::now(),
        consecutive_failures: 0,
        consecutive_successes: 0,
        circuit_breaker_state: CircuitBreakerState::Closed,
        performance_stats: PerformanceStats::default(),
    }
}
