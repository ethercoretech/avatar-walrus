use anyhow::Result;
use clap::{Parser, Subcommand};
use alloy::{
    consensus::{TxLegacy, TxEnvelope},
    eips::eip2718::Encodable2718,
    network::TxSigner,
    primitives::{Address, Bytes, U256},
    signers::local::PrivateKeySigner,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

/// 交易生成器
/// 
/// 生成以太坊密钥、签名交易并发送到 RPC Gateway
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 生成新的密钥对
    GenerateKey,
    
    /// 生成并发送单笔交易
    SendTx {
        /// 私钥（64 位十六进制，可选 0x 前缀）
        #[arg(long)]
        private_key: String,
        
        /// 接收地址
        #[arg(long)]
        to: String,
        
        /// 转账金额（ETH）
        #[arg(long, default_value = "1.0")]
        value: f64,
        
        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,
    },
    
    /// 批量生成测试交易
    BatchGenerate {
        /// 批次大小
        #[arg(long, default_value = "10")]
        count: usize,
        
        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,
        
        /// 发送间隔（毫秒）
        #[arg(long, default_value = "100")]
        interval_ms: u64,
    },
}

/// JSON-RPC 请求
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<serde_json::Value>,
    id: u64,
}

/// JSON-RPC 响应
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// 交易生成器
struct TxGenerator {
    rpc_url: String,
    client: reqwest::Client,
}

impl TxGenerator {
    fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    /// 生成新的密钥对
    fn generate_keypair() -> Result<PrivateKeySigner> {
        let signer = PrivateKeySigner::random();
        Ok(signer)
    }

    /// 创建交易
    fn create_transaction(
        to: Address,
        value: U256,
        nonce: u64,
    ) -> TxLegacy {
        TxLegacy {
            chain_id: Some(1337), // 测试链 ID
            nonce,
            gas_price: 20_000_000_000, // 20 Gwei
            gas_limit: 21000,          // 标准转账 Gas
            to: to.into(),
            value,
            input: Bytes::new(),
        }
    }

    /// 签名交易
    async fn sign_transaction(
        signer: &PrivateKeySigner,
        tx: TxLegacy,
    ) -> Result<String> {
        // 使用 TxSigner trait 的 sign_transaction 方法
        let signature = signer.sign_transaction(&mut tx.clone()).await?;
        
        // 构建签名的交易 envelope
        let envelope = TxEnvelope::Legacy(alloy::consensus::Signed::new_unchecked(
            tx,
            signature,
            Default::default(),
        ));
        
        // 编码为原始交易
        let encoded = envelope.encoded_2718();
        Ok(format!("0x{}", hex::encode(encoded)))
    }

    /// 发送交易到 RPC Gateway
    async fn send_transaction(&self, raw_tx: &str) -> Result<String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendRawTransaction".to_string(),
            params: vec![serde_json::json!(raw_tx)],
            id: 1,
        };

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json_response: JsonRpcResponse = response.json().await?;

        if let Some(error) = json_response.error {
            anyhow::bail!("RPC Error: {} ({})", error.message, error.code);
        }

        let tx_hash = json_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid result format"))?
            .to_string();

        Ok(tx_hash)
    }

    /// 生成并发送单笔交易
    async fn generate_and_send(
        &self,
        private_key: &str,
        to_address: &str,
        value_eth: f64,
    ) -> Result<String> {
        // 1. 加载签名器
        let signer = private_key.parse::<PrivateKeySigner>()?;
        let from_address = signer.address();
        info!("发送地址: {:?}", from_address);

        // 2. 解析接收地址
        let to = Address::from_str(to_address)?;

        // 3. 转换金额（ETH to Wei）
        let value = U256::from((value_eth * 1e18) as u64);

        // 4. 创建交易（使用随机 nonce 用于测试）
        let nonce = rand::thread_rng().gen::<u32>() as u64;
        let tx = Self::create_transaction(to, value, nonce);

        info!("创建交易: {:?} -> {:?}, 金额: {} ETH", from_address, to, value_eth);

        // 5. 签名交易
        let raw_tx = Self::sign_transaction(&signer, tx).await?;
        info!("交易已签名, 原始交易: {}...", &raw_tx[..20]);

        // 6. 发送交易
        let tx_hash = self.send_transaction(&raw_tx).await?;
        info!("✅ 交易已发送, 哈希: {}", tx_hash);

        Ok(tx_hash)
    }

    /// 批量生成测试交易
    async fn batch_generate(&self, count: usize, interval_ms: u64) -> Result<()> {
        info!("🚀 开始批量生成 {} 笔测试交易", count);

        for i in 0..count {
            // 生成随机密钥对
            let signer = Self::generate_keypair()?;
            
            // 生成随机接收地址
            let to_signer = Self::generate_keypair()?;
            let to_address = to_signer.address();
            
            // 随机金额（0.1 - 10 ETH）
            let value_eth = rand::thread_rng().gen_range(0.1..10.0);
            
            // 创建交易
            let nonce = i as u64;
            let value = U256::from((value_eth * 1e18) as u64);
            let tx = Self::create_transaction(
                to_address,
                value,
                nonce,
            );

            // 签名
            let raw_tx = Self::sign_transaction(&signer, tx).await?;

            // 发送
            match self.send_transaction(&raw_tx).await {
                Ok(tx_hash) => {
                    info!(
                        "[{}/{}] ✅ 交易已发送: {} ({:.2} ETH)",
                        i + 1,
                        count,
                        &tx_hash[..16],
                        value_eth
                    );
                }
                Err(e) => {
                    warn!("[{}/{}] ❌ 发送失败: {}", i + 1, count, e);
                }
            }

            // 等待间隔
            if i < count - 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        }

        info!("🎉 批量生成完成！");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    match args.command {
        Command::GenerateKey => {
            let signer = TxGenerator::generate_keypair()?;
            println!("🔑 新密钥对已生成:");
            println!();
            println!("地址:     {:?}", signer.address());
            println!("私钥:     {}", hex::encode(signer.to_bytes()));
            println!();
            println!("⚠️  请妥善保管私钥，不要泄露！");
        }

        Command::SendTx {
            private_key,
            to,
            value,
            rpc_url,
        } => {
            let generator = TxGenerator::new(rpc_url);
            let tx_hash = generator.generate_and_send(&private_key, &to, value).await?;
            println!("✅ 交易哈希: {}", tx_hash);
        }

        Command::BatchGenerate {
            count,
            rpc_url,
            interval_ms,
        } => {
            let generator = TxGenerator::new(rpc_url);
            generator.batch_generate(count, interval_ms).await?;
        }
    }

    Ok(())
}
