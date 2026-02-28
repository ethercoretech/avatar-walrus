# 使用 MetaMask 测试指南

## 问题说明

**为什么之前 MetaMask 报错无法发现链 ID？**

因为 RPC Gateway 缺少 `eth_chainId` RPC 方法的实现。MetaMask 连接到新网络时会自动调用以下方法：
- `eth_chainId` - 获取链 ID（必需）
- `eth_blockNumber` - 获取最新区块号（推荐）

## 已实现的 RPC 方法

现在 RPC Gateway 已经实现了以下方法：

### 1. eth_chainId
返回配置的链 ID（十六进制格式）

**请求示例：**
```bash
curl -X POST "http://127.0.0.1:8545" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
```

**响应示例：**
```json
{"jsonrpc":"2.0","result":"0x7a69","id":1}
```

**说明：**
- 链 ID：`31337`（十进制） = `0x7a69`（十六进制）
- 这是 Hardhat/Foundry 等开发工具常用的本地测试链 ID

### 2. eth_blockNumber
返回最新区块号（十六进制格式）

**请求示例：**
```bash
curl -X POST "http://127.0.0.1:8545" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

**响应示例：**
```json
{"jsonrpc":"2.0","result":"0x0","id":1}
```

**说明：**
- 初始状态返回 `0x0`（尚未生成区块）
- 随着交易执行，区块号会递增

### 3. eth_getTransactionCount
获取账户的交易计数（nonce）

**请求示例：**
```bash
curl -X POST "http://127.0.0.1:8545" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "method":"eth_getTransactionCount",
    "params":["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "latest"],
    "id":1
  }'
```

**响应示例：**
```json
{"jsonrpc":"2.0","result":"0x0","id":1}
```

### 4. health
健康检查方法

**请求示例：**
```bash
curl -X POST "http://127.0.0.1:8545" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}'
```

**响应示例：**
```json
{"jsonrpc":"2.0","result":"OK","id":1}
```

## 配置 MetaMask

### 步骤 1: 启动系统

```bash
cd /opt/rust/project/avatar-walrus/block-producer

# 方式一：使用一键启动脚本
./scripts/start_full_system.sh start

# 方式二：手动启动
# 终端 1: Walrus 集群
cd /opt/rust/project/avatar-walrus
./scripts/start_walrus_cluster.sh start

# 终端 2: RPC Gateway
cd /opt/rust/project/avatar-walrus/rpc-gateway
cargo run --release -- --walrus-addr 127.0.0.1:9091 --rpc-port 8545

# 终端 3: Block Producer
cd /opt/rust/project/avatar-walrus/block-producer
cargo run --release -- --walrus-addr 127.0.0.1:9091 --topic blockchain-txs
```

### 步骤 2: 验证 RPC 服务

```bash
cd /opt/rust/project/avatar-walrus/rpc-gateway
./test_rpc_methods.sh
```

**预期输出：**
```
======================================
  测试 RPC Gateway 方法
======================================

RPC URL: http://127.0.0.1:8545

✅ RPC Gateway 连接正常

=== 测试 health ===
响应：{"jsonrpc":"2.0","result":"OK","id":1}
✅ 健康检查通过

=== 测试 eth_chainId ===
响应：{"jsonrpc":"2.0","result":"0x7a69","id":1}
✅ 链 ID: 0x7a69
   十进制：31337

=== 测试 eth_blockNumber ===
响应：{"jsonrpc":"2.0","result":"0x0","id":1}
✅ 区块号：0x0
   十进制：0

=== 测试 eth_getTransactionCount ===
响应：{"jsonrpc":"2.0","result":"0x0","id":1}
✅ 账户 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 的 nonce: 0x0
   十进制：0

======================================
  测试完成
======================================
```

### 步骤 3: 在 MetaMask 中添加网络

1. **打开 MetaMask**
   - 点击网络选择器（顶部）
   - 选择 "添加网络"
   - 选择 "手动添加网络"

2. **填写网络配置**

   | 字段 | 值 |
   |------|-----|
   | **网络名称** | `Avatar Walrus Testnet` |
   | **RPC URL** | `http://127.0.0.1:8545` |
   | **链 ID** | `31337` |
   | **货币符号** | `ETH` |
   | **区块浏览器 URL** | 留空 |

3. **保存配置**
   - 点击 "保存"
   - MetaMask 应该能自动识别链 ID，不再报错

### 步骤 4: 导入测试账户

系统预置了以下测试账户（每个账户有 10,000 ETH）：

**账户 1（默认发送账户）:**
- 地址：`0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`
- 私钥：`0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`

**账户 2（接收账户）:**
- 地址：`0x70997970C51812dc3A010C7d01b50e0d17dc79C8`
- 私钥：`0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d`

**导入方法：**
1. 点击 MetaMask 账户图标
2. 选择 "导入账户"
3. 选择 "私钥"
4. 输入私钥，点击 "导入"

## 发送测试交易

### 方法 1: 使用 MetaMask 发送 ETH 转账

1. **确保切换到 `Avatar Walrus Testnet` 网络**

2. **点击 "发送" 按钮**

3. **填写交易信息：**
   - **收款人**: `0x70997970C51812dc3A010C7d01b50e0d17dc79C8`
   - **金额**: `0.001 ETH`
   - **Gas 限制**: `21000`
   - **Gas 价格**: `20 Gwei`

4. **确认并发送**
   - 点击 "下一步"
   - 确认交易信息
   - 点击 "确认"

5. **观察区块生成**
   ```bash
   # 在新终端监控日志
   tail -f /opt/rust/project/avatar-walrus/.logs/block-producer.log
   ```

### 方法 2: 使用脚本发送交易

```bash
cd /opt/rust/project/avatar-walrus/block-producer

# 发送 5 笔测试交易
./scripts/send_test_transaction.sh 5
```

### 方法 3: 部署智能合约

1. **编译合约**
   ```bash
   cd /opt/rust/project/avatar-walrus/block-producer/scripts/contracts
   node compile.js
   ```

2. **使用 Remix 部署**
   - 打开 [Remix IDE](https://remix.ethereum.org)
   - 创建新的 Solidity 文件
   - 编译合约
   - 在 "Deploy & Run Transactions" 面板：
     - **ENVIRONMENT**: 选择 `Injected Provider - MetaMask`
     - **ACCOUNT**: 选择导入的账户
     - 点击 "Deploy"
   - 在 MetaMask 中确认交易

## 监控和验证

### 1. 查看区块生成

```bash
# 监控区块生成日志
tail -f /opt/rust/project/avatar-walrus/.logs/block-producer.log
```

### 2. 查看 RPC Gateway 日志

```bash
tail -f /opt/rust/project/avatar-walrus/.logs/rpc-gateway.log
```

### 3. 验证数据库状态

```bash
cd /opt/rust/project/avatar-walrus/block-producer
./scripts/verify_database.sh
```

### 4. 监控区块生成

```bash
./scripts/monitor_blocks.sh
```

## 常见问题排查

### 问题 1: MetaMask 仍然报错 "无法连接到 RPC"

**可能原因：**
- RPC Gateway 未启动
- 端口被占用
- 防火墙阻止

**解决方案：**
```bash
# 检查 RPC Gateway 是否运行
curl -X POST "http://127.0.0.1:8545" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}'

# 如果没有响应，重启 RPC Gateway
cd /opt/rust/project/avatar-walrus/rpc-gateway
cargo run --release
```

### 问题 2: 链 ID 不匹配

**症状：** MetaMask 显示 "链 ID 不匹配" 错误

**解决方案：**
1. 在 MetaMask 中删除该网络
2. 重新添加网络，确保链 ID 填写 `31337`
3. 清除浏览器缓存

### 问题 3: 交易一直 Pending

**可能原因：**
- Block Producer 未运行
- Walrus 集群异常
- Gas 价格过低

**解决方案：**
```bash
# 检查所有组件状态
cd /opt/rust/project/avatar-walrus/block-producer
./scripts/start_full_system.sh status

# 查看 Walrus 集群状态
cd /opt/rust/project/avatar-walrus
./scripts/start_walrus_cluster.sh status
```

### 问题 4: Nonce 错误

**症状：** MetaMask 提示 "Nonce too low" 或 "Nonce too high"

**解决方案：**
1. 在 MetaMask 中重置账户：
   - 设置 → 高级 → 清除活动标签页数据
2. 手动设置 Nonce：
   - 发送交易时展开 "编辑" 选项
   - 手动调整 Nonce 值

## 性能监控

### 访问 Prometheus Metrics

```bash
# 打开浏览器访问
http://127.0.0.1:8546/metrics
```

**监控指标包括：**
- 交易总数
- 交易处理时长
- Walrus 写入时长
- 批量处理大小

## 停止系统

```bash
cd /opt/rust/project/avatar-walrus/block-producer
./scripts/start_full_system.sh stop
```

## 清理数据（重新开始）

```bash
# 停止系统
cd /opt/rust/project/avatar-walrus/block-producer
./scripts/start_full_system.sh stop

# 清理数据
rm -rf /opt/rust/project/avatar-walrus/block-producer/data/*

# 重新启动
./scripts/start_full_system.sh start
```

## 总结

现在 RPC Gateway 已经完整实现了 MetaMask 所需的 RPC 方法：

✅ `eth_chainId` - 返回链 ID `31337` (0x7a69)
✅ `eth_blockNumber` - 返回最新区块号
✅ `eth_getTransactionCount` - 返回账户 nonce
✅ `eth_sendTransaction` - 发送交易
✅ `eth_sendRawTransaction` - 发送原始交易
✅ `health` - 健康检查

你可以使用 MetaMask 像使用真实区块链一样与本地测试链交互了！🎉
