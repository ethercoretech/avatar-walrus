# Reth-REVM 集成代码示例

> 本文档提供实际的代码片段，展示 Reth 如何在不同场景下使用 REVM

---

## 📝 场景 1: 最小化示例 - 执行单笔交易

```rust
use reth_revm::{database::StateProviderDatabase, db::State};
use revm::context::TxEnv;

fn execute_single_transaction(
    state_provider: impl StateProvider,
    tx: TransactionSigned,
    block_env: BlockEnv,
) -> Result<ExecutionResult> {
    // 1️⃣ 创建数据库适配器
    let db = StateProviderDatabase::new(state_provider);
    
    // 2️⃣ 创建 REVM State
    let mut state = State::builder()
        .with_database(db)
        .with_bundle_update()  // 追踪状态变更
        .build();
    
    // 3️⃣ 创建 EVM 实例（通过 alloy_evm）
    let mut evm = evm_factory.create_evm(&mut state, EvmEnv {
        cfg_env: CfgEnv {
            chain_id: 1,
            spec_id: SpecId::CANCUN,
            ..Default::default()
        },
        block_env,
    });
    
    // 4️⃣ 准备交易环境
    let tx_env = TxEnv {
        caller: tx.recover_signer()?,
        gas_limit: tx.gas_limit(),
        gas_price: tx.max_fee_per_gas(),
        transact_to: TxKind::Call(tx.to()),
        value: tx.value(),
        data: tx.input().clone(),
        nonce: Some(tx.nonce()),
        ..Default::default()
    };
    
    // 5️⃣ 执行交易 ⭐
    let ResultAndState { result, state } = evm.transact(tx_env)?;
    //                                         ↑
    //                                         └─ REVM 核心执行！
    
    // 6️⃣ 处理状态变更（如果需要）
    if result.is_success() {
        state.commit(state);  // 提交到 State.bundle_state
    }
    
    // 7️⃣ 提取状态变更
    let bundle = state.take_bundle();
    
    Ok(result)
}
```

---

## 📝 场景 2: 区块构建完整流程

```rust
// crates/ethereum/payload/src/lib.rs（简化版）

fn build_payload(
    pool: &TransactionPool,
    state_provider: &StateProvider,
    attributes: PayloadAttributes,
    parent: &SealedHeader,
    chain_spec: &ChainSpec,
    evm_config: &EthEvmConfig,
) -> Result<EthBuiltPayload> {
    // ==========================================
    // 阶段 1: 准备 State
    // ==========================================
    let state_db = StateProviderDatabase::new(state_provider);
    let mut db = State::builder()
        .with_database(state_db)
        .with_bundle_update()
        .build();
    
    // ==========================================
    // 阶段 2: 创建 BlockBuilder
    // ==========================================
    let env_attrs = NextBlockEnvAttributes {
        timestamp: attributes.timestamp,
        suggested_fee_recipient: attributes.suggested_fee_recipient,
        prev_randao: attributes.prev_randao,
        gas_limit: 30_000_000,
        withdrawals: attributes.withdrawals.clone(),
        parent_beacon_block_root: attributes.parent_beacon_block_root,
    };
    
    let mut builder = evm_config.builder_for_next_block(
        &mut db,
        parent,
        env_attrs,
    )?;
    // 内部创建了:
    // - BlockExecutor (来自 alloy_evm)
    // - Evm 实例（包装 REVM）
    
    // ==========================================
    // 阶段 3: Pre-Execution 系统调用
    // ==========================================
    builder.apply_pre_execution_changes()?;
    // 内部调用:
    // - evm.transact_system_call(BEACON_ROOTS_ADDRESS, ...)
    // - evm.transact_system_call(HISTORY_STORAGE_ADDRESS, ...)
    
    // ==========================================
    // 阶段 4: 执行用户交易
    // ==========================================
    let block_gas_limit = builder.evm().block().gas_limit();
    let base_fee = builder.evm().block().basefee();
    
    let mut best_txs = pool.best_transactions_with_attributes(
        BestTransactionsAttributes::new(base_fee, None)
    );
    
    let mut cumulative_gas_used = 0u64;
    let mut total_fees = U256::ZERO;
    
    while let Some(pool_tx) = best_txs.next() {
        // 4.1 检查 gas limit
        if cumulative_gas_used + pool_tx.gas_limit() > block_gas_limit {
            best_txs.mark_invalid(&pool_tx, ...);
            continue;
        }
        
        // 4.2 执行交易
        let tx = pool_tx.to_consensus();
        let gas_used = match builder.execute_transaction(tx.clone()) {
            //                        ↑
            //                        └─ 内部调用 evm.transact()
            Ok(gas_used) => gas_used,
            Err(BlockExecutionError::Validation(InvalidTx { error, .. })) => {
                // 交易无效，标记并跳过
                best_txs.mark_invalid(&pool_tx, ...);
                continue;
            }
            Err(err) => return Err(err),
        };
        
        // 4.3 累积值
        cumulative_gas_used += gas_used;
        let miner_fee = tx.effective_tip_per_gas(base_fee).unwrap();
        total_fees += U256::from(miner_fee) * U256::from(gas_used);
    }
    
    // ==========================================
    // 阶段 5: 完成构建
    // ==========================================
    let BlockBuilderOutcome {
        execution_result,  // receipts, gas_used, requests
        block,             // RecoveredBlock
        hashed_state,      // 已哈希的状态
        trie_updates,      // Trie 更新
    } = builder.finish(state_provider)?;
    
    // builder.finish() 内部:
    // 1. 从 State 提取 bundle_state
    // 2. 转换为 HashedPostState
    // 3. 计算 state_root
    // 4. 组装 block header
    
    // ==========================================
    // 阶段 6: 封装 Payload
    // ==========================================
    let sealed_block = Arc::new(block.sealed_block().clone());
    let payload = EthBuiltPayload::new(
        attributes.id,
        sealed_block,
        total_fees,
        execution_result.requests,
    );
    
    Ok(payload)
}
```

---

## 📝 场景 3: Execution Stage 批量执行

```rust
// crates/stages/stages/src/stages/execution.rs（简化版）

fn execute_stage(
    provider: &Provider,
    evm_config: &EthEvmConfig,
    start_block: u64,
    end_block: u64,
) -> Result<()> {
    // ==========================================
    // 1. 创建批量执行器
    // ==========================================
    let state_provider = LatestStateProviderRef::new(provider);
    let db = StateProviderDatabase(state_provider);
    let mut executor = evm_config.batch_executor(db);
    //                            ↑
    //                            └─ 创建 BasicBlockExecutor
    //                               └─ 内部持有 State<StateProviderDatabase>
    
    let mut cumulative_gas = 0;
    let mut executor_lifetime = Instant::now();
    
    // ==========================================
    // 2. 批量执行循环
    // ==========================================
    for block_number in start_block..=end_block {
        // 2.1 获取区块（已恢复签名）
        let block = provider
            .recovered_block(block_number, TransactionVariant::NoHash)?
            .ok_or(ProviderError::HeaderNotFound(block_number))?;
        
        // 2.2 执行区块 ⭐
        let result = executor.execute_one(&block)?;
        //                     ↑
        //                     └─ 内部流程:
        //                        1. 创建 BlockExecutor
        //                        2. 调用 execute_block(transactions)
        //                        3. 循环: for tx in transactions {
        //                              evm.transact(tx_env)  ← REVM 执行
        //                           }
        //                        4. 累积状态到 State.bundle_state
        
        // 2.3 验证执行结果
        consensus.validate_block_post_execution(&block, &result, None)?;
        
        cumulative_gas += result.gas_used;
        
        // 2.4 检查是否需要 commit
        if executor.size_hint() > 1_000_000 ||
           executor_lifetime.elapsed() > Duration::from_secs(120) 
        {
            // ==========================================
            // 3. Commit 并重置
            // ==========================================
            
            // 3.1 Finalize executor
            let outcome = executor.finalize()?;
            //                     ↑
            //                     └─ 内部调用:
            //                        let bundle = self.db.take_bundle();
            //                        return ExecutionOutcome { bundle, ... }
            
            // 3.2 写入数据库
            provider.write_execution_outcome(outcome)?;
            //       ↑
            //       └─ 写入 MDBX:
            //          - PlainAccountState (账户)
            //          - PlainStorageState (存储)
            //          - Bytecodes (合约代码)
            //          - AccountChangeSets
            //          - StorageChangeSets
            
            // 3.3 重新创建 executor
            let new_state_provider = LatestStateProviderRef::new(provider);
            let new_db = StateProviderDatabase(new_state_provider);
            executor = evm_config.batch_executor(new_db);
            
            cumulative_gas = 0;
            executor_lifetime = Instant::now();
        }
    }
    
    // ==========================================
    // 4. 最终 Commit
    // ==========================================
    let outcome = executor.finalize()?;
    provider.write_execution_outcome(outcome)?;
    
    Ok(())
}
```

---

## 📝 场景 4: RPC eth_call 调用

```rust
// crates/rpc/rpc-eth-api/src/helpers/call.rs（简化版）

async fn eth_call(
    request: CallRequest,
    block_id: BlockId,
    state_override: Option<StateOverride>,
) -> Result<Bytes> {
    // ==========================================
    // 1. 获取状态
    // ==========================================
    let state_provider = self.state_at_block_id(block_id)?;
    let state_db = StateProviderDatabase::new(state_provider);
    
    // ==========================================
    // 2. 创建 State（只读，不需要 bundle_update）
    // ==========================================
    let mut db = State::builder()
        .with_database(state_db)
        .build();  // 注意: 不追踪状态变更
    
    // 2.1 应用 state override（如果有）
    if let Some(overrides) = state_override {
        for (address, account) in overrides {
            if let Some(balance) = account.balance {
                db.insert_account_info(address, AccountInfo {
                    balance,
                    ..Default::default()
                });
            }
            // ... 其他 override
        }
    }
    
    // ==========================================
    // 3. 准备 EVM 环境
    // ==========================================
    let block = self.block_by_id(block_id)?;
    let evm_env = EvmEnv {
        cfg_env: CfgEnv {
            chain_id: 1,
            spec_id: revm_spec(chain_spec, block.header()),
            ..Default::default()
        },
        block_env: BlockEnv {
            number: U256::from(block.number()),
            timestamp: U256::from(block.timestamp()),
            gas_limit: block.gas_limit(),
            basefee: block.base_fee_per_gas().unwrap_or_default(),
            beneficiary: block.beneficiary(),
            ..Default::default()
        },
    };
    
    // ==========================================
    // 4. 创建 EVM 并执行
    // ==========================================
    let mut evm = evm_config.create_evm(&mut db, evm_env);
    
    // 4.1 准备交易环境
    let tx_env = TxEnv {
        caller: request.from.unwrap_or_default(),
        gas_limit: request.gas.map(|g| g as u64).unwrap_or(30_000_000),
        gas_price: request.gas_price.unwrap_or_default(),
        transact_to: request.to.map(TxKind::Call).unwrap_or(TxKind::Create),
        value: request.value.unwrap_or_default(),
        data: request.data.unwrap_or_default(),
        nonce: None,  // eth_call 不检查 nonce
        ..Default::default()
    };
    
    // 4.2 执行 ⭐
    let ResultAndState { result, .. } = evm.transact(tx_env)?;
    //                                      ↑
    //                                      └─ REVM 执行
    
    // ==========================================
    // 5. 返回结果
    // ==========================================
    match result {
        ExecutionResult::Success { output, .. } => {
            Ok(output.into_data())  // 返回 return data
        }
        ExecutionResult::Revert { output, .. } => {
            Err(RpcError::Revert(output))
        }
        ExecutionResult::Halt { reason, .. } => {
            Err(RpcError::Halt(reason))
        }
    }
}
```

---

## 📝 场景 5: newPayload 验证收到的区块

```rust
// crates/engine/tree/src/tree/payload_validator.rs（简化版）

async fn validate_new_payload(
    payload: ExecutionPayloadV3,
    versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256,
) -> Result<PayloadStatus> {
    // ==========================================
    // 1. 转换 Payload 为 Block
    // ==========================================
    let block = convert_payload_to_sealed_block(payload)?;
    let recovered_block = block.try_recover()?;  // 恢复签名
    
    // ==========================================
    // 2. Pre-Execution 验证
    // ==========================================
    consensus.validate_header(&block.header)?;
    consensus.validate_header_against_parent(&block.header, &parent)?;
    consensus.validate_block_pre_execution(&block)?;
    
    // 检查无效祖先
    if let Some(invalid) = self.find_invalid_ancestor(&block) {
        return Ok(PayloadStatus::Invalid { ... });
    }
    
    // ==========================================
    // 3. 执行区块
    // ==========================================
    let parent_state = state_by_block_hash(block.parent_hash())?;
    let state_db = StateProviderDatabase::new(parent_state);
    let mut db = State::builder()
        .with_database(state_db)
        .with_bundle_update()
        .build();
    
    // 3.1 创建执行器
    let mut executor = evm_config
        .executor_for_block(&mut db, &recovered_block)?;
    
    // 3.2 应用 Pre-Execution 变更
    executor.apply_pre_execution_changes()?;
    
    // 3.3 执行所有交易 ⭐
    let result = executor.execute_block(
        recovered_block.transactions_recovered()
    )?;
    // 内部循环:
    // for tx in transactions {
    //     let tx_result = evm.transact(tx_env)?;  ← REVM
    //     receipts.push(build_receipt(tx_result));
    // }
    
    // ==========================================
    // 4. Post-Execution 验证
    // ==========================================
    consensus.validate_block_post_execution(
        &recovered_block,
        &result,
        None,
    )?;
    
    // 验证内容:
    // - header.gas_used == sum(receipts.gas_used)
    // - header.receipts_root == calculate_receipt_root(receipts)
    // - header.logs_bloom == aggregate_logs_bloom(receipts)
    // - header.state_root == calculate_state_root(bundle_state)
    
    // ==========================================
    // 5. 返回验证结果
    // ==========================================
    Ok(PayloadStatus::Valid {
        latest_valid_hash: block.hash(),
    })
}
```

---

## 📝 场景 6: 自定义 Inspector 实现

```rust
// 示例: 追踪所有 SSTORE 操作

use revm::{Inspector, context_interface::result::ExecutionResult};
use alloy_primitives::{Address, U256};

#[derive(Debug, Default)]
struct StorageTracer {
    storage_writes: Vec<StorageWrite>,
}

#[derive(Debug, Clone)]
struct StorageWrite {
    address: Address,
    slot: U256,
    value: U256,
}

impl<Context> Inspector<Context> for StorageTracer {
    fn step(&mut self, interp: &mut Interpreter, context: &mut Context) {
        // 检查是否是 SSTORE opcode
        if interp.current_opcode() == opcode::SSTORE {
            let slot = interp.stack().peek(0).unwrap();
            let value = interp.stack().peek(1).unwrap();
            
            self.storage_writes.push(StorageWrite {
                address: interp.contract().address,
                slot: slot.into(),
                value: value.into(),
            });
        }
    }
}

// 使用:
fn trace_storage_writes(block: &Block) -> Result<Vec<StorageWrite>> {
    let state_db = StateProviderDatabase::new(state_provider);
    let mut db = State::builder()
        .with_database(state_db)
        .build();
    
    // 创建带 Inspector 的 EVM
    let inspector = StorageTracer::default();
    let mut evm = evm_factory.create_evm_with_inspector(
        &mut db,
        evm_env,
        inspector,
    );
    
    // 执行所有交易
    for tx in block.transactions() {
        let tx_env = prepare_tx_env(tx);
        let _ = evm.transact(tx_env)?;  // REVM 会回调 Inspector
    }
    
    // 提取追踪结果
    let inspector = evm.into_inspector();
    Ok(inspector.storage_writes)
}
```

---

## 📝 场景 7: Gas Estimation 二分查找

```rust
// crates/rpc/rpc-eth-api/src/helpers/estimate.rs（简化版）

async fn estimate_gas(
    call: CallRequest,
    block_id: BlockId,
) -> Result<U256> {
    // ==========================================
    // 1. 准备环境
    // ==========================================
    let state = StateProviderDatabase::new(state_provider);
    let mut db = State::builder()
        .with_database(state)
        .build();
    
    let mut evm = evm_config.create_evm(&mut db, evm_env);
    
    // ==========================================
    // 2. 二分查找最优 gas limit
    // ==========================================
    let mut lo = 21_000u64;  // 最小 gas
    let mut hi = 30_000_000u64;  // 最大 gas
    
    while lo < hi {
        let mid = (lo + hi) / 2;
        
        // 2.1 准备交易环境
        let tx_env = TxEnv {
            gas_limit: mid,  // 尝试这个 gas limit
            caller: call.from.unwrap_or_default(),
            transact_to: call.to.map(TxKind::Call).unwrap_or(TxKind::Create),
            value: call.value.unwrap_or_default(),
            data: call.data.clone().unwrap_or_default(),
            ..Default::default()
        };
        
        // 2.2 执行并检查结果 ⭐
        let res = evm.transact(tx_env)?;
        //            ↑
        //            └─ REVM 执行
        
        match res.result {
            ExecutionResult::Success { .. } => {
                // 成功了，尝试更小的 gas
                hi = mid;
            }
            ExecutionResult::Revert { .. } => {
                // Revert 了，需要更多 gas
                lo = mid + 1;
            }
            ExecutionResult::Halt { reason: HaltReason::OutOfGas, .. } => {
                // Gas 不够，需要更多
                lo = mid + 1;
            }
            ExecutionResult::Halt { reason, .. } => {
                // 其他错误，不是 gas 问题
                return Err(RpcError::Halt(reason));
            }
        }
    }
    
    // ==========================================
    // 3. 返回估算的 gas
    // ==========================================
    Ok(U256::from(lo))
}
```

---

## 📝 场景 8: 系统调用（EIP-4788, EIP-2935）

```rust
// Pre-Execution 系统调用示例

fn apply_pre_execution_changes(
    evm: &mut impl Evm,
    block_number: u64,
    timestamp: u64,
    parent_beacon_block_root: Option<B256>,
    parent_hash: B256,
) -> Result<()> {
    // ==========================================
    // 1. EIP-4788: Beacon Root Contract Call
    // ==========================================
    if let Some(root) = parent_beacon_block_root {
        // 准备系统调用环境
        let caller = SYSTEM_ADDRESS;  // 0x000...00000
        let to = BEACON_ROOTS_ADDRESS;  // 0x000...04788
        let input = root.as_slice();
        
        // 执行系统调用 ⭐
        let result = evm.transact_system_call(caller, to, input.into())?;
        //               ↑
        //               └─ REVM 执行系统调用
        //                  - 不检查 nonce
        //                  - 不扣除 gas
        //                  - 不增加 nonce
        
        // 系统调用的效果:
        // 在 BEACON_ROOTS_ADDRESS 合约中:
        // storage[timestamp % 8191] = root
        // storage[timestamp % 8191 + 8191] = timestamp
        
        // 状态变更被追踪到 State.bundle_state
    }
    
    // ==========================================
    // 2. EIP-2935: Block Hash History
    // ==========================================
    if block_number > 1 {
        let caller = SYSTEM_ADDRESS;
        let to = HISTORY_STORAGE_ADDRESS;  // EIP-2935 地址
        let input = parent_hash.as_slice();
        
        // 执行系统调用 ⭐
        let result = evm.transact_system_call(caller, to, input.into())?;
        //               ↑
        //               └─ REVM 执行系统调用
        
        // 系统调用的效果:
        // 在 HISTORY_STORAGE_ADDRESS 合约中:
        // storage[parent_block_number % 8192] = parent_hash
    }
    
    Ok(())
}
```

---

## 📝 场景 9: 状态变更的提取和持久化

```rust
// 展示 BundleState 如何从 REVM 流向 Reth 数据库

fn finalize_and_persist(
    executor: BasicBlockExecutor<...>,
    provider: &Provider,
) -> Result<()> {
    // ==========================================
    // 1. Finalize 执行器
    // ==========================================
    let mut state = executor.into_state();
    //                        ↑
    //                        └─ 消费 executor，获取 State
    
    // ==========================================
    // 2. 提取 BundleState
    // ==========================================
    let bundle = state.take_bundle();
    //                 ↑
    //                 └─ 从 REVM State 中 move 出 BundleState
    
    // bundle 内容示例:
    // BundleState {
    //     state: {
    //         // 账户 1: EOA 发送交易
    //         0x1111...: BundleAccount {
    //             info: Some(Account {
    //                 balance: 99.9 ETH,  // 减少了（支付 gas）
    //                 nonce: 6,           // 增加了
    //                 code_hash: KECCAK_EMPTY,
    //             }),
    //             storage: {},  // 无存储变更
    //             status: Changed,
    //         },
    //         
    //         // 账户 2: 智能合约被调用
    //         0x2222...: BundleAccount {
    //             info: None,  // 账户信息未变
    //             storage: {
    //                 U256::from(0): U256::from(42),   // slot 0 改为 42
    //                 U256::from(5): U256::from(100),  // slot 5 改为 100
    //             },
    //             status: Changed,
    //         },
    //         
    //         // 账户 3: 新部署的合约
    //         0x3333...: BundleAccount {
    //             info: Some(Account {
    //                 balance: 0,
    //                 nonce: 1,
    //                 code_hash: 0xabcd...,  // 新合约的 code hash
    //             }),
    //             storage: {
    //                 U256::from(0): U256::from(1),  // 初始化存储
    //             },
    //             status: Created,  // ← 新创建
    //         },
    //     },
    //     
    //     contracts: {
    //         0xabcd...: Bytecode::new_raw(vec![0x60, 0x80, ...]),  // 合约字节码
    //     },
    //     
    //     reverts: [
    //         // 每个区块一个 HashMap，用于 unwind
    //     ],
    // }
    
    // ==========================================
    // 3. 转换为 HashedPostState
    // ==========================================
    let hashed_state = HashedPostState::from_bundle_state::<KeccakKeyHasher>(
        bundle.state()  // 并行哈希化所有地址
    );
    
    // hashed_state:
    // HashedPostState {
    //     accounts: {
    //         keccak256(0x1111...): Some(Account { ... }),
    //         keccak256(0x2222...): None,  // 只有存储变更
    //         keccak256(0x3333...): Some(Account { ... }),
    //     },
    //     storages: {
    //         keccak256(0x2222...): HashedStorage {
    //             storage: {
    //                 keccak256(slot_0): U256::from(42),
    //                 keccak256(slot_5): U256::from(100),
    //             },
    //         },
    //         keccak256(0x3333...): HashedStorage { ... },
    //     },
    // }
    
    // ==========================================
    // 4. 计算 State Root
    // ==========================================
    let state_root = calculate_state_root_with_updates(hashed_state)?;
    
    // ==========================================
    // 5. 持久化到数据库
    // ==========================================
    let outcome = ExecutionOutcome {
        bundle,       // ← 来自 REVM
        receipts,     // ← Reth 构建
        requests,     // ← Reth 收集
        first_block: block.number(),
    };
    
    provider.write_execution_outcome(outcome)?;
    // 内部写入:
    // - PlainAccountState table
    // - PlainStorageState table
    // - Bytecodes table
    // - AccountChangeSets table
    // - StorageChangeSets table
    // - Receipts (静态文件)
    
    Ok(())
}
```

---

## 📝 场景 10: 自定义链的 EVM 配置（Optimism 示例）

```rust
// crates/optimism/evm/src/lib.rs

/// Optimism 的 EVM 配置
pub struct OpEvmConfig<ChainSpec, N, R, EvmFactory> {
    inner: EthEvmConfig<ChainSpec, EvmFactory>,  // 复用以太坊配置
    phantom: PhantomData<(N, R)>,
}

impl ConfigureEvm for OpEvmConfig {
    type BlockExecutorFactory = OpExecutorFactory<...>;
    
    fn executor<DB>(&self, db: DB) -> impl Executor<DB> {
        // 使用 Optimism 特定的执行器
        OpExecutor::new(self, db)
    }
}

// Optimism 的特殊处理
impl OpExecutor {
    fn apply_pre_execution_changes(&mut self) -> Result<()> {
        // 1. 标准以太坊 Pre-Execution
        self.inner.apply_pre_execution_changes()?;
        
        // 2. Optimism 特定: L1 属性存款
        if let Some(l1_block_info) = self.l1_block_info {
            // 执行 L1Block 合约调用
            let result = self.evm.transact_system_call(
                SYSTEM_ADDRESS,
                L1_BLOCK_ADDRESS,
                l1_block_info.encode(),
            )?;
            //  ↑
            //  └─ 仍然使用 REVM，但参数不同
        }
        
        Ok(())
    }
    
    fn execute_transaction(&mut self, tx: OpTransaction) -> Result<u64> {
        // Optimism 的 gas 计算不同
        let l1_cost = calculate_l1_cost(tx)?;
        
        // 扣除 L1 cost
        let sender_account = self.evm.db_mut().basic(tx.signer())?;
        sender_account.balance -= l1_cost;
        
        // 标准执行
        let result = self.inner.execute_transaction(tx)?;
        //                          ↑
        //                          └─ 内部仍然调用 evm.transact()
        
        Ok(result)
    }
}

// 关键: Optimism 使用 op-revm（REVM 的分支）
// 但接口完全兼容，Reth 的封装层不需要改变
```

---

## 🎯 快速参考卡片

### REVM 核心接口

```rust
// 1. 执行方法
evm.transact(tx_env)              // 执行单笔交易
evm.transact_commit(tx_env)       // 执行并提交
evm.transact_system_call(...)     // 系统调用

// 2. 数据库接口（Reth 实现）
db.basic(address)                 // 读取账户
db.storage(address, slot)         // 读取存储
db.code_by_hash(hash)             // 读取字节码
db.block_hash(number)             // 读取区块哈希

// 3. 状态管理
State::builder()...build()        // 创建 State
state.take_bundle()                // 提取状态变更
state.merge_transitions(...)       // 合并状态
state.commit(changes)              // 提交变更
```

### Reth 封装接口

```rust
// 1. 配置层
evm_config.executor(db)                    // 创建执行器
evm_config.batch_executor(db)              // 创建批量执行器
evm_config.builder_for_next_block(...)     // 创建区块构建器

// 2. 执行层
executor.execute_one(block)                // 执行单个区块
executor.execute_batch(blocks)             // 批量执行
builder.execute_transaction(tx)            // 执行单笔交易
builder.apply_pre_execution_changes()      // Pre-execution

// 3. 状态层
StateProviderDatabase::new(provider)       // 创建数据库适配器
HashedPostState::from_bundle_state(...)    // 转换状态
provider.write_execution_outcome(...)      // 持久化
```

---

## 📖 阅读建议

### 源代码阅读顺序

1. **入门级**: 
   - `crates/revm/src/database.rs` - 理解 Database trait
   - `crates/ethereum/payload/src/lib.rs:216-353` - 看交易执行循环

2. **进阶级**:
   - `crates/evm/evm/src/execute.rs:528-595` - BasicBlockExecutor
   - `crates/stages/stages/src/stages/execution.rs` - 批量执行

3. **高级**:
   - `crates/engine/tree/src/tree/payload_validator.rs` - 复杂的验证流程
   - `crates/rpc/rpc-eth-api/src/helpers/` - RPC 实现

### 关键文件清单

```
核心接口:
├─ crates/revm/src/database.rs              (Database trait 实现)
├─ crates/revm/src/lib.rs                   (REVM re-exports)
└─ crates/evm/evm/src/execute.rs            (Executor trait)

配置层:
├─ crates/evm/evm/src/lib.rs                (ConfigureEvm)
└─ crates/ethereum/evm/src/lib.rs           (EthEvmConfig)

使用场景:
├─ crates/ethereum/payload/src/lib.rs       (Payload building)
├─ crates/stages/stages/src/stages/execution.rs  (Sync)
├─ crates/rpc/rpc-eth-api/src/helpers/call.rs    (RPC)
└─ crates/engine/tree/src/tree/payload_validator.rs  (Validation)
```

---

**总结**: 通过这些实际代码示例，可以看到 Reth 和 REVM 的集成是**层次清晰、职责明确、高度可复用**的。无论是区块构建、批量同步还是 RPC 调用，都遵循相同的模式：**准备 State → 创建 Executor → 调用 REVM → 提取结果**。这种一致性使得代码易于理解和维护！🚀