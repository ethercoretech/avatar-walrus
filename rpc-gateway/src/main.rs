mod error;
mod metrics;

use anyhow::Result;
use clap::Parser;
use distributed_walrus::cli_client::CliClient;
use error::RpcError;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{Server, ServerHandle};
use metrics::{
    BATCH_SIZE, TRANSACTIONS_FAILED, TRANSACTIONS_TOTAL, TRANSACTION_DURATION,
    WALRUS_WRITE_DURATION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, OnceCell, Semaphore};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

// Block Producer 类型导入
use block_producer::db::{RedbStateDB, StateDatabase};
use alloy_primitives::Address;

/// RPC Gateway
///
/// 接收外部钱包的区块链交易，并写入 Walrus 服务器
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Walrus 服务器地址
    #[arg(long, default_value = "127.0.0.1:9091")]
    walrus_addr: String,

    /// JSON-RPC 服务器监听端口
    #[arg(long, default_value = "8545")]
    rpc_port: u16,

    /// JSON-RPC 服务器监听地址
    #[arg(long, default_value = "127.0.0.1")]
    rpc_host: String,

    /// 默认写入的 topic
    #[arg(long, default_value = "blockchain-txs")]
    default_topic: String,

    /// 最大并发连接数
    #[arg(long, default_value = "10000")]
    max_connections: u32,

    /// 最大并发请求处理数
    #[arg(long, default_value = "1000")]
    max_concurrent_requests: usize,

    /// 批量处理间隔（毫秒）
    #[arg(long, default_value = "10")]
    batch_interval_ms: u64,

    /// 批量处理最大大小
    #[arg(long, default_value = "100")]
    max_batch_size: usize,

    /// 请求超时时间（秒）
    #[arg(long, default_value = "30")]
    request_timeout_secs: u64,

    /// 状态数据库路径（与 block-producer 共享）
    #[arg(long, default_value = "./data/block_producer_state_blockchain-txs.redb")]
    state_db_path: String,
}

/// 区块链交易数据结构（简化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub data: String,
    pub gas: String,
    pub gas_price: Option<String>,
    pub nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// 批量处理任务
#[derive(Debug)]
struct BatchTask {
    data: String,
    response_tx: tokio::sync::oneshot::Sender<Result<String, jsonrpsee::types::ErrorObjectOwned>>,
}

/// 批量处理器
///
/// 将短时间内收到的多个交易批量提交到 Walrus，减少网络往返次数
struct BatchProcessor {
    tx: mpsc::Sender<BatchTask>,
}

impl BatchProcessor {
    fn new(
        walrus_client: CliClient,
        topic: String,
        batch_interval: Duration,
        max_batch_size: usize,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<BatchTask>(10000);

        // 启动批量处理任务
        tokio::spawn(async move {
            let mut batch: Vec<BatchTask> = Vec::new();
            let mut interval = tokio::time::interval(batch_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    // 收到新任务
                    Some(task) = rx.recv() => {
                        batch.push(task);

                        // 如果批量大小达到上限，立即处理
                        if batch.len() >= max_batch_size {
                            Self::process_batch(&walrus_client, &topic, &mut batch).await;
                        }
                    }
                    // 定时器触发
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            Self::process_batch(&walrus_client, &topic, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    async fn process_batch(walrus_client: &CliClient, topic: &str, batch: &mut Vec<BatchTask>) {
        if batch.is_empty() {
            return;
        }

        let batch_size = batch.len();
        info!("处理批量任务: {} 个交易", batch_size);

        // 记录批量大小
        BATCH_SIZE
            .with_label_values(&["write"])
            .observe(batch_size as f64);

        let start = Instant::now();

        // 并发写入所有交易
        let tasks: Vec<_> = batch.drain(..).collect();
        let results: Vec<_> = futures::future::join_all(tasks.into_iter().map(|task| {
            let client = walrus_client.clone();
            let topic = topic.to_string();
            async move {
                let write_start = Instant::now();
                let result = client.put(&topic, &task.data).await;
                let duration = write_start.elapsed();

                WALRUS_WRITE_DURATION
                    .with_label_values(&[&topic])
                    .observe(duration.as_secs_f64());

                (task, result)
            }
        }))
        .await;

        // 发送响应
        for (task, result) in results {
            let response = match result {
                Ok(_) => {
                    let mut hasher = Sha256::new();
                    hasher.update(task.data.as_bytes());
                    let hash_bytes = hasher.finalize();
                    Ok(format!("0x{}", hex::encode(hash_bytes)))
                }
                Err(e) => Err(RpcError::WalrusWriteFailed.into_error_object(e.to_string())),
            };

            let _ = task.response_tx.send(response);
        }

        let duration = start.elapsed();
        info!("批量处理完成: {} 个交易，耗时 {:?}", batch_size, duration);
    }

    async fn submit(&self, data: String) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = BatchTask {
            data,
            response_tx: tx,
        };

        self.tx
            .send(task)
            .await
            .map_err(|_| RpcError::InternalError.into_error_object("批量处理器已关闭"))?;

        rx.await
            .map_err(|_| RpcError::InternalError.into_error_object("批量处理响应丢失"))?
    }
}

/// JSON-RPC API 定义
#[rpc(server)]
pub trait WalrusRpcApi {
    /// 提交交易到 Walrus
    #[method(name = "eth_sendTransaction")]
    async fn send_transaction(
        &self,
        tx: Transaction,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;

    /// 提交原始交易数据
    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(
        &self,
        data: String,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;

    /// 健康检查
    #[method(name = "health")]
    async fn health(&self) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;

    /// 获取账户交易计数（nonce）
    #[method(name = "eth_getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: String,
        block_number: Option<String>,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;
}

/// RPC 服务实现
pub struct WalrusRpcServer {
    walrus_client: CliClient,
    default_topic: String,
    /// 使用 OnceCell 确保 topic 只注册一次
    topic_registered: Arc<OnceCell<()>>,
    /// 批量处理器（可选）
    batch_processor: Option<Arc<BatchProcessor>>,
    /// 并发限制器
    semaphore: Arc<Semaphore>,
    /// 请求超时时间
    request_timeout: Duration,
    /// 状态数据库路径（用于定期刷新读取）
    state_db_path: String,
}

impl WalrusRpcServer {
    pub fn new(
        walrus_addr: String,
        default_topic: String,
        max_concurrent_requests: usize,
        batch_interval: Duration,
        max_batch_size: usize,
        request_timeout: Duration,
        enable_batching: bool,
        state_db_path: String,
    ) -> Self {
        let walrus_client = CliClient::new(walrus_addr);

        // 创建批量处理器
        let batch_processor = if enable_batching {
            Some(Arc::new(BatchProcessor::new(
                walrus_client.clone(),
                default_topic.clone(),
                batch_interval,
                max_batch_size,
            )))
        } else {
            None
        };

        Self {
            walrus_client,
            default_topic,
            topic_registered: Arc::new(OnceCell::new()),
            batch_processor,
            semaphore: Arc::new(Semaphore::new(max_concurrent_requests)),
            request_timeout,
            state_db_path,
        }
    }

    /// 确保 topic 已注册(只执行一次)
    ///
    /// 使用 OnceCell 保证线程安全的单次初始化
    /// 如果注册失败,会返回错误给调用方
    async fn ensure_topic_registered(&self) -> Result<(), jsonrpsee::types::ErrorObjectOwned> {
        self.topic_registered
            .get_or_try_init(|| async {
                debug!("正在注册 topic: {}", self.default_topic);
                match self.walrus_client.register(&self.default_topic).await {
                    Ok(_) => {
                        debug!("✅ Topic '{}' 注册成功", self.default_topic);
                        Ok(())
                    }
                    Err(e) => {
                        // 检查是否是"已存在"的错误
                        let err_msg = e.to_string();
                        if err_msg.contains("already exists")
                            || err_msg.contains("already registered")
                        {
                            debug!("Topic '{}' 已存在,跳过注册", self.default_topic);
                            // 对于"已存在"的情况,我们认为是成功的
                            Ok(())
                        } else {
                            error!("注册 topic '{}' 失败: {}", self.default_topic, err_msg);
                            Err(RpcError::WalrusWriteFailed.into_error_object(err_msg))
                        }
                    }
                }
            })
            .await
            .map_err(|e: jsonrpsee::types::ErrorObjectOwned| e)?;

        Ok(())
    }

    /// 将十六进制字符串转换为 Walrus 可以接受的格式
    fn ensure_hex_format(data: &str) -> String {
        if data.starts_with("0x") || data.starts_with("0X") {
            data.to_string()
        } else {
            format!("0x{}", data)
        }
    }

    /// 处理普通交易
    async fn process_transaction(
        &self,
        tx: Transaction,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        // 序列化交易为 JSON
        let tx_json = serde_json::to_string(&tx)
            .map_err(|e| RpcError::SerializationError.into_error_object(e.to_string()))?;

        // 转换为十六进制字符串
        let hex_data = hex::encode(tx_json.as_bytes());
        let hex_data = Self::ensure_hex_format(&hex_data);

        // 确保 topic 已注册（只会执行一次）
        self.ensure_topic_registered().await?;

        // 使用批量处理器或直接写入
        if let Some(batch_processor) = &self.batch_processor {
            batch_processor.submit(hex_data).await
        } else {
            self.write_to_walrus(hex_data).await
        }
    }

    /// 处理原始交易
    async fn process_raw_transaction(
        &self,
        data: String,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        let hex_data = Self::ensure_hex_format(&data);

        // 确保 topic 已注册（只会执行一次）
        self.ensure_topic_registered().await?;

        // 使用批量处理器或直接写入
        if let Some(batch_processor) = &self.batch_processor {
            batch_processor.submit(hex_data).await
        } else {
            self.write_to_walrus(hex_data).await
        }
    }

    /// 直接写入 Walrus
    async fn write_to_walrus(
        &self,
        hex_data: String,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        let start = Instant::now();

        self.walrus_client
            .put(&self.default_topic, &hex_data)
            .await
            .map_err(|e| RpcError::WalrusWriteFailed.into_error_object(e.to_string()))?;

        let duration = start.elapsed();
        WALRUS_WRITE_DURATION
            .with_label_values(&[&self.default_topic])
            .observe(duration.as_secs_f64());

        // 返回交易哈希
        let mut hasher = Sha256::new();
        hasher.update(hex_data.as_bytes());
        let hash_bytes = hasher.finalize();
        Ok(format!("0x{}", hex::encode(hash_bytes)))
    }
}

#[async_trait]
impl WalrusRpcApiServer for WalrusRpcServer {
    async fn send_transaction(
        &self,
        tx: Transaction,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        let start = Instant::now();
        TRANSACTIONS_TOTAL
            .with_label_values(&["send_transaction"])
            .inc();

        // 获取并发许可
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| RpcError::InternalError.into_error_object("获取并发许可失败"))?;

        debug!("收到交易: from={}, to={:?}", tx.from, tx.to);

        // 使用超时包装整个操作
        let result = tokio::time::timeout(self.request_timeout, self.process_transaction(tx)).await;

        let duration = start.elapsed();
        TRANSACTION_DURATION
            .with_label_values(&["send_transaction"])
            .observe(duration.as_secs_f64());

        match result {
            Ok(Ok(hash)) => {
                debug!("交易处理成功, hash: {}, 耗时: {:?}", hash, duration);
                Ok(hash)
            }
            Ok(Err(e)) => {
                TRANSACTIONS_FAILED
                    .with_label_values(&["send_transaction", &e.code().to_string()])
                    .inc();
                Err(e)
            }
            Err(_) => {
                TRANSACTIONS_FAILED
                    .with_label_values(&["send_transaction", "timeout"])
                    .inc();
                Err(RpcError::RequestTimeout.into_error_object("请求超时"))
            }
        }
    }

    async fn send_raw_transaction(
        &self,
        data: String,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        let start = Instant::now();
        TRANSACTIONS_TOTAL
            .with_label_values(&["send_raw_transaction"])
            .inc();

        // 获取并发许可
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| RpcError::InternalError.into_error_object("获取并发许可失败"))?;

        debug!("收到原始交易数据: {} bytes", data.len());

        // 使用超时包装整个操作
        let result =
            tokio::time::timeout(self.request_timeout, self.process_raw_transaction(data)).await;

        let duration = start.elapsed();
        TRANSACTION_DURATION
            .with_label_values(&["send_raw_transaction"])
            .observe(duration.as_secs_f64());

        match result {
            Ok(Ok(hash)) => {
                debug!("原始交易处理成功, hash: {}, 耗时: {:?}", hash, duration);
                Ok(hash)
            }
            Ok(Err(e)) => {
                TRANSACTIONS_FAILED
                    .with_label_values(&["send_raw_transaction", &e.code().to_string()])
                    .inc();
                Err(e)
            }
            Err(_) => {
                TRANSACTIONS_FAILED
                    .with_label_values(&["send_raw_transaction", "timeout"])
                    .inc();
                Err(RpcError::RequestTimeout.into_error_object("请求超时"))
            }
        }
    }

    async fn health(&self) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        // 通过调用 Walrus METRICS 命令验证连接状态
        match self.walrus_client.metrics().await {
            Ok(_metrics) => {
                debug!("✅ 健康检查通过: Walrus 连接正常");
                Ok("OK".to_string())
            }
            Err(e) => {
                warn!("❌ 健康检查失败: Walrus 连接异常 - {}", e);
                Err(RpcError::WalrusConnectionFailed.into_error_object(e.to_string()))
            }
        }
    }

    async fn get_transaction_count(
        &self,
        address: String,
        _block_number: Option<String>,
    ) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        debug!("收到 get_transaction_count 请求: address={}", address);

        // 1. 解析地址（移除 0x 前缀）
        let addr_hex = address.trim_start_matches("0x").trim_start_matches("0X");
        
        // 2. 解码十六进制地址
        let addr_bytes = hex::decode(addr_hex)
            .map_err(|_| RpcError::InvalidParams.into_error_object("Invalid address format"))?;
        
        // 3. 验证地址长度（必须是 20 字节）
        if addr_bytes.len() != 20 {
            return Err(RpcError::InvalidParams.into_error_object("Address must be 20 bytes"));
        }
        
        // 4. 转换为 Address 类型
        let addr = Address::from_slice(&addr_bytes);
        
        // 5. 尝试从状态数据库查询账户（短暂打开数据库）
        let nonce = match RedbStateDB::open_readonly(&self.state_db_path) {
            Ok(state_db) => {
                match state_db.get_account(&addr) {
                    Ok(account) => account.map(|a| a.nonce).unwrap_or(0),
                    Err(e) => {
                        warn!("查询账户失败: {}, 返回 nonce=0", e);
                        0
                    }
                }
            }
            Err(e) => {
                // 数据库可能被 block-producer 锁定，返回默认值 0
                warn!("无法打开状态数据库 (可能被锁定): {}, 返回 nonce=0", e);
                0
            }
        };
        
        debug!("获取到 nonce: address={:?}, nonce={}", addr, nonce);
        
        // 6. 返回十六进制格式的 nonce
        Ok(format!("0x{:x}", nonce))
    }
}

async fn start_rpc_server(args: Args) -> Result<ServerHandle> {
    let bind_addr = format!("{}:{}", args.rpc_host, args.rpc_port);

    info!("启动 JSON-RPC 服务器: {}", bind_addr);
    info!("Walrus 服务器地址: {}", args.walrus_addr);
    info!("默认 topic: {}", args.default_topic);
    info!("最大并发连接数: {}", args.max_connections);
    info!("最大并发请求数: {}", args.max_concurrent_requests);
    info!("批量处理间隔: {} ms", args.batch_interval_ms);
    info!("批量处理大小: {}", args.max_batch_size);

    // 配置 jsonrpsee Server
    // 注意：jsonrpsee 0.26.0 的并发控制主要通过底层 tokio runtime 和自定义中间件实现
    // 我们在应用层使用 Semaphore 来控制并发
    let server = Server::builder().build(&bind_addr).await?;

    let rpc_impl = WalrusRpcServer::new(
        args.walrus_addr.clone(),
        args.default_topic.clone(),
        args.max_concurrent_requests,
        Duration::from_millis(args.batch_interval_ms),
        args.max_batch_size,
        Duration::from_secs(args.request_timeout_secs),
        args.batch_interval_ms > 0, // 启用批量处理
        args.state_db_path.clone(),
    );

    let handle = server.start(rpc_impl.into_rpc());

    info!("✅ JSON-RPC 服务器已启动，监听地址: {}", bind_addr);
    info!("💡 可以使用 MetaMask 等钱包连接到此 RPC 端点");
    info!(
        "📊 性能指标端点: http://{}:{}/metrics",
        args.rpc_host,
        args.rpc_port + 1
    );

    Ok(handle)
}

/// 启动 Prometheus metrics HTTP 服务器
async fn start_metrics_server(_host: String, port: u16) -> Result<()> {
    use std::convert::Infallible;
    use std::net::SocketAddr;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let make_svc = hyper::service::make_service_fn(|_conn| async {
        Ok::<_, Infallible>(hyper::service::service_fn(|_req| async {
            let metrics = metrics::get_metrics();
            Ok::<_, Infallible>(hyper::Response::new(hyper::Body::from(metrics)))
        }))
    });

    info!("📊 Prometheus metrics 服务器启动: http://{}", addr);

    hyper::Server::bind(&addr).serve(make_svc).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    // 启动 metrics 服务器
    let metrics_host = args.rpc_host.clone();
    let metrics_port = args.rpc_port + 1;
    tokio::spawn(async move {
        if let Err(e) = start_metrics_server(metrics_host, metrics_port).await {
            error!("Metrics 服务器错误: {}", e);
        }
    });

    // 启动 RPC 服务器
    let handle = start_rpc_server(args).await?;

    info!("🚀 RPC Gateway 已完全启动");
    info!("💡 按 Ctrl+C 退出");

    // 保持运行
    handle.stopped().await;

    Ok(())
}
