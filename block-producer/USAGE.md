# Block Producer 完整使用手册

> 从发送一笔 JSON-RPC 交易开始到区块生成、执行、存储的全流程指南

## 🚀 快速开始 (推荐)

使用一键启动脚本快速启动整个系统：

```bash
# 启动完整系统 (Walrus + RPC Gateway + Block Producer)
./scripts/start_full_system.sh start

# 监控区块生成
./scripts/monitor_blocks.sh

# 发送测试交易
cd block-producer
./scripts/send_test_transaction.sh 5

# 验证数据库状态
./scripts/verify_database.sh
```

> **注意**: 首次运行需要编译，可能需要几分钟时间。

---

## 📋 目录

1. [快速开始](#快速开始-推荐)
2. [系统架构概览](#系统架构概览)
3. [前置准备](#前置准备)
4. [启动完整流程](#启动完整流程)
5. [实用脚本工具](#实用脚本工具)
6. [发送交易测试](#发送交易测试)
7. [观察执行结果](#观察执行结果)
8. [数据存储验证](#数据存储验证)
9. [故障排除](#故障排除)
10. [性能监控](#性能监控)

---

## 系统架构概览

```
┌─────────────┐    ┌──────────────┐    ┌────────────────┐    ┌──────────────┐    ┌────────────────┐
│  MetaMask   │───▶│ RPC Gateway  │───▶│ Walrus Cluster │───▶│ Block        │───▶│ Redb State     │
│   (钱包)    │    │  (端口8545)  │    │ (端口9091-9093)│    │  Producer    │    │  Database      │
└─────────────┘    └──────────────┘    └────────────────┘    │ (执行引擎)   │    │ (状态存储)     │
                                                              └──────────────┘    └────────────────┘
                                                                    │                    │
                                                                    ▼                    ▼
                                                              ┌──────────────┐    ┌────────────────┐
                                                              │ EVM Executor │    │ Block Storage  │
                                                              │ (REVM)       │    │ (区块持久化)   │
                                                              └──────────────┘    └────────────────┘
```

**数据流向**:
1. 用户通过 MetaMask 发送交易到 RPC Gateway
2. RPC Gateway 将交易批量写入 Walrus 集群
3. Block Producer 定期从 Walrus 读取交易
4. 执行引擎(REVM)执行交易并计算状态变化
5. 结果写入 Redb 数据库并生成新区块

---

## 前置准备

### 1. 环境要求

```bash
# Rust 版本 (建议 1.75+)
rustc --version

# 系统依赖
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

### 2. 编译项目

```bash
# 进入项目根目录
cd /opt/rust/project/avatar-walrus

# 编译所有组件 (首次可能需要几分钟)
cargo build --release
```

### 3. 准备数据目录

```bash
mkdir -p block-producer/data
```

---

## 启动完整流程

> ⚠️ **重要**: 必须按以下顺序启动所有组件

### 步骤 1: 启动 Walrus 集群

```bash
# 方式一: 使用管理脚本 (推荐)
./scripts/start_walrus_cluster.sh start

# 方式二: 手动启动三个终端
# 终端 1
./scripts/start_walrus_cluster.sh start

# 验证集群状态
./scripts/start_walrus_cluster.sh status
```

**预期输出**:
```
[SUCCESS] Walrus 集群已启动！
客户端端口:
  - 节点 1: 127.0.0.1:9091
  - 节点 2: 127.0.0.1:9092
  - 节点 3: 127.0.0.1:9093
```

### 步骤 2: 启动 RPC Gateway

```bash
# 新终端窗口
cd rpc-gateway
cargo run --release
```

**预期输出**:
```
🚀 RPC Gateway 已完全启动
💡 按 Ctrl+C 退出
✅ JSON-RPC 服务器已启动，监听地址: 127.0.0.1:8545
```

### 步骤 3: 启动 Block Producer

```bash
# 新终端窗口
cd block-producer
cargo run --release
```

**预期输出**:
```
🚀 Block Producer 启动
   Walrus topic: blockchain-txs
   出块间隔: 5s
   每块最大交易数: 10000
```

---

## 实用脚本工具

项目提供了一系列实用脚本，简化日常开发和测试工作。

### 1. 一键启动脚本

```bash
# 启动完整系统
./scripts/start_full_system.sh start

# 查看系统状态
./scripts/start_full_system.sh status

# 停止系统
./scripts/start_full_system.sh stop
```

**功能特点**:
- 自动检查依赖和端口占用
- 按正确顺序启动所有组件
- 后台运行并记录 PID
- 提供详细的启动反馈

### 2. 交易发送脚本

```bash
# 发送指定数量的测试交易
./scripts/send_test_transaction.sh 10

# 脚本会自动:
# - 检查 RPC 连接
# - 发送多笔交易
# - 统计成功率
# - 显示交易哈希
```

### 3. 区块监控脚本

```bash
# 实时监控区块生成
./scripts/monitor_blocks.sh

# 显示内容:
# - 区块生成通知
# - 执行结果统计
# - Gas 使用情况
# - 系统进程状态
```

### 4. 数据库验证脚本

```bash
# 验证数据库状态
./scripts/verify_database.sh

# 检查内容:
# - 数据库文件完整性
# - 区块链状态
# - Walrus 存储状态
# - 系统性能指标
```

### 5. 手动启动方式

如果不使用一键脚本，也可以手动启动各个组件：

```bash
# 1. 启动 Walrus 集群
./scripts/start_walrus_cluster.sh start

# 2. 启动 RPC Gateway (新终端)
cd rpc-gateway
cargo run --release

# 3. 启动 Block Producer (新终端)
cd block-producer
cargo run --release
```

---

## 发送交易测试

### 方式一: 使用 curl 命令

```bash
# 发送简单转账交易
curl -X POST http://127.0.0.1:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendTransaction",
    "params": [{
      "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc",
      "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
      "value": "0xde0b6b3a7640000",  # 1 ETH
      "gas": "0x5208",            # 21000 gas
      "nonce": "0x0"
    }],
    "id": 1
  }'
```

**成功响应**:
```json
{
  "jsonrpc": "2.0",
  "result": "0x123456789abcdef...",
  "id": 1
}
```

### 方式二: 使用 MetaMask 钱包

1. 打开 MetaMask 插件
2. 添加自定义网络:
   - 网络名称: `Local Walrus Chain`
   - RPC URL: `http://127.0.0.1:8545`
   - Chain ID: `1337`
   - Currency Symbol: `ETH`
3. 导入测试账户私钥 (如需要)
4. 发送交易

### 方式三: 批量发送交易脚本

创建测试脚本 `send_bulk_txs.sh`:

```bash
#!/bin/bash

for i in {1..10}; do
  curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{
      "jsonrpc": "2.0",
      "method": "eth_sendTransaction",
      "params": [{
        "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc",
        "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "value": "0x'$(printf '%x' $((RANDOM % 1000000000000000)))'",
        "gas": "0x5208",
        "nonce": "0x'$(printf '%x' $i)'"
      }],
      "id": '$i'
    }' &
done

wait
```

```bash
chmod +x send_bulk_txs.sh
./send_bulk_txs.sh
```

---

## 观察执行结果

### 1. Block Producer 日志观察

在 Block Producer 终端中应该看到类似输出:

```
📋 开始交易选择: 候选交易 5 笔
✅ 交易选择完成: 已选 5 笔, 预估 gas 105000/30000000 (0.4%), 跳过 0 笔 (gas不足)
📦 交易池剩余: 0 笔

📦 提交区块 #1 到执行层...
   ✓ 执行完成: 5 成功, 0 失败
   ✓ 状态根: 0xa1b2c3d4...
   ✓ Gas 使用: 105000

✅ 区块 #1 已生成
   区块哈希: 0xe5f6a7b8...
   交易数量: 5
   父区块: 0x12345678...
```

### 2. 关键指标解读

- **交易选择**: 显示从交易池中选出多少笔交易用于打包
- **Gas 使用**: 实际消耗的 Gas 数量
- **状态根**: Merkle Patricia Trie 的根哈希，代表世界状态
- **执行结果**: 成功/失败的交易数量

### 3. 实时监控脚本

创建 `monitor_blocks.sh`:

```bash
#!/bin/bash

echo "=== Block Producer 实时监控 ==="
echo "按 Ctrl+C 停止"
echo ""

tail -f block-producer/target/debug/block-producer.log 2>/dev/null | grep -E "(区块 #[0-9]+|✓ 执行完成|状态根|Gas 使用)"
```

---

## 数据存储验证

### 1. 查看生成的区块文件

```bash
# Block Producer 会在 data/ 目录下创建状态数据库
ls -la block-producer/data/

# 应该看到类似文件:
# block_producer_state_blockchain-txs.redb
```

### 2. 查询区块数据

使用 Walrus CLI 查询存储的数据:

```bash
# 连接到 Walrus 集群
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091

# 在 CLI 中执行:
> STATE blocks
> GET blocks
```

### 3. 验证状态数据库

创建验证脚本 `verify_state.rs`:

```rust
use block_producer::db::RedbStateDB;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let db = RedbStateDB::new("./block-producer/data/block_producer_state_blockchain-txs.redb")?;
    
    println!("=== 状态数据库验证 ===");
    
    // 查询最新区块
    if let Some(latest_block) = db.get_latest_block()? {
        println!("最新区块号: {}", latest_block.header.number);
        println!("区块哈希: {}", latest_block.hash());
        println!("交易数量: {}", latest_block.transactions.len());
        println!("状态根: {:?}", latest_block.header.state_root);
    }
    
    // 查询账户余额
    let account = "0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc";
    if let Some(balance) = db.get_account_balance(account)? {
        println!("账户 {} 余额: {} wei", account, balance);
    }
    
    Ok(())
}
```

运行验证:

```bash
cd block-producer
cargo run --example verify_state
```

### 4. 数据库结构说明

Redb 数据库存储以下表:

```
blockchain-txs.redb
├── blocks          # 区块数据
├── accounts        # 账户状态
├── storage         # 存储槽
├── code            # 合约字节码
└── receipts        # 交易收据
```

---

## 故障排除

### 常见问题及解决方案

#### 1. Walrus 集群无法启动

```bash
# 检查端口占用
lsof -i :9091
lsof -i :6001

# 清理残留数据
./scripts/start_walrus_cluster.sh stop
./scripts/start_walrus_cluster.sh clean
./scripts/start_walrus_cluster.sh start
```

#### 2. RPC Gateway 连接拒绝

```bash
# 检查服务状态
ps aux | grep rpc-gateway

# 检查端口监听
tl -tulnp | grep 8545

# 重新启动
killall rpc-gateway
cd rpc-gateway
cargo run --release
```

#### 3. Block Producer 无交易处理

```bash
# 检查 Walrus topic 是否存在
./distributed-walrus/target/debug/walrus-cli --addr 127.0.0.1:9091
> STATE blockchain-txs

# 手动创建 topic (如果不存在)
> REGISTER blockchain-txs

# 检查交易池状态
# 在 Block Producer 日志中查找 "交易池为空"
```

#### 4. 状态根计算失败

```bash
# 检查数据库权限
ls -la block-producer/data/

# 删除损坏的数据库
rm block-producer/data/block_producer_state_*.redb

# 重启 Block Producer
```

#### 5. Gas 不足错误

```bash
# 检查交易 Gas 设置
# 确保 gas >= 21000 (简单转账)
# 合约调用需要更多 gas

# 查看具体错误
# 在 Block Producer 日志中搜索 "GasUsedGreaterThanGasLimit"
```

### 日志级别调整

```bash
# 详细调试信息
RUST_LOG=debug cargo run --release

# 只看 block-producer 日志
RUST_LOG=block_producer=trace cargo run --release

# 只看错误信息
RUST_LOG=error cargo run --release
```

---

## 性能监控

### 1. 实时性能指标

```bash
# 查看系统资源使用
top -p $(pgrep -f "block-producer\|rpc-gateway\|distributed-walrus")

# 查看网络连接
ss -tulnp | grep -E "(8545|9091)"

# 查看磁盘 IO
iotop -p $(pgrep -f "block-producer")
```

### 2. TPS 测试脚本

创建 `tps_test.sh`:

```bash
#!/bin/bash

echo "开始 TPS 测试..."

START_TIME=$(date +%s)
TX_COUNT=100

for i in $(seq 1 $TX_COUNT); do
  curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{
      "jsonrpc": "2.0",
      "method": "eth_sendTransaction",
      "params": [{
        "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc",
        "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "value": "0x1",
        "gas": "0x5208",
        "nonce": "0x'$(printf '%x' $i)'"
      }],
      "id": '$i'
    }' >/dev/null &
done

wait

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
TPS=$(echo "scale=2; $TX_COUNT / $DURATION" | bc)

echo "发送 $TX_COUNT 笔交易，耗时 ${DURATION} 秒"
echo "平均 TPS: $TPS"
```

### 3. 区块确认时间监控

创建 `block_time_monitor.sh`:

```bash
#!/bin/bash

LAST_BLOCK=0
LAST_TIME=$(date +%s)
44
while true; do
  # 获取最新区块号 (需要实现相应的 RPC 方法)
  CURRENT_BLOCK=$(curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    | jq -r '.result' 2>/dev/null || echo "0")
  
  if [[ "$CURRENT_BLOCK" != "0x0" && "$CURRENT_BLOCK" != "$LAST_BLOCK" ]]; then
    CURRENT_TIME=$(date +%s)
    TIME_DIFF=$((CURRENT_TIME - LAST_TIME))
    
    echo "[$(date)] 区块 $CURRENT_BLOCK 生成，距离上一块 ${TIME_DIFF} 秒"
    
    LAST_BLOCK=$CURRENT_BLOCK
    LAST_TIME=$CURRENT_TIME
  fi
  
  sleep 1
done
```

---

## 🎯 完整测试流程总结

1. **启动基础设施**:
   ```bash
   ./scripts/start_walrus_cluster.sh start
   cd rpc-gateway && cargo run --release &
   cd ../block-producer && cargo run --release
   ```

2. **发送测试交易**:
   ```bash
   curl -X POST http://127.0.0.1:8545 -d '{"jsonrpc":"2.0","method":"eth_sendTransaction",...}'
   ```

3. **观察执行结果**:
   - 查看 Block Producer 日志
   - 确认区块生成和状态更新

4. **验证数据存储**:
   - 检查 Redb 数据库文件
   - 查询账户余额变化

5. **性能评估**:
   - 运行 TPS 测试
   - 监控资源使用情况

---

## 📚 相关文档

- [Block Producer README](./README.md) - 详细技术说明
- [RPC Gateway 文档](../rpc-gateway/README.md) - RPC 接口文档
- [Walrus 集群文档](../distributed-walrus/README.md) - 分布式存储说明
- [系统架构设计](../docs/blockchain-design.md) - 整体架构文档

---
