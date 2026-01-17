# TX Generator (交易生成器)

一个以太坊交易生成工具，支持密钥生成、交易签名并发送到 RPC Gateway。

## 功能特性

- 🔑 生成以太坊密钥对（私钥/公钥/地址）
- ✍️ 使用私钥签名交易
- 📤 发送交易到 RPC Gateway
- 🎲 批量生成测试交易
- 🚀 异步高性能处理
- 🦀 使用 [Alloy](https://github.com/alloy-rs) - Paradigm 的专业以太坊库

---

## 快速开始

### 1. 生成密钥对

```bash
cd tx-generator

cargo run -- generate-key
```

**输出示例：**
```
🔑 新密钥对已生成:

地址:     0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
私钥:     0x4c0883a69102937d6231471b5dbb6204fe512961708279f8b9a9ba8f8a5c8c8f

⚠️  请妥善保管私钥，不要泄露！
```

### 2. 发送单笔交易

```bash
cargo run -- send-tx \
  --private-key 0x4c0883a69102937d6231471b5dbb6204fe512961708279f8b9a9ba8f8a5c8c8f \
  --to 0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed \
  --value 1.5 \
  --rpc-url http://localhost:8545
```

**参数说明：**
- `--private-key`: 发送方私钥（64 位十六进制）
- `--to`: 接收地址
- `--value`: 转账金额（ETH）
- `--rpc-url`: RPC Gateway 地址（默认 http://localhost:8545）

### 3. 批量生成测试交易

```bash
cargo run -- batch-generate \
  --count 100 \
  --interval-ms 100 \
  --rpc-url http://localhost:8545
```

**参数说明：**
- `--count`: 生成交易数量（默认 10）
- `--interval-ms`: 发送间隔毫秒数（默认 100ms）
- `--rpc-url`: RPC Gateway 地址

**输出示例：**
```
🚀 开始批量生成 100 笔测试交易
[1/100] ✅ 交易已发送: 0x1a2b3c4d5e6f (3.42 ETH)
[2/100] ✅ 交易已发送: 0x7f8a9b0c1d2e (7.89 ETH)
...
🎉 批量生成完成！
```

---

## 工作流程

```
1. 生成/加载私钥
   ↓
2. 创建交易对象
   (from, to, value, gas, nonce)
   ↓
3. 使用私钥签名交易
   (ECDSA 签名)
   ↓
4. 编码为 RLP 格式
   (原始交易)
   ↓
5. 通过 JSON-RPC 发送
   (eth_sendRawTransaction)
   ↓
6. RPC Gateway 接收
   ↓
7. 写入 Walrus 集群
```

---

## 命令详解

### `generate-key` - 生成密钥

生成新的以太坊密钥对。

```bash
cargo run -- generate-key
```

**安全提示：**
- 私钥是账户的唯一凭证
- 永远不要与他人分享私钥
- 建议使用硬件钱包存储大额资产

### `send-tx` - 发送交易

使用指定私钥发送一笔交易。

```bash
cargo run -- send-tx \
  --private-key <私钥> \
  --to <接收地址> \
  --value <金额> \
  --rpc-url <RPC地址>
```

**交易参数：**
- Gas: 21000（标准转账）
- Gas Price: 20 Gwei
- Nonce: 随机生成（测试用）

### `batch-generate` - 批量生成

批量生成测试交易，用于压力测试。

```bash
cargo run -- batch-generate \
  --count 1000 \
  --interval-ms 50 \
  --rpc-url http://localhost:8545
```

**特点：**
- 每笔交易使用随机密钥对
- 随机接收地址
- 随机金额（0.1 - 10 ETH）
- 可配置发送速率

---

## 集成测试

### 完整流程测试

```bash
# 终端 1: 启动 Walrus 集群
cd distributed-walrus
# 参考 docs/start-walrus-cluster.md

# 终端 2: 启动 RPC Gateway
cd rpc-gateway
cargo run

# 终端 3: 启动 Block Producer
cd block-producer
cargo run

# 终端 4: 生成测试交易
cd tx-generator
cargo run -- batch-generate --count 50
```

### 验证交易

```bash
# 使用 walrus-cli 查看
cd distributed-walrus
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
> GET blockchain-txs
```

---

## 技术细节

### 密钥生成

使用 [Alloy](https://github.com/alloy-rs) 的 `PrivateKeySigner` 生成随机密钥：

```rust
use alloy::signers::{local::PrivateKeySigner, Signer};

let signer = PrivateKeySigner::random();
println!("地址: {:?}", signer.address());
println!("私钥: {}", hex::encode(signer.to_bytes()));
```

### 交易签名

```rust
use alloy::consensus::{SignableTransaction, TxLegacy};
use alloy::primitives::{Address, Bytes, U256};

// 1. 创建交易
let tx = TxLegacy {
    chain_id: Some(1337),
    nonce: 0,
    gas_price: 20_000_000_000,
    gas_limit: 21000,
    to: to_address.into(),
    value,
    input: Bytes::new(),
};

// 2. 签名
let signature = signer.sign_transaction(&tx).await?;
let signed_tx = tx.into_signed(signature);

// 3. 编码为 2718 格式
let encoded = signed_tx.encoded_2718();
```

### RPC 调用

```rust
// JSON-RPC 2.0 请求
{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0xf86c808504a817c800825208945aAeb..."],
    "id": 1
}
```

---

## 环境变量

```bash
# 调整日志级别
RUST_LOG=debug cargo run -- batch-generate --count 10

# 只看 tx-generator 日志
RUST_LOG=tx_generator=debug cargo run -- send-tx ...
```

---

## 性能测试

### 吞吐量测试

```bash
# 每秒 100 笔交易 (10ms 间隔)
cargo run --release -- batch-generate \
  --count 10000 \
  --interval-ms 10

# 每秒 1000 笔交易 (1ms 间隔)
cargo run --release -- batch-generate \
  --count 10000 \
  --interval-ms 1
```

### 预期性能

| 间隔 | TPS | 用途 |
|------|-----|------|
| 1ms | ~1000 | 压力测试 |
| 10ms | ~100 | 中等负载 |
| 100ms | ~10 | 轻量测试 |
| 1000ms | ~1 | 单元测试 |

---

## 故障排查

### 连接 RPC Gateway 失败

```bash
# 检查 RPC Gateway 是否运行
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}'
```

### 私钥格式错误

确保私钥格式正确：
- ✅ 64 位十六进制字符串
- ✅ 可选 `0x` 前缀
- ❌ 不要包含空格或其他字符

**正确格式：**
```
0x4c0883a69102937d6231471b5dbb6204fe512961708279f8b9a9ba8f8a5c8c8f
4c0883a69102937d6231471b5dbb6204fe512961708279f8b9a9ba8f8a5c8c8f
```

### 交易发送失败

检查清单：
1. ✅ RPC Gateway 正常运行
2. ✅ Walrus 集群正常运行
3. ✅ 网络连接正常
4. ✅ 私钥格式正确

---

## 安全提示

⚠️ **重要：**
- 本工具仅用于测试和开发
- 不要在主网使用测试私钥
- 不要在代码中硬编码私钥
- 生产环境使用硬件钱包或 KMS

---

## 下一步

- [ ] 支持从文件加载私钥
- [ ] 支持助记词（BIP39）
- [ ] 支持 EIP-1559 交易（新 Gas 机制）
- [ ] 支持智能合约调用
- [ ] 支持批量签名（离线）
- [ ] 添加交易状态查询
- [ ] 支持多链（Polygon、BSC 等）

---

## 许可证

同 Walrus 主项目
