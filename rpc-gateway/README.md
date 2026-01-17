# RPC Gateway

一个 JSON-RPC 服务器，接收区块链钱包（如 MetaMask）的交易请求，并将交易数据写入 Walrus 分布式日志系统。

## 功能特性

- 🔌 标准 JSON-RPC 2.0 接口
- 💼 兼容以太坊钱包（MetaMask 等）
- 📝 将区块链交易持久化到 Walrus
- 🚀 异步高性能处理

---

## 快速开始

### 1. 启动 Walrus 集群

```bash
cd distributed-walrus
make cluster-up
make cluster-bootstrap
```

### 2. 启动 RPC Gateway

```bash
cd ../rpc-gateway

# 开发模式
cargo run

# 生产模式（推荐）
cargo build --release
./target/release/rpc-gateway --walrus-addr 127.0.0.1:9091
```

### 3. 测试

```bash
# 健康检查
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}'

# 或使用测试脚本
./test.sh
```

### 4. 查看写入的数据

```bash
cd ../distributed-walrus
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
# 在 CLI 中: GET blockchain-txs
```

---

## 架构

```
┌──────────────┐
│   MetaMask   │  用户钱包
└──────┬───────┘
       │ JSON-RPC (eth_sendTransaction)
       ▼
┌──────────────────────┐
│ rpc-gateway          │  JSON-RPC 服务器 (8545)
│ - 接收区块链交易      │
│ - 序列化为 JSON       │
│ - 转换为 hex          │
└──────┬───────────────┘
       │ Walrus Protocol (PUT)
       ▼
┌──────────────────────┐
│ Distributed Walrus   │  分布式日志集群
│ - Node 1 (Leader)    │  (9091-9093)
│ - Node 2 (Follower)  │
│ - Node 3 (Follower)  │
└──────────────────────┘
```

**数据流程：** Transaction (JSON) → Hex String → Walrus Topic

---

## 配置

### 命令行参数

```bash
cargo run -- \
  --walrus-addr 127.0.0.1:9091 \
  --rpc-port 8545 \
  --rpc-host 0.0.0.0 \
  --default-topic blockchain-txs
```

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--walrus-addr` | `127.0.0.1:9091` | Walrus 服务器地址 |
| `--rpc-port` | `8545` | JSON-RPC 监听端口 |
| `--rpc-host` | `127.0.0.1` | JSON-RPC 监听地址 |
| `--default-topic` | `blockchain-txs` | 默认写入的 topic |

### 环境变量

```bash
# 调整日志级别
RUST_LOG=debug cargo run

# 只看 rpc-gateway 日志
RUST_LOG=rpc_gateway=debug cargo run
```

---

## API 文档

### 支持的方法

| 方法 | 说明 | 状态 |
|------|------|------|
| `health` | 健康检查 | ✅ |
| `eth_sendTransaction` | 发送交易 | ✅ |
| `eth_sendRawTransaction` | 发送原始交易 | ✅ |

### 示例

#### 发送交易

```bash
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
      "gasPrice": "0x4a817c800",
      "nonce": "0x0"
    }],
    "id": 1
  }'
```

#### 发送原始交易

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0xf86c808504a817c800825208945aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed880de0b6b3a764000080"],
    "id": 1
  }'
```

### 配置 MetaMask

1. 打开 MetaMask → 设置 → 网络 → 添加网络
2. 填写配置：
   - **网络名称**: `Walrus Local`
   - **RPC URL**: `http://localhost:8545`
   - **链 ID**: `1337`
   - **货币符号**: `ETH`

---

## 开发

### 添加新的 RPC 方法

1. 在 `WalrusRpcApi` trait 中定义方法：
```rust
#[method(name = "eth_getBalance")]
async fn get_balance(&self, address: String) -> Result<String>;
```

2. 在 `WalrusRpcServer` 中实现逻辑

### 读取存储的交易

```bash
# 使用 walrus-cli
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
> GET blockchain-txs
```

---

## 生产部署

### 安全建议

⚠️ **生产环境必须配置：**
- ✅ 使用反向代理（Nginx/Caddy）添加 HTTPS
- ✅ 添加认证机制（API Key 或 JWT）
- ✅ 配置限流（防止 DDoS）
- ✅ 通过防火墙限制访问

### 监控

- **指标监控**: Prometheus + Grafana
- **日志聚合**: ELK Stack
- **告警**: 集成到现有告警系统

---

## 故障排查

### 连接 Walrus 失败

```bash
# 检查 Walrus 是否运行
curl -v 127.0.0.1:9091

# 查看日志
cd distributed-walrus
docker-compose logs -f
```

### RPC 端口被占用

```bash
# 使用其他端口
cargo run -- --rpc-port 8546

# 或查看占用
lsof -i :8545
```

### MetaMask 无法连接

检查清单：
1. ✅ RPC URL 正确: `http://localhost:8545`
2. ✅ 防火墙允许连接
3. ✅ 使用 `--rpc-host 0.0.0.0` 允许外部访问
4. ✅ 查看 Gateway 日志确认请求是否到达

---

## 许可证

同 Walrus 主项目
