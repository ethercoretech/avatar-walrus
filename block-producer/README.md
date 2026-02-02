# Block Producer (区块生产者)

从 Walrus 集群读取交易，打包成区块，并提交给执行层。

## 功能特性

- ⏰ 定时出块（默认 5 秒）
- 📦 交易打包成区块
- 🔗 维护区块链结构（区块号、父哈希）
- 📝 计算交易根和区块哈希
- 🚀 异步高性能处理
- 🔌 执行层接口（占位符，待实现）


```text
MetaMask
   ↓
rpc-gateway (服务端口 8545)
   ↓
Walrus Cluster (服务端口 9091-9093)
   ↓
block-producer (每 5 秒读取消息队列，并切割生成一个区块)
   ↓
Execution Layer (使用 revm)
   ↓
State Database (使用 redb KV数据库)
```

---

## 快速开始

### 1. 启动 Walrus 集群

```bash
cd distributed-walrus
# 参考 docs/start-walrus-cluster.md 启动 3 节点集群
```

### 2. 启动 RPC Gateway（可选）

```bash
cd rpc-gateway
cargo run
```

### 3. 启动 Block Producer

```bash
cd block-producer

# 使用默认配置
cargo run

# 自定义配置
cargo run -- \
  --walrus-addr 127.0.0.1:9091 \
  --topic blockchain-txs \
  --block-interval 5 \
  --max-txs-per-block 100
```

---

## 工作流程

```
┌─────────────────┐
│  RPC Gateway    │  接收交易
└────────┬────────┘
         │ PUT
         ▼
┌─────────────────┐
│ Walrus Cluster  │  存储交易
└────────┬────────┘
         │ GET (每 5 秒)
         ▼
┌─────────────────┐
│ Block Producer  │  读取交易
│  - 打包成区块    │
│  - 计算哈希      │
└────────┬────────┘
         │ Submit
         ▼
┌─────────────────┐
│ Execution Layer │  执行区块（待实现）
│  - EVM 执行      │
│  - 状态更新      │
└─────────────────┘
```

---

## 配置

### 命令行参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--walrus-addr` | `127.0.0.1:9091` | Walrus 服务器地址 |
| `--topic` | `blockchain-txs` | 监听的交易 topic |
| `--block-interval` | `5` | 出块间隔（秒） |
| `--max-txs-per-block` | `100` | 每个区块最大交易数 |

### 环境变量

```bash
# 调整日志级别
RUST_LOG=debug cargo run

# 只看 block-producer 日志
RUST_LOG=block_producer=debug cargo run
```

---

## 区块结构

### Block

```rust
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}
```

### BlockHeader

```rust
pub struct BlockHeader {
    pub number: u64,              // 区块号
    pub parent_hash: String,      // 父区块哈希
    pub timestamp: DateTime<Utc>, // 时间戳
    pub tx_count: usize,          // 交易数量
    pub transactions_root: String,// 交易根哈希
    pub state_root: Option<String>, // 状态根（执行后填充）
}
```

---

## 示例输出

```
🚀 Block Producer 启动
   Walrus topic: blockchain-txs
   出块间隔: 5s
   每块最大交易数: 100

📦 提交区块 #0 到执行层...
   [执行层占位符]
   - 区块号: 0
   - 交易数: 3
   ✓ 执行完成（模拟）
✅ 区块 #0 已生成
   区块哈希: 0x1a2b3c4d...
   交易数量: 3
   父区块: 0x0000000000...

⏭️  没有待处理的交易，跳过本轮出块

📦 提交区块 #1 到执行层...
   [执行层占位符]
   - 区块号: 1
   - 交易数: 5
   ✓ 执行完成（模拟）
✅ 区块 #1 已生成
   区块哈希: 0x5e6f7a8b...
   交易数量: 5
   父区块: 0x1a2b3c4d...
```

---

## 开发指南

### 实现执行层

在 `submit_to_execution_layer` 方法中实现真实的执行逻辑：

```rust
async fn submit_to_execution_layer(&self, block: &Block) -> Result<()> {
    // 1. 初始化 EVM 执行器
    let mut executor = EVMExecutor::new();
    
    // 2. 执行每笔交易
    for tx in &block.transactions {
        let receipt = executor.execute(tx)?;
        // 处理执行结果
    }
    
    // 3. 更新状态根
    let state_root = executor.get_state_root();
    
    // 4. 生成收据和事件
    let receipts = executor.get_receipts();
    
    Ok(())
}
```

### 添加状态持久化

可以将区块存储到Redb数据库：

```rust
// 写入到 Redb数据库
let block_json = serde_json::to_string(&block)?;
let block_hex = format!("0x{}", hex::encode(block_json.as_bytes()));
self.walrus_client.put("blocks", &block_hex).await?;
```

### 添加共识机制

当前是单节点排序器，可以扩展为：
- 多节点选举（基于 Raft）
- PoS 共识
- 拜占庭容错（BFT）

---

## 测试

### 发送测试交易

```bash
# 使用 RPC Gateway 发送交易
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendTransaction",
    "params": [{
      "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
      "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
      "value": "0xde0b6b3a7640000",
      "data": "0x",
      "gas": "0x5208",
      "nonce": "0x0"
    }],
    "id": 1
  }'
```

### 观察出块

等待 5 秒，Block Producer 会自动读取并打包交易。

---

## 性能调优

### 调整出块间隔

```bash
# 更快出块（1 秒）
cargo run -- --block-interval 1

# 更慢出块（10 秒）
cargo run -- --block-interval 10
```

### 调整区块大小

```bash
# 更大的区块
cargo run -- --max-txs-per-block 500
```
