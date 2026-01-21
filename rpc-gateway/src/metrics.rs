/// 性能监控指标模块
///
/// 使用 Prometheus 格式的指标，用于监控 RPC Gateway 的性能
use lazy_static::lazy_static;
use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, HistogramVec};

lazy_static! {
    /// 交易总数（按类型统计：send_transaction, send_raw_transaction）
    pub static ref TRANSACTIONS_TOTAL: CounterVec = register_counter_vec!(
        "rpc_gateway_transactions_total",
        "Total number of transactions received",
        &["method"]
    )
    .expect("Failed to register transactions_total metric");

    /// 交易失败数（按类型和错误码统计）
    pub static ref TRANSACTIONS_FAILED: CounterVec = register_counter_vec!(
        "rpc_gateway_transactions_failed_total",
        "Total number of failed transactions",
        &["method", "error_code"]
    )
    .expect("Failed to register transactions_failed metric");

    /// 交易处理耗时（秒）
    pub static ref TRANSACTION_DURATION: HistogramVec = register_histogram_vec!(
        "rpc_gateway_transaction_duration_seconds",
        "Transaction processing duration in seconds",
        &["method"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .expect("Failed to register transaction_duration metric");

    /// Walrus 写入耗时（秒）
    pub static ref WALRUS_WRITE_DURATION: HistogramVec = register_histogram_vec!(
        "rpc_gateway_walrus_write_duration_seconds",
        "Walrus write operation duration in seconds",
        &["topic"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    )
    .expect("Failed to register walrus_write_duration metric");

    /// 并发请求数
    pub static ref CONCURRENT_REQUESTS: CounterVec = register_counter_vec!(
        "rpc_gateway_concurrent_requests",
        "Number of concurrent requests being processed",
        &["method"]
    )
    .expect("Failed to register concurrent_requests metric");

    /// 批量处理统计
    pub static ref BATCH_SIZE: HistogramVec = register_histogram_vec!(
        "rpc_gateway_batch_size",
        "Size of batched operations",
        &["operation"],
        vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
    )
    .expect("Failed to register batch_size metric");
}

/// 获取所有指标的文本格式输出（用于 /metrics 端点）
pub fn get_metrics() -> String {
    use prometheus::{Encoder, TextEncoder};

    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    String::from_utf8(buffer).unwrap()
}
