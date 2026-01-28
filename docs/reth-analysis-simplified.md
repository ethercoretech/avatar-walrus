# Reth 区块构建流程精简版

## 📋 三大阶段

```
阶段 1: 准备阶段 (Pre-Execution)
阶段 2: 交易执行 (Transaction Execution)  
阶段 3: 区块组装 (Block Assembly)
```

---

## 🎬 完整流程

### **阶段 1：准备阶段**

```
共识层请求 → engine_forkchoiceUpdatedV3(payloadAttributes)
                ↓
步骤 1.1: 验证并生成 payloadId
         └─ 立即返回给共识层 ✅

步骤 1.2: 初始化状态 (异步后台)
         ├─ 获取父区块状态
         └─ 创建 REVM State

步骤 1.3: 创建 BlockBuilder
         └─ 设置 EVM 环境 (block_env, cfg_env)

步骤 1.4: 应用 Pre-Execution 系统调用
         ├─ EIP-4788: Beacon Block Root 写入合约
         └─ EIP-2935: Block Hash History 存储
```

### **阶段 2：交易执行**

```
步骤 2.1: 从交易池获取交易
         └─ pool.best_transactions_with_attributes()
            (按 effective_tip 排序，过滤不合格交易)

步骤 2.2: 循环执行交易
         for tx in best_txs:
           ├─ 2.2.1: 预检查 (gas_limit, blob_gas)
           ├─ 2.2.2: execute_transaction_without_commit()
           │         └─ REVM 执行 → ResultAndState
           ├─ 2.2.3: 处理结果
           │         ├─ Success: 提交状态
           │         ├─ Revert: 消耗 gas 但不改变状态
           │         └─ Halt: 跳过，标记发送者无效
           ├─ 2.2.4: commit_transaction() → 生成 Receipt
           └─ 2.2.5: 累积 gas_used 和 fees

步骤 2.3: 处理 Withdrawals (如有)
         └─ 直接增加账户余额 (不通过交易)
```

### **阶段 3：区块组装**

```
步骤 3.1: 完成构建
         └─ builder.finish() → 获取 bundle_state

步骤 3.2: 计算 POST 字段
         ├─ state_root: BundleState → HashedPostState → Trie 根
         ├─ transactions_root: 交易列表的 Merkle 根
         ├─ receipts_root: Receipts 的 Merkle 根
         ├─ logs_bloom: 聚合所有 logs 的 Bloom filter
         ├─ gas_used: 累积值
         ├─ blob_gas_used: 累积值
         ├─ withdrawals_root: Withdrawals 的 Merkle 根
         └─ requests_hash: 系统请求的哈希

步骤 3.3: 组装完整区块头
         └─ 合并 PRE 字段 + POST 字段

步骤 3.4: 计算 block_hash
         └─ keccak256(rlp_encode(header))

步骤 3.5: 创建 SealedBlock 并缓存 Payload
```

---

## 📊 关键字段分类

### **PRE-EXECUTION 字段** (步骤 1.3 设置)

| 字段 | 来源 |
|------|------|
| `parent_hash` | 父区块 |
| `number` | parent.number + 1 |
| `timestamp` | PayloadAttributes |
| `beneficiary` | PayloadAttributes.suggestedFeeRecipient |
| `gas_limit` | 配置/父区块 |
| `base_fee_per_gas` | 基于父区块计算 (EIP-1559) |
| `prevrandao` | PayloadAttributes |
| `parent_beacon_block_root` | PayloadAttributes |
| `excess_blob_gas` | 基于父区块计算 (EIP-4844) |

### **POST-EXECUTION 字段** (步骤 3.2 计算)

| 字段 | 何时计算 |
|------|----------|
| `gas_used` | 交易执行中累积 |
| `blob_gas_used` | 交易执行中累积 |
| `state_root` | 所有交易+Withdrawals后计算 |
| `transactions_root` | 交易列表确定后计算 |
| `receipts_root` | 所有 receipts 生成后计算 |
| `logs_bloom` | 聚合所有 receipts 的 bloom |
| `withdrawals_root` | 基于 withdrawals 列表计算 |
| `requests_hash` | 基于系统请求计算 |
| `block_hash` | 完整 header 组装后计算 |

---

## 🔑 核心技术要点

### **1. BundleState → HashedPostState**

```
BundleState (Plain) → HashedPostState (Keccak256) → State Root
├─ 并行处理: into_par_iter() (Rayon)
├─ 增量计算: 只处理修改的账户/存储
└─ 懒惰哈希: 按需计算 keccak256
```

### **2. State Root 增量优化**

```
优化策略:
├─ 从 ChangeSets 加载 PrefixSets (只包含修改的账户前缀)
├─ 只重算这些前缀路径上的 Trie 节点
├─ 缓存中间节点到 TrieUpdates
└─ 并行计算每个账户的 storage_root

性能: O(M) vs O(N), M = 修改账户数 << N = 总账户数
```

### **3. 交易池过滤链**

```
best_transactions_with_attributes():
├─ Layer 1: base_fee 过滤
├─ Layer 2: nonce 连续性检查
├─ Layer 3: 账户余额验证
├─ Layer 4: blob_fee 检查 (EIP-4844)
└─ Layer 5: 按 effective_tip 排序
```

### **4. 关键公式**

#### Base Fee (EIP-1559)
```
gas_target = gas_limit / 2
delta = |gas_used - gas_target|

if gas_used > gas_target:
    base_fee_new = base_fee_old * (1 + delta/gas_target/8)
else:
    base_fee_new = base_fee_old * (1 - delta/gas_target/8)
```

#### Blob Gas Price (EIP-4844)
```
excess_blob_gas = parent.excess_blob_gas + 
                  parent.blob_gas_used - TARGET_BLOB_GAS
blob_base_fee = fake_exponential(1, excess_blob_gas, 3338477)
```

### **5. 交易执行三种结果**

| 结果 | 状态变更 | Gas 消耗 | 计入区块 | Receipt |
|------|----------|----------|----------|---------|
| **Success** | ✅ 应用 | ✅ 扣除 | ✅ 是 | status=1 |
| **Revert** | ❌ 回滚 | ✅ 扣除 | ✅ 是 | status=0 |
| **Halt** | ❌ 不应用 | ❌ 不扣除 | ❌ 否 | 无 receipt |

**关键区别**: Revert 的交易虽然失败，但仍消耗 gas 并占用区块空间！

### **6. 并发安全 (Payload 构建)**

```
Phase 1 (同步, <1s):
├─ 验证 PayloadAttributes
├─ 生成 payloadId
└─ 立即返回给共识层 ✅

Phase 2 (异步后台):
├─ tokio::spawn() 异步构建
├─ 执行所有交易
├─ 计算 POST 字段
└─ 存入 payload_store (RwLock 保护)

getPayload(payloadId):
└─ 从 payload_store 读取 (可能返回 null 如果未完成)
```

---

## 🚨 常见陷阱

1. **Withdrawals 时序**
   - ⚠️ 必须在所有用户交易执行后应用
   - ⚠️ 必须在计算 state_root 前应用
   - ✅ 不消耗 gas，直接修改余额

2. **Logs Bloom 聚合**
   - 每个 receipt 有自己的 bloom
   - 区块头的 bloom = 所有 receipt bloom 的 OR 运算

3. **Blob Sidecar 分离**
   - 区块中只存储: blob_versioned_hashes, max_fee_per_blob_gas
   - Sidecar 单独传播: blobs (128KB 数据), commitments, proofs
   - 只保留约 18 天 (共识层负责)

4. **Revert vs Halt 区别**
   - Revert: 交易有效，消耗 gas，计入区块
   - Halt: 交易无效，不计入区块，后续交易被标记为无效

---

## 📌 Reth vs Geth 核心差异

| 维度 | Reth | Geth |
|------|------|------|
| **State 管理** | BundleState (内存高效) | JournalDB (复杂日志) |
| **Trie 计算** | 增量更新 + PrefixSets | 可能全量重算 |
| **存储引擎** | MDBX + Static Files | LevelDB / Pebble |
| **并行** | Rayon 并行 storage_root | 单线程执行 |
| **语言** | Rust (零成本抽象) | Go (GC overhead) |
| **交易池** | 简化 Vec + 索引 | 多层索引 (pending/queued) |

---

## 🎯 核心要点总结

1. **Pre-Execution 系统调用** 在用户交易之前执行 (EIP-4788, EIP-2935)
2. **交易循环** 逐个执行，累积状态和 gas (Success/Revert 都计入区块)
3. **Withdrawals** 在所有交易后应用，直接增加余额
4. **POST 字段** 在所有状态变更完成后计算 (state_root, receipts_root, logs_bloom)
5. **异步构建** forkchoiceUpdated 立即返回，后台异步构建 payload
6. **增量优化** 只重算修改的账户和 Trie 节点，大幅提升性能

---

**关键数据流**: 
```
PayloadAttributes → BlockBuilder → REVM → BundleState → HashedPostState → 
State Root → Header → SealedBlock → ExecutionPayload
```

这就是 Reth 构建区块的精简流程！🚀
