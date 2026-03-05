## 单节点区块链功能测试报告（avatar-walrus）

> 范围：`block-producer` + `rpc-gateway` + `distributed-walrus` 单节点链路  
> 目标：评估当前实现对「区块头 / 区块体 / 合约执行 / Slot 存储 / 四颗 Trie / Bloom 过滤器 / 数据库 Schema / 基础 RPC」的支持程度。

---

### 1. 总体结论

- **区块头 / 区块体**：结构完整，字段设计与以太坊高度对齐，支持后续扩展。
- **合约执行 & 状态更新**：基于 REVM，单节点下执行闭环完整（部署、调用、状态持久化、收据生成）。
- **Slot 存储 & 四颗 Trie**：账户/存储/区块级 Trie 均已实现并参与根哈希计算，状态 Trie 采用增量计算，存在全量遍历 TODO。
- **Bloom 过滤器**：仅有字段和占位实现，未真实计算 Bloom 位图。
- **数据库 Schema（Redb）**：账户 / 存储 / 代码 / 区块 / 区块哈希五类表设计清晰，并附带事务缓冲与变更追踪。
- **基础 RPC 接口**：已覆盖交易提交与部分链信息查询，但尚未达到「通用 EVM 节点」常见 RPC 完整度。

---

### 2. 区块头与区块体

- **实现位置**：`block-producer/src/schema/block.rs`
- **单元测试**：`block-producer/src/schema/block.rs` 内 `mod tests`：
  - `test_transaction_parsing`
  - `test_contract_creation`
  - `block_hash_changes_when_header_changes`
  - `block_hash_is_stable_for_same_header`
  - `block_tx_count_matches_transactions_len`
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test schema::block::tests`
- **核心结构**：
  - `BlockHeader`
    - `number`：区块号
    - `parent_hash`：父区块哈希
    - `timestamp`：Unix 秒时间戳（`chrono::serde::ts_seconds`）
    - `tx_count`：交易数
    - `transactions_root`：交易 Merkle Patricia Trie 根
    - `state_root`：状态 Trie 根（执行后填充）
    - `gas_used` / `gas_limit`：Gas 使用和限制
    - `receipts_root`：收据 Trie 根（执行后填充）
  - `Block`
    - `header: BlockHeader`
    - `transactions: Vec<Transaction>`
- **区块哈希**：
  - 通过 `serde_json` 序列化 `BlockHeader`，然后 `SHA256` 计算得到 `0x...` 格式哈希。
  - 在执行完成后，会根据最新的 roots / gas 等信息更新头部，再参与后续使用。

**结论**：区块头/体结构足以支持单节点链完整表达必要元数据，兼容以太坊风格字段命名，便于未来扩展共识相关字段。

---

### 3. 交易与智能合约执行

- **交易 Schema**：`block-producer/src/schema/block.rs::Transaction`
  - 字段：`from/to/value/data/gas/nonce/hash`，并扩展 `gas_price/chain_id/max_fee_per_gas/max_priority_fee_per_gas` 等，可向 EIP-1559 方向演进。
  - 提供解析助手：`from_address/to_address/value_wei/gas_limit/nonce_value/data_bytes/is_create`。
- **执行链路**：
  1. **RPC 入口**（`rpc-gateway/src/main.rs`）  
     - 提供 `eth_sendTransaction` / `eth_sendRawTransaction`，将交易编码后写入 Walrus 指定 topic。
  2. **交易收集与出块**（`block-producer/src/main.rs`）  
     - `BlockProducer` 周期性从 Walrus 拉取交易，按 `block_interval` 和 `max_txs_per_block` 组装 `Block`。
  3. **区块执行**（`block-producer/src/executor/block_executor.rs`）  
     - `BlockExecutor::execute_block` 遍历区块内交易：
       - 预验证（gas、nonce、余额等），验证失败的交易被跳过且不影响其他交易。
       - 调用 `TransactionExecutor::execute`，内部基于 REVM 执行。
       - 累加 `total_gas_used`，记录成功/失败计数。
       - 使用 `ReceiptBuilder` 构建收据（包含日志）。
     - 整个区块在一个事务（`begin_transaction` → `commit` / `rollback`）中执行。
  4. **REVM 适配与状态写入**（`block-producer/src/executor/revm_adapter.rs`）  
     - `CachedRedbState` 实现 REVM `Database` trait：
       - `basic`：从 Redb 读取账户信息并缓存。
       - `code_by_hash`：按 `code_hash` 查找并返回合约字节码。
       - `storage`：按 `(address, index)` 读取存储值。
       - `block_hash`：从 `BLOCK_HASHES_TABLE` 获取区块哈希。
     - 执行完成后，将 `BundleState` 中的变更写入 `RedbStateDB`：
       - 账户：`nonce/balance/code_hash` 落盘。
       - 代码：当 `code` 与 `code_hash` 有效时写入 `CODE_TABLE`。
       - 存储槽：对每个 `slot` 调用 `set_storage` 持久化。
     - 成功与失败的执行结果以 `ExecutionResult` 结构返回，供收据构建与统计使用。

**结论**：  
单节点场景下，合约执行从交易 → REVM → 状态/存储/代码 → 收据 的执行和持久化链路是完整的。缺口主要体现在「读 RPC」（`eth_call`、`eth_estimateGas` 等）未暴露，而非执行能力不足。

- **单元测试**：`block-producer/src/executor/block_executor.rs` 与 `block_producer/src/executor/revm_adapter.rs`：
  - `executor::block_executor::tests::*`（包含 `test_block_execution_*` 系列）
  - `executor::revm_adapter::tests::*`（验证 REVM 适配和简单转账）
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test executor::block_executor::tests`
  - `cargo test executor::revm_adapter::tests`

---

### 4. Slot 存储与四颗 Trie

#### 4.1 Slot 存储模型

- **结构定义**：`block-producer/src/schema/storage.rs`
  - `StorageSlot { address: Address, key: U256, value: U256 }`
  - `StorageChange { address, key, old_value, new_value }`，附带 `is_changed` 与 `gas_refund`（SSTORE 退款规则）。
- **数据库映射**：`RedbStateDB` 中的 `STORAGE_TABLE`：
  - `(address (20 bytes), key (32 bytes)) -> value (32 bytes)`
  - 提供 `get_storage/set_storage/get_all_storage` 等接口。
- **执行后写入**：  
  - REVM 返回的每个账户 `account.storage` 中的变更被遍历，所有变更槽位写入 `STORAGE_TABLE`。

- **单元测试**：
  - `block-producer/src/schema/storage.rs::tests`（`test_storage_slot`、`test_storage_change`、`hashed_key_is_stable_and_32_bytes`）
  - `block-producer/src/db/redb_db.rs::tests::test_storage_crud`（含 `get_all_storage` 覆盖）
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test schema::storage::tests`
  - `cargo test db::redb_db::tests::test_storage_crud`

#### 4.2 Storage Trie（存储树）

- **实现位置**：`block-producer/src/trie/storage_root.rs`
- **核心逻辑**：
  - 对给定合约地址调用 `db.get_all_storage(address)` 获取所有槽位。
  - 过滤零值槽位（gas 优化，不进入 Trie）。
  - 对每个 `(key, value)`：
    - 将 `key` 转为 32 字节大端，再经 `hash_key` 计算哈希，作为 Trie 键。
    - `value` 通过 `rlp_encode_storage_value` 编码，作为 Trie 值。
  - 对哈希键排序后构建 Trie，输出根哈希。
  - 无存储槽时返回 `EMPTY_STORAGE_ROOT`（以太坊空 Trie 根常量）。

#### 4.3 State Trie（全局状态树）

- **实现位置**：`block-producer/src/trie/state_root.rs`
- **核心逻辑**：
  - 通过 `StateDatabase::get_changed_accounts()` 获取变更过的账户集合（由 Redb 追踪），然后：
    - 对每个账户：
      - 读取 `Account`（nonce、balance、code_hash 等）。
      - 调用 `StorageRootCalculator::calculate(address)` 获取该账户的存储根。
      - 使用 `hash_key(address.as_slice())` 作为 Trie 键。
      - 使用 `rlp_encode_account(nonce, balance, storage_root, code_hash)` 作为值，插入 Trie。
    - 按哈希后的地址排序后构建 Trie，得到全局状态根。
  - 提供 `calculate_incremental()` 用于增量计算；`calculate()` 当前委托给增量版本，并存在全量遍历 TODO。

#### 4.4 交易 Trie 与收据 Trie

- **Merkle 工具**：`block-producer/src/utils/merkle.rs`
  - 通用函数 `calculate_merkle_root<T: Encodable>(items: &[T]) -> B256`：
    - 对每个元素进行 RLP 编码。
    - 使用索引的 RLP 编码再 keccak256 作为 Trie 键。
    - 对哈希键排序后构建 MPT，输出根哈希。
  - 常量 `EMPTY_ROOT_HASH` 与以太坊空 Trie 根一致。
- **使用位置**：`block-producer/src/main.rs::submit_to_execution_layer`：
  - 交易根：`calculate_merkle_root(&schema_block.transactions)`
  - 收据根：对 `execution_result.receipts.values()` 计算；无收据时使用 `EMPTY_ROOT_HASH`。
  - 最终将 `transactions_root`、`receipts_root`、`state_root` 更新回区块头，并持久化区块。

**结论**：  
状态 Trie、存储 Trie、交易 Trie、收据 Trie 四者均有明确实现与落盘路径。状态 Trie 目前以「增量变更」为主，对大规模全量重建的支持仍有 TODO；收据/交易 Trie 虽然计算正确，但尚未通过 RPC 暴露验证接口。

- **单元测试**：
  - 交易 / 收据 Trie：`block-producer/src/utils/merkle.rs::tests::*`
  - Storage Trie：`block-producer/src/trie/storage_root.rs::tests::*`
  - State Trie：`block-producer/src/trie/state_root.rs::tests::*`
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test utils::merkle::tests`
  - `cargo test trie::storage_root::tests`
  - `cargo test trie::state_root::tests`

---

### 5. Bloom 过滤器

- **收据结构**：`schema::TransactionReceipt` 中的 `logs_bloom: Bytes` 字段已预留。
- **当前实现**：`executor/receipts.rs::compute_logs_bloom`：
  - 计算标准 2048-bit Bloom（对地址与 topic 做 `keccak256`，取 3 个 11-bit 索引置位）。
- **缺失能力**：
  - 暂未有基于 Bloom 的日志过滤 RPC（如 `eth_getLogs`、filter 相关接口）。

**结论**：  
Bloom 过滤器内部实现已兼容以太坊 2048-bit 规范；要达到生产级日志索引能力，还需配套 `eth_getLogs` 等 RPC 与索引策略。

- **单元测试**：`block-producer/src/executor/receipts.rs::tests`：
  - `bloom_empty_logs_is_all_zero`
  - `bloom_is_deterministic_for_same_logs`
  - `bloom_changes_when_log_changes`
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test executor::receipts::tests`

---

### 6. 数据库 Schema（Redb）

- **实现位置**：`block-producer/src/db/redb_db.rs` + `db/traits.rs`
- **表设计**：
  - `ACCOUNTS_TABLE`: `address (20 bytes) -> bincode(Account)`
  - `STORAGE_TABLE`: `(address (20), key (32)) -> value (32)`
  - `CODE_TABLE`: `code_hash (32) -> bytecode`
  - `BLOCKS_TABLE`: `block_number -> bincode(Block)`
  - `BLOCK_HASHES_TABLE`: `block_number -> hash (32)`
- **关键特性**：
  - **事务缓冲**：`TransactionBuffer` 支持块级原子提交（`begin_transaction/commit/rollback`），执行错误可回滚。
  - **变更追踪**：`changed_accounts` 记录发生变更的账户，减少状态根计算范围。
  - **只读模式**：`RedbStateDB::open_readonly` 允许 RPC 网关等组件以只读方式访问状态库。
  - **内置测试账户初始化**：启动时会根据内置 Wallet 列表创建带初始余额的账户，便于开发与本地联调。

**结论**：  
Redb Schema 覆盖了 EVM 状态的核心维度（账户、存储、代码、区块与索引），并通过事务与变更追踪增强了可靠性与性能，是适合单节点实验链的设计。

- **单元测试**：`block-producer/src/db/redb_db.rs::tests`：
  - `test_account_crud`
  - `test_storage_crud`
  - `test_transaction` / `test_transaction_rollback`
  - `test_changed_accounts_tracking`
  - `test_code_crud`
  - `test_block_and_block_hash_crud`
- **运行命令**（在 `block-producer` 目录）：
  - `cargo test db::redb_db::tests`

---

### 7. 基础 RPC 接口覆盖情况

- **实现位置**：`rpc-gateway/src/main.rs`
- **已实现接口**：
  - `eth_sendTransaction`：提交交易结构体到 Walrus。
  - `eth_sendRawTransaction`：提交原始编码交易。
  - `health`：健康检查。
  - `eth_getTransactionCount`：返回账户 nonce（从状态库读取）。
  - `eth_chainId`：返回链 ID。
  - `eth_blockNumber`：返回当前最新区块号。
- **尚未实现但常见的基础接口**：
  - 区块相关：`eth_getBlockByNumber` / `eth_getBlockByHash`
  - 收据：`eth_getTransactionReceipt`
  - 状态：`eth_getBalance` / `eth_getCode` / `eth_getStorageAt`
  - 调用类：`eth_call` / `eth_estimateGas`
  - 日志过滤：`eth_getLogs` 及 filter 系列。

**结论**：  
当前 RPC 网关更偏向「写多读少」，对钱包和简单工具（只需要发送交易 + 查看高度）已足够，但尚不符合通用 EVM 节点的最小 RPC 集合。

- **单元测试**：`rpc-gateway/src/main.rs::tests`：
  - `test_get_transaction_count_reads_nonce_from_state_db`
  - `test_chain_id_returns_configured_value`
  - `test_block_number_uses_highest_persisted_block`
- **运行命令**：
  - 在 `rpc-gateway` 目录：`cargo test`

---

### 8. 建议与演进路线

1. **补全 Bloom 过滤器实现**
   - 基于 EVM 日志（address + topics + data）构建标准 2048-bit Bloom。
   - 为收据与区块增加 Bloom 聚合，并为将来的 `eth_getLogs` 提供支撑。

2. **扩展 RPC 接口**
   - 第一批：`eth_getBlockByNumber/Hash`、`eth_getTransactionReceipt`、`eth_getBalance`、`eth_getCode`、`eth_getStorageAt`。
   - 第二批：`eth_call` / `eth_estimateGas`（只读 EVM 执行路径）。
   - 第三批：`eth_getLogs` + 过滤器接口，结合 Bloom 过滤器落地日志查询。

3. **完善状态根计算**
   - 在 `StateRootCalculator::calculate` 中补全全量遍历逻辑（遍历 `ACCOUNTS_TABLE`），用于：
     - 冷启动时构建完整状态根。
     - 与增量计算结果对比，做一致性自检。

4. **可观测性与调试工具**
   - 在 `block-producer` 与 `rpc-gateway` 中增加更细粒度的 tracing 埋点（如每笔交易的 gas 明细、Trie 计算耗时）。
   - 提供简单的 CLI 工具用于：
     - Dump 指定账户状态（balance/nonce/code_hash/storage 根）。
     - Dump 指定区块及其 4 个根与收据摘要。

