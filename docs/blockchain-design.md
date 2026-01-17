# Walrus 区块链架构

极简的区块链架构，基于 Walrus 分布式日志系统构建。

---

## 整体架构

```
                         用户钱包
                        (MetaMask)
                            │
                            │ eth_sendTransaction
                            ▼
                   ┌─────────────────┐
                   │  rpc-gateway    │  端口 8545
                   │  JSON-RPC 服务   │
                   └────────┬────────┘
                            │
                            │ PUT 交易
                            ▼
              ┌──────────────────────────┐
              │   Walrus Cluster         │  端口 9091-9093
              │   分布式日志系统          │
              │  - Node 1 (Leader)       │
              │  - Node 2 (Follower)     │
              │  - Node 3 (Follower)     │
              └──────────┬───────────────┘
                         │
                         │ GET 交易（每 5 秒）
                         ▼
                ┌──────────────────┐
                │ block-producer   │
                │ 区块生产者        │
                └────────┬─────────┘
                         │
                         │ 提交区块
                         ▼
                ┌──────────────────┐
                │ Execution Layer  │  (待实现)
                │ EVM 执行引擎      │
                │ (使用 revm)       │
                └────────┬─────────┘
                         │
                         │ 状态更新
                         ▼
                ┌──────────────────┐
                │ State Database   │  (待实现)
                │ 状态数据库        │
                │ (使用 redb)       │
                └──────────────────┘
```

---

## 核心组件

### 1. RPC Gateway (端口 8545)

**角色：** 入口层 / 交易接收器

**功能：**
- 接收来自 MetaMask 等钱包的 JSON-RPC 请求
- 将交易序列化为 JSON
- 转换为十六进制格式
- 写入 Walrus 集群

**类比：** 就像一个邮局，接收用户的信件（交易）并存档

### 2. Walrus Cluster (端口 9091-9093)

**角色：** 存储层 / 消息队列

**功能：**
- 持久化存储所有交易
- 基于 Raft 的分布式共识
- 3 节点集群（1 Leader + 2 Followers）
- 保证数据不丢失

**类比：** 就像一个分布式的交易缓冲池，先存起来再处理

### 3. Block Producer

**角色：** 共识层 / 区块生产者

**功能：**
- 每 5 秒从 Walrus 读取交易
- 打包交易成区块
- 计算区块哈希和交易根
- 提交给执行层

**类比：** 就像工厂流水线，定时把原料（交易）打包成产品（区块）

### 4. Execution Layer (待实现)

**角色：** 执行层

**功能：**
- 使用 revm 执行 EVM 字节码
- 更新账户状态（余额、nonce）
- 生成交易收据和事件
- 计算 Gas 消耗

**类比：** 就像 CPU，真正执行交易指令

### 5. State Database (待实现)

**角色：** 状态层

**功能：**
- 使用 redb 存储账户状态
- 键值对数据库（地址 → 账户信息）
- 支持快速查询和更新
- 持久化所有状态变更

**类比：** 就像硬盘，存储所有账户的当前状态

---

## 数据流

### 发送交易流程

```
1. 用户在 MetaMask 发起转账
   ↓
2. RPC Gateway 接收 eth_sendTransaction
   ↓
3. 序列化为 JSON: {"from":"0x...","to":"0x...","value":"1 ETH",...}
   ↓
4. 转换为十六进制: 0x7b2266726f6d223a...
   ↓
5. 写入 Walrus: PUT blockchain-txs 0x7b2266726f6d223a...
   ↓
6. Walrus 返回: OK
   ↓
7. RPC Gateway 返回交易哈希给用户
```

### 区块生产流程

```
1. Block Producer 定时器触发（每 5 秒）
   ↓
2. 从 Walrus 读取交易: GET blockchain-txs
   ↓
3. 解析十六进制 → JSON → Transaction 结构体
   ↓
4. 收集最多 100 笔交易
   ↓
5. 构建区块:
   - 区块号: 0, 1, 2, ...
   - 父区块哈希: 0x...
   - 时间戳: 2026-01-17 11:00:00
   - 交易数: 3
   - 交易根: 0x... (所有交易的哈希)
   ↓
6. 计算区块哈希: 0x1a2b3c4d...
   ↓
7. 提交给执行层: execute_block(block)
   ↓
8. 执行层返回状态根: 0xabcd...
```

---

## 为什么要这样设计？

### 分层架构的优势

1. **解耦合**
   - RPC Gateway 崩溃不影响 Walrus
   - Block Producer 重启不丢交易
   - 每个组件独立升级

2. **可扩展**
   - 可以部署多个 RPC Gateway 实例
   - Walrus 集群可以水平扩展
   - Block Producer 可以优化出块速度

3. **容错性**
   - Walrus 基于 Raft，容忍 1 节点故障
   - 交易先持久化再处理
   - 即使 Block Producer 宕机，交易也不丢

### 使用 Walrus 作为消息队列的好处

传统区块链的交易池（Mempool）存在的问题：
- ❌ 内存中存储，重启后丢失
- ❌ 难以在多节点间同步
- ❌ 无法追溯历史交易

使用 Walrus 的优势：
- ✅ 持久化存储，永不丢失
- ✅ 自带分布式共识（Raft）
- ✅ 可以回放所有历史交易
- ✅ 支持多个消费者（Block Producer）

---

## 端口映射

| 组件 | 端口 | 协议 | 说明 |
|------|------|------|------|
| RPC Gateway | 8545 | JSON-RPC | 兼容以太坊钱包 |
| Walrus Node 1 | 9091 | Walrus Protocol | 客户端连接 |
| Walrus Node 2 | 9092 | Walrus Protocol | 客户端连接 |
| Walrus Node 3 | 9093 | Walrus Protocol | 客户端连接 |
| Walrus Node 1 | 6001 | QUIC | Raft 内部通信 |
| Walrus Node 2 | 6002 | QUIC | Raft 内部通信 |
| Walrus Node 3 | 6003 | QUIC | Raft 内部通信 |

---

## 对比以太坊

| 层级 | 以太坊 | Walrus 区块链 |
|------|--------|--------------|
| **入口** | Geth RPC | rpc-gateway |
| **交易池** | Mempool (内存) | Walrus Cluster (持久化) |
| **共识** | PoS (Beacon Chain) | Block Producer (定时出块) |
| **执行** | Geth EVM | revm (待实现) |
| **状态** | LevelDB | redb (待实现) |

**核心区别：** 交易先持久化到 Walrus，再批量处理，而不是直接广播到 Mempool。

---

## 快速体验

### 1. 启动所有组件

```bash
# 终端 1-3: 启动 Walrus 集群
cd distributed-walrus
# 参考 docs/start-walrus-cluster.md

# 终端 4: 启动 RPC Gateway
cd rpc-gateway
cargo run

# 终端 5: 启动 Block Producer
cd block-producer
cargo run
```

### 2. 发送交易

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendTransaction",
    "params": [{
      "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
      "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
      "value": "0xde0b6b3a7640000"
    }],
    "id": 1
  }'
```

### 3. 观察出块

等待 5 秒，Block Producer 会打印：

```
✅ 区块 #0 已生成
   区块哈希: 0x1a2b3c4d...
   交易数量: 1
   父区块: 0x0000000000...
```

---

## 下一步

- [ ] 实现 EVM 执行引擎（使用 revm）
- [ ] 实现状态数据库（使用 redb）
- [ ] 添加账户余额查询 API
- [ ] 支持智能合约部署和调用
- [ ] 添加交易收据和事件日志
- [ ] 实现 Gas 计量和限制
- [ ] 优化区块生产策略（动态出块）
- [ ] 添加区块浏览器

---

## 相关文档

- [启动 Walrus 集群](start-walrus-cluster.md)
- [RPC Gateway 文档](../rpc-gateway/README.md)
- [Block Producer 文档](../block-producer/README.md)
