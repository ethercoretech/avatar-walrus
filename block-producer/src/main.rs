use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use distributed_walrus::cli_client::CliClient;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::{info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};

/// 区块生产者（Block Producer）
/// 
/// 从 Walrus 集群读取交易，打包成区块，并提交给执行层
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Walrus 服务器地址
    #[arg(long, default_value = "127.0.0.1:9091")]
    walrus_addr: String,

    /// 监听的 topic 名称
    #[arg(long, default_value = "blockchain-txs")]
    topic: String,

    /// 出块间隔（秒）
    #[arg(long, default_value = "5")]
    block_interval: u64,

    /// 每个区块最大交易数
    #[arg(long, default_value = "100")]
    max_txs_per_block: usize,
}

/// 交易数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub data: String,
    pub gas: String,
    pub nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// 区块头
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// 区块号
    pub number: u64,
    /// 父区块哈希
    pub parent_hash: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 交易数量
    pub tx_count: usize,
    /// 交易根哈希
    pub transactions_root: String,
    /// 状态根哈希（执行后更新）
    pub state_root: Option<String>,
}

/// 区块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// 计算区块哈希
    pub fn hash(&self) -> String {
        let data = serde_json::to_string(&self.header).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("0x{:x}", hasher.finalize())
    }
}

/// 区块生产者
pub struct BlockProducer {
    walrus_client: CliClient,
    topic: String,
    block_interval: Duration,
    max_txs_per_block: usize,
    current_block_number: u64,
    last_block_hash: String,
}

impl BlockProducer {
    pub fn new(
        walrus_addr: String,
        topic: String,
        block_interval_secs: u64,
        max_txs_per_block: usize,
    ) -> Self {
        let walrus_client = CliClient::new(walrus_addr);
        Self {
            walrus_client,
            topic,
            block_interval: Duration::from_secs(block_interval_secs),
            max_txs_per_block,
            current_block_number: 0,
            last_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        }
    }

    /// 启动区块生产者主循环
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Block Producer 启动");
        info!("   Walrus topic: {}", self.topic);
        info!("   出块间隔: {}s", self.block_interval.as_secs());
        info!("   每块最大交易数: {}", self.max_txs_per_block);
        info!("");

        let mut interval = tokio::time::interval(self.block_interval);

        loop {
            interval.tick().await;
            
            match self.produce_block().await {
                Ok(block) => {
                    info!("✅ 区块 #{} 已生成", block.header.number);
                    info!("   区块哈希: {}", block.hash());
                    info!("   交易数量: {}", block.transactions.len());
                    info!("   父区块: {}", block.header.parent_hash);
                    info!("");
                }
                Err(e) => {
                    error!("❌ 生成区块失败: {}", e);
                }
            }
        }
    }

    /// 生成一个区块
    async fn produce_block(&mut self) -> Result<Block> {
        // 1. 从 Walrus 读取交易
        let transactions = self.fetch_transactions().await?;
        
        if transactions.is_empty() {
            info!("⏭️  没有待处理的交易，跳过本轮出块");
            return Err(anyhow::anyhow!("No transactions"));
        }

        // 2. 计算交易根哈希
        let transactions_root = self.calculate_transactions_root(&transactions);

        // 3. 构建区块头
        let header = BlockHeader {
            number: self.current_block_number,
            parent_hash: self.last_block_hash.clone(),
            timestamp: Utc::now(),
            tx_count: transactions.len(),
            transactions_root,
            state_root: None, // 执行后填充
        };

        // 4. 构建区块
        let block = Block {
            header,
            transactions,
        };

        // 5. 提交给执行层
        self.submit_to_execution_layer(&block).await?;

        // 6. 更新状态
        self.last_block_hash = block.hash();
        self.current_block_number += 1;

        Ok(block)
    }

    /// 从 Walrus 读取交易
    async fn fetch_transactions(&self) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();

        for _ in 0..self.max_txs_per_block {
            match self.walrus_client.get(&self.topic).await? {
                Some(hex_data) => {
                    match self.parse_transaction(&hex_data) {
                        Ok(tx) => transactions.push(tx),
                        Err(e) => {
                            warn!("解析交易失败: {}, 数据: {}", e, hex_data);
                            continue;
                        }
                    }
                }
                None => break, // 没有更多交易
            }
        }

        Ok(transactions)
    }

    /// 解析交易数据
    fn parse_transaction(&self, hex_data: &str) -> Result<Transaction> {
        // 移除 0x 前缀
        let hex_clean = hex_data.trim_start_matches("0x").trim_start_matches("0X");
        
        // 解码十六进制
        let bytes = hex::decode(hex_clean)?;
        
        // 转换为 UTF-8 字符串
        let json_str = String::from_utf8(bytes)?;
        
        // 解析 JSON
        let tx: Transaction = serde_json::from_str(&json_str)?;
        
        Ok(tx)
    }

    /// 计算交易根哈希
    fn calculate_transactions_root(&self, transactions: &[Transaction]) -> String {
        let mut hasher = Sha256::new();
        
        for tx in transactions {
            let tx_json = serde_json::to_string(tx).unwrap();
            hasher.update(tx_json.as_bytes());
        }
        
        format!("0x{:x}", hasher.finalize())
    }

    /// 提交区块给执行层
    async fn submit_to_execution_layer(&self, block: &Block) -> Result<()> {
        info!("📦 提交区块 #{} 到执行层...", block.header.number);
        
        // TODO: 实现执行层接口
        // 这里是执行层的占位符
        // 
        // 未来可以实现：
        // 1. EVM 执行引擎
        // 2. 状态更新
        // 3. 收据生成
        // 4. 事件日志
        
        self.execute_block_placeholder(block).await?;
        
        Ok(())
    }

    /// 执行层占位符实现
    async fn execute_block_placeholder(&self, block: &Block) -> Result<()> {
        info!("   [执行层占位符]");
        info!("   - 区块号: {}", block.header.number);
        info!("   - 交易数: {}", block.transactions.len());
        
        // 模拟执行延迟
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 未来在这里实现：
        // for tx in &block.transactions {
        //     execution_engine.execute(tx)?;
        // }
        
        info!("   ✓ 执行完成（模拟）");
        
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

    // 创建区块生产者
    let mut producer = BlockProducer::new(
        args.walrus_addr.clone(),
        args.topic.clone(),
        args.block_interval,
        args.max_txs_per_block,
    );

    // 启动
    producer.start().await?;

    Ok(())
}
