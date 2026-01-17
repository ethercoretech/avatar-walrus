use anyhow::Result;
use clap::Parser;
use distributed_walrus::cli_client::CliClient;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{Server, ServerHandle};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};
use sha2::{Digest, Sha256};

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

/// JSON-RPC API 定义
#[rpc(server)]
pub trait WalrusRpcApi {
    /// 提交交易到 Walrus
    #[method(name = "eth_sendTransaction")]
    async fn send_transaction(&self, tx: Transaction) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;

    /// 提交原始交易数据
    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, data: String) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;

    /// 健康检查
    #[method(name = "health")]
    async fn health(&self) -> Result<String, jsonrpsee::types::ErrorObjectOwned>;
}

/// RPC 服务实现
pub struct WalrusRpcServer {
    walrus_client: CliClient,
    default_topic: String,
}

impl WalrusRpcServer {
    pub fn new(walrus_addr: String, default_topic: String) -> Self {
        let walrus_client = CliClient::new(walrus_addr);
        Self {
            walrus_client,
            default_topic,
        }
    }

    /// 将十六进制字符串转换为 Walrus 可以接受的格式
    fn ensure_hex_format(data: &str) -> String {
        if data.starts_with("0x") || data.starts_with("0X") {
            data.to_string()
        } else {
            format!("0x{}", data)
        }
    }
}

#[async_trait]
impl WalrusRpcApiServer for WalrusRpcServer {
    async fn send_transaction(&self, tx: Transaction) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        info!("收到交易: from={}, to={:?}", tx.from, tx.to);

        // 序列化交易为 JSON
        let tx_json = serde_json::to_string(&tx)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("序列化失败: {}", e),
                None::<String>,
            ))?;

        // 转换为十六进制字符串
        let hex_data = hex::encode(tx_json.as_bytes());
        let hex_data = Self::ensure_hex_format(&hex_data);

        // 确保 topic 存在
        if let Err(e) = self.walrus_client.register(&self.default_topic).await {
            warn!("注册 topic 失败 (可能已存在): {}", e);
        }

        // 写入 Walrus
        self.walrus_client
            .put(&self.default_topic, &hex_data)
            .await
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                format!("写入 Walrus 失败: {}", e),
                None::<String>,
            ))?;

        // 返回稳定的交易哈希（基于写入 Walrus 的数据计算）
        let mut hasher = Sha256::new();
        hasher.update(hex_data.as_bytes());
        let hash_bytes = hasher.finalize();
        let tx_hash = format!("0x{}", hex::encode(hash_bytes));
        
        info!("交易已写入 Walrus, hash: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn send_raw_transaction(&self, data: String) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        info!("收到原始交易数据: {} bytes", data.len());

        let hex_data = Self::ensure_hex_format(&data);

        // 确保 topic 存在
        if let Err(e) = self.walrus_client.register(&self.default_topic).await {
            warn!("注册 topic 失败 (可能已存在): {}", e);
        }

        // 直接写入 Walrus
        self.walrus_client
            .put(&self.default_topic, &hex_data)
            .await
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                format!("写入 Walrus 失败: {}", e),
                None::<String>,
            ))?;

        // 返回交易哈希（基于写入 Walrus 的数据计算）
        let mut hasher = Sha256::new();
        hasher.update(hex_data.as_bytes());
        let hash_bytes = hasher.finalize();
        let tx_hash = format!("0x{}", hex::encode(hash_bytes));
        
        info!("原始交易已写入 Walrus, hash: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn health(&self) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        Ok("OK".to_string())
    }
}

async fn start_rpc_server(args: Args) -> Result<ServerHandle> {
    let bind_addr = format!("{}:{}", args.rpc_host, args.rpc_port);
    
    info!("启动 JSON-RPC 服务器: {}", bind_addr);
    info!("Walrus 服务器地址: {}", args.walrus_addr);
    info!("默认 topic: {}", args.default_topic);

    let server = Server::builder()
        .build(&bind_addr)
        .await?;

    let rpc_impl = WalrusRpcServer::new(
        args.walrus_addr.clone(),
        args.default_topic.clone(),
    );

    let handle = server.start(rpc_impl.into_rpc());

    info!("✅ JSON-RPC 服务器已启动，监听地址: {}", bind_addr);
    info!("💡 可以使用 MetaMask 等钱包连接到此 RPC 端点");

    Ok(handle)
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    // 启动 RPC 服务器
    let handle = start_rpc_server(args).await?;

    // 保持运行
    handle.stopped().await;

    Ok(())
}
