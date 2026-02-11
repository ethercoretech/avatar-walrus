use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use distributed_walrus::cli_client::CliClient;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{fmt, EnvFilter};

// === 使用 lib 中的模块 ===
use block_producer::{db, schema, trie, executor, utils, wallet};

// === 区块链常量配置 ===
// 使用 lib.rs 中定义的常量，保持单一来源
use block_producer::DEFAULT_BLOCK_GAS_LIMIT;

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
    #[arg(long, default_value = "10000")]
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
    /// Gas 使用量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<u64>,
    /// Gas 限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<u64>,
    /// 收据根哈希
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipts_root: Option<String>,
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
    
    // ===== 交易池 (类似 Reth 设计) =====
    /// 待处理交易池：存储从 Walrus 读取但尚未打包的交易
    pending_pool: VecDeque<Transaction>,
    
    /// 交易池最大容量（避免无限增长）
    pool_max_size: usize,
}

impl BlockProducer {
    pub fn new(
        walrus_addr: String,
        topic: String,
        block_interval_secs: u64,
        max_txs_per_block: usize,
    ) -> Self {
        let walrus_client = CliClient::new(walrus_addr);
        let pool_max_size = max_txs_per_block * 10; // 交易池容量为单区块的10倍
        
        Self {
            walrus_client,
            topic,
            block_interval: Duration::from_secs(block_interval_secs),
            max_txs_per_block,
            current_block_number: 0,
            last_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            
            // 初始化交易池
            pending_pool: VecDeque::new(),
            pool_max_size,
        }
    }

    /// 启动区块生产者主循环
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Block Producer 启动");
        info!("   Walrus topic: {}", self.topic);
        info!("   出块间隔: {}s", self.block_interval.as_secs());
        info!("   每块最大交易数: {}", self.max_txs_per_block);
        info!("   交易池容量: {} 笔", self.pool_max_size);
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
        // 1. 从交易池选择交易（而不是直接从 Walrus 读取）
        let transactions = self.select_transactions_for_block().await?;
            
        if transactions.is_empty() {
            info!("⭭️  交易池为空，跳过本轮出块");
            return Err(anyhow::anyhow!("No transactions in pool"));
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
            gas_used: None,
            gas_limit: Some(DEFAULT_BLOCK_GAS_LIMIT), // 默认 gas 限制
            receipts_root: None,
        };
    
        // 4. 构建区块
        let mut block = Block {
            header,
            transactions,
        };
    
        // 5. 提交给执行层（会更新 state_root 和 gas_used）
        self.submit_to_execution_layer(&mut block).await?;
    
        // 6. 更新状态
        self.last_block_hash = block.hash();
        self.current_block_number += 1;
    
        Ok(block)
    }
    
    /// 从 Walrus 补充交易池
    async fn refill_pool(&mut self) -> Result<()> {
        let initial_size = self.pending_pool.len();
        let mut fetched = 0;
        
        while self.pending_pool.len() < self.pool_max_size {
            match self.walrus_client.get(&self.topic).await? {
                Some(hex_data) => {
                    match self.parse_transaction(&hex_data) {
                        Ok(tx) => {
                            self.pending_pool.push_back(tx);
                            fetched += 1;
                        }
                        Err(e) => {
                            warn!("解析交易失败: {}, 数据: {}", e, hex_data);
                            continue;
                        }
                    }
                }
                None => break,
            }
        }
        
        if fetched > 0 {
            debug!("交易池补充: {} -> {} (新增 {})", 
                   initial_size, self.pending_pool.len(), fetched);
        }
        
        Ok(())
    }
    
    /// 从交易池选择交易打包
    async fn select_transactions_for_block(&mut self) -> Result<Vec<Transaction>> {
        self.refill_pool().await?;
        
        if self.pending_pool.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut candidates: Vec<Transaction> = self.pending_pool.drain(..).collect();
        
        info!("📋 开始交易选择: 候选交易 {} 笔", candidates.len());
        
        // 按 gas price 降序排序（优先打包高价交易）
        candidates.sort_by(|a, b| {
            let a_price = Self::parse_gas_price(&a.gas).unwrap_or(0);
            let b_price = Self::parse_gas_price(&b.gas).unwrap_or(0);
            b_price.cmp(&a_price)
        });
        
        let mut selected = Vec::new();
        let mut estimated_gas = 0u64;
        // 统一使用常量作为 gas limit 来源
        let block_gas_limit = DEFAULT_BLOCK_GAS_LIMIT;
        let mut skipped_by_gas = 0;
        
        debug!("⛽ 区块 gas 限制: {}", block_gas_limit);
        
        for (idx, tx) in candidates.into_iter().enumerate() {
            let tx_gas = Self::parse_gas_limit(&tx.gas).unwrap_or(21000);
            let tx_hash_display = tx.hash.as_deref().unwrap_or("unknown");
            
            // 移除 max_txs_per_block 的硬性限制，只检查 gas
            if estimated_gas + tx_gas <= block_gas_limit {
                estimated_gas += tx_gas;
                
                debug!(
                    "  ✓ 选择交易 #{}: hash={}, gas={}, 累计={}/{} ({:.1}%)",
                    idx + 1,
                    tx_hash_display,
                    tx_gas,
                    estimated_gas,
                    block_gas_limit,
                    (estimated_gas as f64 / block_gas_limit as f64) * 100.0
                );
                
                selected.push(tx);
            } else {
                // Gas 不足，无法容纳此交易
                skipped_by_gas += 1;
                debug!(
                    "  ✗ 跳过交易 #{}: hash={}, gas={} (剩余空间不足: {}/{})",
                    idx + 1,
                    tx_hash_display,
                    tx_gas,
                    block_gas_limit - estimated_gas,
                    block_gas_limit
                );
                
                // 放回队列，供下次打包
                self.pending_pool.push_front(tx);
            }
        }
        
        // 输出详细的选择统计
        info!(
            "✅ 交易选择完成: 已选 {} 笔, 预估 gas {}/{} ({:.1}%), 跳过 {} 笔 (gas不足)",
            selected.len(),
            estimated_gas,
            block_gas_limit,
            (estimated_gas as f64 / block_gas_limit as f64) * 100.0,
            skipped_by_gas
        );
        info!("📦 交易池剩余: {} 笔", self.pending_pool.len());
        
        Ok(selected)
    }
    
    /// 将执行失败的交易放回池中
    fn return_to_pool(&mut self, transactions: Vec<Transaction>) {
        if transactions.is_empty() {
            return;
        }
        
        debug!("️ 将 {} 笔交易放回交易池", transactions.len());
        
        for tx in transactions {
            if self.pending_pool.len() >= self.pool_max_size {
                warn!("️ 交易池已满，丢弃交易: {:?}", tx.hash);
                break;
            }
            self.pending_pool.push_front(tx);
        }
    }
    
    fn parse_gas_price(gas_hex: &str) -> Result<u64> {
        let hex = gas_hex.trim_start_matches("0x");
        u64::from_str_radix(hex, 16)
            .map_err(|e| anyhow::anyhow!("Invalid gas: {}", e))
    }
    
    fn parse_gas_limit(gas_hex: &str) -> Result<u64> {
        let hex = gas_hex.trim_start_matches("0x");
        u64::from_str_radix(hex, 16)
            .map_err(|e| anyhow::anyhow!("Invalid gas: {}", e))
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
    async fn submit_to_execution_layer(&self, block: &mut Block) -> Result<()> {
        info!("📦 提交区块 #{} 到执行层...", block.header.number);
        
        use block_producer::db::RedbStateDB;
        use block_producer::executor::block_executor::BlockExecutor;
        use block_producer::utils::calculate_merkle_root;
        
        // 1. 初始化状态数据库
        let db_path = format!("./data/block_producer_state_{}.redb", self.topic);
        let state_db = RedbStateDB::new(&db_path)
            .map_err(|e| anyhow::anyhow!("Failed to create state DB: {}", e))?;
        
        // 2. 创建区块执行器
        let mut executor = BlockExecutor::new(state_db);
        
        // 3. 转换区块格式（从旧格式到新格式）
        let schema_block = self.convert_to_schema_block(block)?;
        
        // 4. 执行区块
        let execution_result = executor.execute_block(&schema_block).await
            .map_err(|e| anyhow::anyhow!("Block execution failed: {}", e))?;
        
        // 5. 计算状态根
        let state_root = executor.calculate_state_root()
            .map_err(|e| anyhow::anyhow!("State root calculation failed: {}", e))?;
        
        // 6. 计算交易根
        let transactions_root = calculate_merkle_root(&schema_block.transactions);
        
        // 7. 计算收据根
        let receipts: Vec<_> = execution_result.receipts.values().cloned().collect();
        let receipts_root = if !receipts.is_empty() {
            calculate_merkle_root(&receipts)
        } else {
            block_producer::utils::EMPTY_ROOT_HASH
        };
        
        // 8. 更新区块头
        block.header.state_root = Some(format!("0x{}", hex::encode(state_root.as_slice())));
        block.header.gas_used = Some(execution_result.total_gas_used);
        block.header.transactions_root = format!("0x{}", hex::encode(transactions_root.as_slice()));
        block.header.receipts_root = Some(format!("0x{}", hex::encode(receipts_root.as_slice())));
        
        // 9. 持久化区块到数据库
        executor.db_mut().save_block(&schema_block)
            .map_err(|e| anyhow::anyhow!("Failed to save block: {}", e))?;
        
        info!("   ✓ 执行完成: {} 成功, {} 失败",
              execution_result.successful_txs,
              execution_result.failed_txs);
        info!("   ✓ 状态根: 0x{}", hex::encode(state_root.as_slice()));
        info!("   ✓ Gas 使用: {}", execution_result.total_gas_used);
        
        Ok(())
    }
    
    /// 转换区块格式
    fn convert_to_schema_block(&self, block: &Block) -> Result<block_producer::schema::Block> {
        use block_producer::schema::{Block as SchemaBlock, BlockHeader as SchemaHeader, Transaction as SchemaTx};
        
        // 转换交易列表
        let transactions: Vec<SchemaTx> = block.transactions.iter().map(|tx| {
            SchemaTx {
                from: tx.from.clone(),
                to: tx.to.clone(),
                value: tx.value.clone(),
                data: tx.data.clone(),
                gas: tx.gas.clone(),
                nonce: tx.nonce.clone(),
                hash: tx.hash.clone(),
                gas_price: None,
                chain_id: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
            }
        }).collect();
        
        Ok(SchemaBlock {
            header: SchemaHeader {
                number: block.header.number,
                parent_hash: block.header.parent_hash.clone(),
                timestamp: block.header.timestamp,
                tx_count: block.transactions.len(),
                transactions_root: block.header.transactions_root.clone(),
                state_root: block.header.state_root.clone(),
                gas_used: block.header.gas_used,
                gas_limit: block.header.gas_limit,
                receipts_root: block.header.receipts_root.clone(),
            },
            transactions,
        })
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
