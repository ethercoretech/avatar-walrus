mod utils;

use anyhow::Result;
use clap::Parser;
use distributed_walrus::cli_client::CliClient;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{Server, ServerHandle};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};
use sha2::{Digest, Sha256};
use alloy_rlp::{RlpDecodable, Decodable};
use alloy_primitives::{Address, U256};

use utils::RpcError;

/// 以太坊 Legacy 交易结构（用于 RLP 解析）
#[derive(Debug, RlpDecodable)]
struct LegacyTransaction {
    nonce: U256,
    #[rlp(default)]
    to: Address,
    value: U256,
}

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
    /// 使用 OnceCell 确保 topic 只注册一次
    topic_registered: Arc<OnceCell<()>>,
}

impl WalrusRpcServer {
    pub fn new(walrus_addr: String, default_topic: String) -> Self {
        let walrus_client = CliClient::new(walrus_addr);
        Self {
            walrus_client,
            default_topic,
            topic_registered: Arc::new(OnceCell::new()),
        }
    }

    /// 确保 topic 已注册(只执行一次)
    /// 
    /// 使用 OnceCell 保证线程安全的单次初始化
    /// 如果注册失败,会返回错误给调用方
    async fn ensure_topic_registered(&self) -> Result<(), jsonrpsee::types::ErrorObjectOwned> {
        self.topic_registered
            .get_or_try_init(|| async {
                info!("正在注册 topic: {}", self.default_topic);
                match self.walrus_client.register(&self.default_topic).await {
                    Ok(_) => {
                        info!("✅ Topic '{}' 注册成功", self.default_topic);
                        Ok(())
                    }
                    Err(e) => {
                        // 检查是否是"已存在"的错误
                        let err_msg = e.to_string();
                        if err_msg.contains("already exists") || err_msg.contains("already registered") {
                            info!("Topic '{}' 已存在,跳过注册", self.default_topic);
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

    /// 验证并解析原始交易数据
    /// 
    /// 执行两级校验：
    /// 1. 验证是否为合法的 hex 字符串
    /// 2. 对 legacy 交易使用 alloy-rlp 解析 RLP 编码的交易结构
    ///    对 EIP-2718 typed 交易仅做 hex 校验，避免误判
    fn validate_raw_transaction(data: &str) -> Result<Vec<u8>, jsonrpsee::types::ErrorObjectOwned> {
        // 移除 0x 前缀
        let hex_str = data
            .strip_prefix("0x")
            .or_else(|| data.strip_prefix("0X"))
            .unwrap_or(data);
        
        // 第一步：验证是否为有效的 hex 字符串
        let raw_bytes = hex::decode(hex_str).map_err(|e| {
            RpcError::InvalidHexFormat.into_error_object(e.to_string())
        })?;

        if raw_bytes.is_empty() {
            return Err(RpcError::InvalidHexFormat.into_error_object("空的交易数据"));
        }

        let first_byte = raw_bytes[0];

        // 检测 EIP-2718 typed transaction（0x01..0x7f）
        // 这类交易的格式为：<tx_type_byte><RLP(交易字段)>
        // 我们只做 hex 校验即可，不强制解析为 LegacyTransaction。
        if first_byte >= 0x01 && first_byte <= 0x7f {
            info!("✅ 检测到 EIP-2718 typed transaction, tx_type={:#x}, size={} bytes", 
                  first_byte, raw_bytes.len());
            return Ok(raw_bytes);
        }
        
        // 第二步：尝试使用 alloy-rlp 解析 RLP 编码的 legacy 交易
        // 这会验证交易结构的完整性
        let mut slice = raw_bytes.as_slice();
        let tx = LegacyTransaction::decode(&mut slice).map_err(|e| {
            RpcError::InvalidTransaction.into_error_object(e.to_string())
        })?;
        
        info!("✅ Legacy 交易验证通过: to={:?}, value={}, nonce={}", 
              tx.to, tx.value, tx.nonce);
        
        Ok(raw_bytes)
    }
}

#[async_trait]
impl WalrusRpcApiServer for WalrusRpcServer {
    async fn send_transaction(&self, tx: Transaction) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        info!("收到交易: from={}, to={:?}", tx.from, tx.to);

        // 序列化交易为 JSON
        let tx_json = serde_json::to_string(&tx)
            .map_err(|e| RpcError::SerializationError.into_error_object(e.to_string()))?;

        // 转换为十六进制字符串
        let hex_data = hex::encode(tx_json.as_bytes());
        let hex_data = Self::ensure_hex_format(&hex_data);

        // 确保 topic 已注册（只会执行一次）
        self.ensure_topic_registered().await?;

        // 写入 Walrus
        self.walrus_client
            .put(&self.default_topic, &hex_data)
            .await
            .map_err(|e| RpcError::WalrusWriteFailed.into_error_object(e.to_string()))?;

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

        // 验证并解析原始交易（hex + RLP 解析）
        // let _raw_bytes = Self::validate_raw_transaction(&data)?;

        let hex_data = Self::ensure_hex_format(&data);

        // 确保 topic 已注册（只会执行一次）
        self.ensure_topic_registered().await?;

        // 直接写入 Walrus
        self.walrus_client
            .put(&self.default_topic, &hex_data)
            .await
            .map_err(|e| RpcError::WalrusWriteFailed.into_error_object(e.to_string()))?;

        // 返回交易哈希（基于写入 Walrus 的数据计算）
        let mut hasher = Sha256::new();
        hasher.update(hex_data.as_bytes());
        let hash_bytes = hasher.finalize();
        let tx_hash = format!("0x{}", hex::encode(hash_bytes));
        
        info!("原始交易已写入 Walrus, hash: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn health(&self) -> Result<String, jsonrpsee::types::ErrorObjectOwned> {
        // 通过调用 Walrus METRICS 命令验证连接状态
        match self.walrus_client.metrics().await {
            Ok(_metrics) => {
                info!("✅ 健康检查通过: Walrus 连接正常");
                Ok("OK".to_string())
            }
            Err(e) => {
                warn!("❌ 健康检查失败: Walrus 连接异常 - {}", e);
                Err(RpcError::WalrusConnectionFailed.into_error_object(e.to_string()))
            }
        }
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
