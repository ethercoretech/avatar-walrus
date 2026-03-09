use alloy::{
    consensus::{TxEnvelope, TxLegacy},
    eips::eip2718::Encodable2718,
    network::TxSigner,
    primitives::{Address, Bytes, U256},
    signers::local::PrivateKeySigner,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::str::FromStr;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

/// 交易生成器
///
/// 生成以太坊密钥、签名交易并发送到 RPC Gateway
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 生成新的密钥对
    GenerateKey,

    /// 生成并发送单笔交易
    SendTx {
        /// 私钥（64 位十六进制，可选 0x 前缀）
        #[arg(long)]
        private_key: String,

        /// 接收地址
        #[arg(long)]
        to: String,

        /// 转账金额（ETH）
        #[arg(long, default_value = "1.0")]
        value: f64,

        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,
    },

    /// 批量生成测试交易
    BatchGenerate {
        /// 批次大小
        #[arg(long, default_value = "10")]
        count: usize,

        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,

        /// 发送间隔（毫秒）
        #[arg(long, default_value = "0")]
        interval_ms: u64,
    },

    /// 完整流程测试（单节点）：发送交易 -> 出块/执行 -> 校验链式结构与 Slot 落盘
    ///
    /// 说明：
    /// - 使用 block-producer 内置的测试账户地址（有初始余额）
    /// - 通过 eth_sendTransaction 发送“转账 + 合约部署”，尽量拆到不同区块
    /// - 通过 block-producer 的 state-inspect 工具校验区块链式结构，并查看合约地址的 storage slots
    FullFlowTest {
        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,

        /// block-producer 数据库路径（用于 state-inspect）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/data/block_producer_state_blockchain-txs.redb")]
        state_db_path: String,

        /// state-inspect 可执行文件路径（可选；不填则自动探测 target/{release,debug}/state-inspect）
        #[arg(long)]
        state_inspect_path: Option<String>,

        /// block-producer 工作目录（用于相对路径与自动探测）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer")]
        block_producer_dir: String,

        /// 合约字节码 JSON 路径（默认使用 MiniUSDT）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/scripts/contracts/MiniUSDT.json")]
        contract_json_path: String,

        /// 等待出块的超时时间（秒）
        #[arg(long, default_value = "30")]
        timeout_secs: u64,

        /// 轮询间隔（毫秒）
        #[arg(long, default_value = "500")]
        poll_interval_ms: u64,
    },

    /// 流程测试：合约调用导致 Slot 变化
    ///
    /// - 部署 MiniUSDT 合约
    /// - 从 owner 账户调用 mint，使 storage 发生二次变化
    /// - 通过 state-inspect 对比调用前后 slot
    CallFlowTest {
        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,

        /// block-producer 数据库路径（用于 state-inspect）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/data/block_producer_state_blockchain-txs.redb")]
        state_db_path: String,

        /// state-inspect 可执行文件路径（可选；不填则自动探测 target/{release,debug}/state-inspect）
        #[arg(long)]
        state_inspect_path: Option<String>,

        /// block-producer 工作目录（用于相对路径与自动探测）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer")]
        block_producer_dir: String,

        /// 合约字节码 JSON 路径（默认使用 MiniUSDT）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/scripts/contracts/MiniUSDT.json")]
        contract_json_path: String,

        /// 等待出块的超时时间（秒）
        #[arg(long, default_value = "30")]
        timeout_secs: u64,

        /// 轮询间隔（毫秒）
        #[arg(long, default_value = "500")]
        poll_interval_ms: u64,
    },

    /// 流程测试：失败/回滚路径（revert + nonce 错误）
    ///
    /// - 部署 MiniUSDT 合约
    /// - 使用非 owner 调用 mint，触发 revert，验证 slot 不变
    /// - 构造正常 nonce + 重复 nonce，验证重复 nonce 不再额外改变 slot
    FailureFlowTest {
        /// RPC Gateway 地址
        #[arg(long, default_value = "http://localhost:8545")]
        rpc_url: String,

        /// block-producer 数据库路径（用于 state-inspect）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/data/block_producer_state_blockchain-txs.redb")]
        state_db_path: String,

        /// state-inspect 可执行文件路径（可选；不填则自动探测 target/{release,debug}/state-inspect）
        #[arg(long)]
        state_inspect_path: Option<String>,

        /// block-producer 工作目录（用于相对路径与自动探测）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer")]
        block_producer_dir: String,

        /// 合约字节码 JSON 路径（默认使用 MiniUSDT）
        #[arg(long, default_value = "/home/ubuntu/RustSpace/company/avatar-walrus/block-producer/scripts/contracts/MiniUSDT.json")]
        contract_json_path: String,

        /// 等待出块的超时时间（秒）
        #[arg(long, default_value = "30")]
        timeout_secs: u64,

        /// 轮询间隔（毫秒）
        #[arg(long, default_value = "500")]
        poll_interval_ms: u64,
    },
}

/// JSON-RPC 请求
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<serde_json::Value>,
    id: u64,
}

/// JSON-RPC 响应
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// 交易生成器
struct TxGenerator {
    rpc_url: String,
    client: reqwest::Client,
}

impl TxGenerator {
    fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    /// 生成新的密钥对
    fn generate_keypair() -> Result<PrivateKeySigner> {
        let signer = PrivateKeySigner::random();
        Ok(signer)
    }

    /// 创建交易
    fn create_transaction(to: Address, value: U256, nonce: u64) -> TxLegacy {
        TxLegacy {
            chain_id: Some(1337), // 测试链 ID
            nonce,
            gas_price: 20_000_000_000, // 20 Gwei
            gas_limit: 21000,          // 标准转账 Gas
            to: to.into(),
            value,
            input: Bytes::new(),
        }
    }

    /// 签名交易
    async fn sign_transaction(signer: &PrivateKeySigner, tx: TxLegacy) -> Result<String> {
        // 使用 TxSigner trait 的 sign_transaction 方法
        let signature = signer.sign_transaction(&mut tx.clone()).await?;

        // 构建签名的交易 envelope
        let envelope = TxEnvelope::Legacy(alloy::consensus::Signed::new_unchecked(
            tx,
            signature,
            Default::default(),
        ));

        // 编码为原始交易
        let encoded = envelope.encoded_2718();
        Ok(format!("0x{}", hex::encode(encoded)))
    }

    /// 发送交易到 RPC Gateway
    async fn send_transaction(&self, raw_tx: &str) -> Result<String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendRawTransaction".to_string(),
            params: vec![serde_json::json!(raw_tx)],
            id: 1,
        };

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json_response: JsonRpcResponse = response.json().await?;

        if let Some(error) = json_response.error {
            anyhow::bail!("RPC Error: {} ({})", error.message, error.code);
        }

        let tx_hash = json_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid result format"))?
            .to_string();

        Ok(tx_hash)
    }

    async fn rpc_call(&self, method: &str, params: Vec<serde_json::Value>, id: u64) -> Result<serde_json::Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        };

        let response = self.client.post(&self.rpc_url).json(&request).send().await?;
        let json_response: JsonRpcResponse = response.json().await?;

        if let Some(error) = json_response.error {
            anyhow::bail!("RPC Error: {} ({})", error.message, error.code);
        }

        Ok(json_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))?)
    }

    async fn rpc_health(&self) -> Result<()> {
        let _ = self.rpc_call("health", vec![], 1).await?;
        Ok(())
    }

    async fn rpc_block_number_u64(&self) -> Result<u64> {
        let v = self.rpc_call("eth_blockNumber", vec![], 1).await?;
        let s = v
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid eth_blockNumber result"))?;
        Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
    }

    async fn rpc_get_nonce_u64(&self, address_hex: &str) -> Result<u64> {
        let v = self
            .rpc_call(
                "eth_getTransactionCount",
                vec![serde_json::json!(address_hex), serde_json::json!("pending")],
                1,
            )
            .await?;
        let s = v
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid eth_getTransactionCount result"))?;
        Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
    }

    async fn send_json_transaction(&self, tx_obj: serde_json::Value, id: u64) -> Result<String> {
        let v = self
            .rpc_call("eth_sendTransaction", vec![tx_obj], id)
            .await?;
        let s = v
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid eth_sendTransaction result"))?;
        Ok(s.to_string())
    }

    async fn wait_for_block_at_least(&self, target: u64, timeout_secs: u64, poll_ms: u64) -> Result<u64> {
        let start = std::time::Instant::now();
        loop {
            let bn = self.rpc_block_number_u64().await?;
            if bn >= target {
                return Ok(bn);
            }
            if start.elapsed().as_secs() >= timeout_secs {
                anyhow::bail!("Timeout waiting for block >= {}, current={}", target, bn);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(poll_ms)).await;
        }
    }

    fn rlp_encode_bytes(bytes: &[u8]) -> Vec<u8> {
        match bytes.len() {
            0 => vec![0x80],
            1 if bytes[0] < 0x80 => vec![bytes[0]],
            len if len <= 55 => {
                let mut out = Vec::with_capacity(1 + len);
                out.push(0x80 + (len as u8));
                out.extend_from_slice(bytes);
                out
            }
            len => {
                let len_bytes = Self::to_be_len_bytes(len);
                let mut out = Vec::with_capacity(1 + len_bytes.len() + len);
                out.push(0xb7 + (len_bytes.len() as u8));
                out.extend_from_slice(&len_bytes);
                out.extend_from_slice(bytes);
                out
            }
        }
    }

    fn rlp_encode_u64(n: u64) -> Vec<u8> {
        if n == 0 {
            return vec![0x80];
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&n.to_be_bytes());
        let first_nonzero = buf.iter().position(|b| *b != 0).unwrap_or(7);
        Self::rlp_encode_bytes(&buf[first_nonzero..])
    }

    fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
        let payload_len: usize = items.iter().map(|v| v.len()).sum();
        let mut out = Vec::new();
        if payload_len <= 55 {
            out.push(0xc0 + (payload_len as u8));
        } else {
            let len_bytes = Self::to_be_len_bytes(payload_len);
            out.push(0xf7 + (len_bytes.len() as u8));
            out.extend_from_slice(&len_bytes);
        }
        for it in items {
            out.extend_from_slice(it);
        }
        out
    }

    fn to_be_len_bytes(len: usize) -> Vec<u8> {
        let mut v = Vec::new();
        let mut n = len;
        while n > 0 {
            v.push((n & 0xff) as u8);
            n >>= 8;
        }
        v.reverse();
        v
    }

    fn compute_create_address(sender: Address, nonce: u64) -> Address {
        use alloy::primitives::keccak256;
        let sender_rlp = Self::rlp_encode_bytes(sender.as_slice());
        let nonce_rlp = Self::rlp_encode_u64(nonce);
        let list = Self::rlp_encode_list(&[sender_rlp, nonce_rlp]);
        let hash = keccak256(list);
        Address::from_slice(&hash.as_slice()[12..])
    }

    fn abi_selector(signature: &str) -> [u8; 4] {
        use alloy::primitives::keccak256;
        let hash = keccak256(signature.as_bytes());
        [hash[0], hash[1], hash[2], hash[3]]
    }

    fn abi_encode_address(addr: Address) -> [u8; 32] {
        let mut out = [0u8; 32];
        let raw = addr.as_slice();
        out[12..].copy_from_slice(raw);
        out
    }

    fn abi_encode_u64(amount: u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[24..].copy_from_slice(&amount.to_be_bytes());
        out
    }

    fn build_erc20_mint_calldata(to: Address, amount: u64) -> String {
        let selector = Self::abi_selector("mint(address,uint256)");
        let to_bytes = Self::abi_encode_address(to);
        let amount_bytes = Self::abi_encode_u64(amount);

        let mut data = Vec::with_capacity(4 + 32 * 2);
        data.extend_from_slice(&selector);
        data.extend_from_slice(&to_bytes);
        data.extend_from_slice(&amount_bytes);

        format!("0x{}", hex::encode(data))
    }

    fn load_contract_bytecode_hex(contract_json_path: &str) -> Result<String> {
        let text = std::fs::read_to_string(contract_json_path)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;
        let bytecode = v
            .get("bytecode")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("contract json missing 'bytecode' string field"))?;
        Ok(bytecode.to_string())
    }

    fn find_state_inspect(block_producer_dir: &str, explicit: &Option<String>) -> Result<PathBuf> {
        if let Some(p) = explicit {
            return Ok(PathBuf::from(p));
        }
        let base = PathBuf::from(block_producer_dir);
        let candidates = [
            base.join("target/release/state-inspect"),
            base.join("target/debug/state-inspect"),
        ];
        for c in candidates {
            if c.exists() {
                return Ok(c);
            }
        }
        anyhow::bail!(
            "state-inspect not found under target/{{release,debug}}; please build it or pass --state-inspect-path"
        );
    }

    fn run_state_inspect_blocks(state_inspect: &PathBuf, block_producer_dir: &str, db_path: &str) -> Result<String> {
        let out = ProcessCommand::new(state_inspect)
            .current_dir(block_producer_dir)
            .arg("--db-path")
            .arg(db_path)
            .arg("blocks")
            .arg("--from")
            .arg("0")
            .arg("--limit")
            .arg("20")
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn run_state_inspect_storage(state_inspect: &PathBuf, block_producer_dir: &str, db_path: &str, addr: Address) -> Result<String> {
        let out = ProcessCommand::new(state_inspect)
            .current_dir(block_producer_dir)
            .arg("--db-path")
            .arg(db_path)
            .arg("storage")
            .arg("--address")
            .arg(format!("0x{}", hex::encode(addr.as_slice())))
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn count_blocks_from_state_inspect(output: &str) -> u64 {
        output
            .lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .count() as u64
    }

    async fn wait_for_db_blocks(
        state_inspect: &PathBuf,
        block_producer_dir: &str,
        db_path: &str,
        target_blocks: u64,
        timeout_secs: u64,
        poll_ms: u64,
    ) -> Result<String> {
        let start = std::time::Instant::now();
        loop {
            let out = Self::run_state_inspect_blocks(state_inspect, block_producer_dir, db_path)?;
            let n = Self::count_blocks_from_state_inspect(&out);
            if n >= target_blocks {
                return Ok(out);
            }
            if start.elapsed().as_secs() >= timeout_secs {
                anyhow::bail!("Timeout waiting for db blocks >= {}, current={}", target_blocks, n);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(poll_ms)).await;
        }
    }

    async fn full_flow_test(
        &self,
        block_producer_dir: String,
        state_db_path: String,
        state_inspect_path: Option<String>,
        contract_json_path: String,
        timeout_secs: u64,
        poll_interval_ms: u64,
    ) -> Result<()> {
        // 使用 block-producer 内置钱包（有初始余额）
        let from = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
        let to = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";

        self.rpc_health().await?;
        let before_bn = self.rpc_block_number_u64().await?;
        info!("当前区块号(RPC): {}", before_bn);

        let start_nonce = self.rpc_get_nonce_u64(from).await?;
        info!("发送账户 nonce(pending): {}", start_nonce);

        let state_inspect = Self::find_state_inspect(&block_producer_dir, &state_inspect_path)?;
        let before_blocks_out =
            Self::run_state_inspect_blocks(&state_inspect, &block_producer_dir, &state_db_path)?;
        let before_blocks = Self::count_blocks_from_state_inspect(&before_blocks_out);
        info!("当前区块数量(DB): {}", before_blocks);

        // 1) 转账交易（尽量单独进一个区块）
        let transfer_tx = serde_json::json!({
            "from": from,
            "to": to,
            "value": "0xde0b6b3a7640000", // 1 ETH
            "data": "0x",
            "gas": "0x5208", // 21000
            "nonce": format!("0x{:x}", start_nonce),
        });
        let transfer_hash = self.send_json_transaction(transfer_tx, start_nonce).await?;
        info!("✅ 转账已发送: {}", transfer_hash);

        // 等待数据库里出现至少 1 个新区块
        let _ = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 1,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        // 2) 合约部署（MiniUSDT），nonce = start_nonce + 1
        let bytecode = Self::load_contract_bytecode_hex(&contract_json_path)?;
        let deploy_nonce = start_nonce + 1;
        let deploy_tx = serde_json::json!({
            "from": from,
            "value": "0x0",
            "data": bytecode,
            "gas": "0x1e8480", // 2,000,000
            "nonce": format!("0x{:x}", deploy_nonce),
        });
        let deploy_hash = self.send_json_transaction(deploy_tx, deploy_nonce).await?;
        info!("✅ 部署已发送: {}", deploy_hash);

        let blocks_out = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 2,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;
        let final_bn = self.rpc_block_number_u64().await.unwrap_or(before_bn);
        info!("✅ 已观察到新区块(DB>=+2)，当前区块号(RPC): {}", final_bn);

        // 3) 计算合约地址并检查 slot
        let from_addr = Address::from_str(from)?;
        let contract_addr = Self::compute_create_address(from_addr, deploy_nonce);
        info!("推导合约地址(基于 from+nonce): 0x{}", hex::encode(contract_addr.as_slice()));

        // 4) 调用 state-inspect 校验链式结构 + slot 存储
        let storage_out = Self::run_state_inspect_storage(&state_inspect, &block_producer_dir, &state_db_path, contract_addr)?;

        println!("=== state-inspect blocks ===");
        print!("{blocks_out}");
        println!("=== state-inspect storage (contract) ===");
        print!("{storage_out}");

        Ok(())
    }

    /// 合约调用流程测试：验证 mint 调用会让合约 storage 发生二次变化
    async fn call_flow_test(
        &self,
        block_producer_dir: String,
        state_db_path: String,
        state_inspect_path: Option<String>,
        contract_json_path: String,
        timeout_secs: u64,
        poll_interval_ms: u64,
    ) -> Result<()> {
        // 使用与 full_flow_test 相同的测试账户
        let from = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"; // owner / 部署者
        let to = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";   // 接收者

        self.rpc_health().await?;

        let state_inspect = Self::find_state_inspect(&block_producer_dir, &state_inspect_path)?;
        let before_blocks_out =
            Self::run_state_inspect_blocks(&state_inspect, &block_producer_dir, &state_db_path)?;
        let before_blocks = Self::count_blocks_from_state_inspect(&before_blocks_out);
        info!("当前区块数量(DB): {}", before_blocks);

        // 1) 部署 MiniUSDT 合约
        let start_nonce = self.rpc_get_nonce_u64(from).await?;
        info!("部署账户 nonce(pending): {}", start_nonce);

        let bytecode = Self::load_contract_bytecode_hex(&contract_json_path)?;
        let deploy_tx = serde_json::json!({
            "from": from,
            "value": "0x0",
            "data": bytecode,
            "gas": "0x1e8480", // 2,000,000
            "nonce": format!("0x{:x}", start_nonce),
        });
        let deploy_hash = self.send_json_transaction(deploy_tx, start_nonce).await?;
        info!("✅ 部署已发送: {}", deploy_hash);

        // 等待部署交易被打包
        let _ = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 1,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        // 计算合约地址并读取调用前的 storage
        let from_addr = Address::from_str(from)?;
        let contract_addr = Self::compute_create_address(from_addr, start_nonce);
        info!(
            "推导合约地址(部署后): 0x{}",
            hex::encode(contract_addr.as_slice())
        );

        let storage_before = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        // 2) 从 owner 调用 mint(to, amount) 触发 SSTORE
        let call_nonce = self.rpc_get_nonce_u64(from).await?;
        info!("合约调用 nonce(pending): {}", call_nonce);
        let to_addr = Address::from_str(to)?;
        let calldata = Self::build_erc20_mint_calldata(to_addr, 1_000);

        let call_tx = serde_json::json!({
            "from": from,
            "to": format!("0x{}", hex::encode(contract_addr.as_slice())),
            "value": "0x0",
            "data": calldata,
            "gas": "0x1e8480",
            "nonce": format!("0x{:x}", call_nonce),
        });
        let call_hash = self.send_json_transaction(call_tx, call_nonce).await?;
        info!("✅ mint 调用已发送: {}", call_hash);

        let blocks_out = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 2,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        let storage_after = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        println!("=== state-inspect storage (before mint call) ===");
        print!("{storage_before}");
        println!("=== state-inspect storage (after successful mint call) ===");
        print!("{storage_after}");
        println!("=== state-inspect blocks ===");
        print!("{blocks_out}");

        Ok(())
    }

    /// 失败/回滚流程测试：revert + nonce 错误
    async fn failure_flow_test(
        &self,
        block_producer_dir: String,
        state_db_path: String,
        state_inspect_path: Option<String>,
        contract_json_path: String,
        timeout_secs: u64,
        poll_interval_ms: u64,
    ) -> Result<()> {
        let from = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"; // owner / 部署者
        let to = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";   // 第二个测试账户（非 owner）

        self.rpc_health().await?;

        let state_inspect = Self::find_state_inspect(&block_producer_dir, &state_inspect_path)?;
        let before_blocks_out =
            Self::run_state_inspect_blocks(&state_inspect, &block_producer_dir, &state_db_path)?;
        let before_blocks = Self::count_blocks_from_state_inspect(&before_blocks_out);
        info!("当前区块数量(DB): {}", before_blocks);

        // 1) 部署合约
        let from_nonce0 = self.rpc_get_nonce_u64(from).await?;
        info!("部署账户 nonce(pending): {}", from_nonce0);

        let bytecode = Self::load_contract_bytecode_hex(&contract_json_path)?;
        let deploy_tx = serde_json::json!({
            "from": from,
            "value": "0x0",
            "data": bytecode,
            "gas": "0x1e8480",
            "nonce": format!("0x{:x}", from_nonce0),
        });
        let deploy_hash = self.send_json_transaction(deploy_tx, from_nonce0).await?;
        info!("✅ 部署已发送: {}", deploy_hash);

        let _ = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 1,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        let from_addr = Address::from_str(from)?;
        let contract_addr = Self::compute_create_address(from_addr, from_nonce0);
        info!(
            "推导合约地址(部署后): 0x{}",
            hex::encode(contract_addr.as_slice())
        );

        let storage_after_deploy = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        // 2) revert 场景：非 owner 调用 mint，预期整笔交易回滚，slot 不变
        let to_addr = Address::from_str(to)?;
        let revert_nonce = self.rpc_get_nonce_u64(to).await?;
        info!("revert 测试账户(to) nonce(pending): {}", revert_nonce);
        let revert_calldata = Self::build_erc20_mint_calldata(to_addr, 1_234_567);

        let revert_tx = serde_json::json!({
            "from": to,
            "to": format!("0x{}", hex::encode(contract_addr.as_slice())),
            "value": "0x0",
            "data": revert_calldata,
            "gas": "0x1e8480",
            "nonce": format!("0x{:x}", revert_nonce),
        });
        let revert_hash = self.send_json_transaction(revert_tx, revert_nonce).await?;
        info!(
            "✅ revert 测试交易已发送(预期执行失败并回滚): {}",
            revert_hash
        );

        let _ = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 2,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        let storage_after_revert = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        // 3) nonce 场景：先发一笔正常 mint，再用同一个 nonce 重复发送
        let valid_nonce = self.rpc_get_nonce_u64(from).await?;
        info!("nonce 测试 from 当前 nonce(pending): {}", valid_nonce);
        let beneficiary = Address::from_str(to)?;
        let valid_calldata = Self::build_erc20_mint_calldata(beneficiary, 9_999);

        let valid_tx = serde_json::json!({
            "from": from,
            "to": format!("0x{}", hex::encode(contract_addr.as_slice())),
            "value": "0x0",
            "data": valid_calldata,
            "gas": "0x1e8480",
            "nonce": format!("0x{:x}", valid_nonce),
        });
        let valid_hash = self.send_json_transaction(valid_tx.clone(), valid_nonce).await?;
        info!("✅ nonce 正常交易已发送: {}", valid_hash);

        let _ = Self::wait_for_db_blocks(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            before_blocks + 3,
            timeout_secs,
            poll_interval_ms,
        )
        .await?;

        let storage_after_valid = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        // 使用同一个 nonce 再发送一次，观察 RPC/执行层行为，并确认不会额外改变 slot
        let dup_tx = valid_tx;
        let dup_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendTransaction".to_string(),
            params: vec![dup_tx],
            id: valid_nonce + 10_000, // 只是一个区分用的 id
        };

        let dup_response = self
            .client
            .post(&self.rpc_url)
            .json(&dup_request)
            .send()
            .await?;
        let dup_json: JsonRpcResponse = dup_response.json().await?;

        if let Some(err) = dup_json.error {
            info!(
                "✅ 重复 nonce 被 RPC 拒绝: code={}, message={}",
                err.code, err.message
            );
        } else {
            let tx_hash = dup_json
                .result
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            info!(
                "⚠️ 重复 nonce 被 RPC 接受(由执行层决定是否丢弃/覆盖)，tx_hash={}",
                tx_hash
            );

            let _ = Self::wait_for_db_blocks(
                &state_inspect,
                &block_producer_dir,
                &state_db_path,
                before_blocks + 4,
                timeout_secs,
                poll_interval_ms,
            )
            .await?;
        }

        let storage_after_dup = Self::run_state_inspect_storage(
            &state_inspect,
            &block_producer_dir,
            &state_db_path,
            contract_addr,
        )?;

        let blocks_out =
            Self::run_state_inspect_blocks(&state_inspect, &block_producer_dir, &state_db_path)?;

        println!("=== state-inspect storage (after deploy) ===");
        print!("{storage_after_deploy}");
        println!("=== state-inspect storage (after revert call, expect NO CHANGE) ===");
        print!("{storage_after_revert}");
        println!("=== state-inspect storage (after valid nonce mint) ===");
        print!("{storage_after_valid}");
        println!("=== state-inspect storage (after duplicate nonce tx, expect NO EXTRA CHANGE) ===");
        print!("{storage_after_dup}");
        println!("=== state-inspect blocks ===");
        print!("{blocks_out}");

        Ok(())
    }

    /// 生成并发送单笔交易
    async fn generate_and_send(
        &self,
        private_key: &str,
        to_address: &str,
        value_eth: f64,
    ) -> Result<String> {
        // 1. 加载签名器
        let signer = private_key.parse::<PrivateKeySigner>()?;
        let from_address = signer.address();
        info!("发送地址: {:?}", from_address);

        // 2. 解析接收地址
        let to = Address::from_str(to_address)?;

        // 3. 转换金额（ETH to Wei）
        let value = U256::from((value_eth * 1e18) as u64);

        // 4. 创建交易（使用随机 nonce 用于测试）
        let nonce = rand::thread_rng().gen::<u32>() as u64;
        let tx = Self::create_transaction(to, value, nonce);

        info!(
            "创建交易: {:?} -> {:?}, 金额: {} ETH",
            from_address, to, value_eth
        );

        // 5. 签名交易
        let raw_tx = Self::sign_transaction(&signer, tx).await?;
        info!("交易已签名, 原始交易: {}...", &raw_tx[..20]);

        // 6. 发送交易
        let tx_hash = self.send_transaction(&raw_tx).await?;
        info!("✅ 交易已发送, 哈希: {}", tx_hash);

        Ok(tx_hash)
    }

    /// 批量生成测试交易
    async fn batch_generate(&self, count: usize, interval_ms: u64) -> Result<()> {
        info!("🚀 开始批量生成 {} 笔测试交易", count);

        // 记录开始时间
        let start_time = std::time::Instant::now();
        let mut success_count = 0;
        let mut failure_count = 0;

        for i in 0..count {
            // 生成随机密钥对
            let signer = Self::generate_keypair()?;

            // 生成随机接收地址
            let to_signer = Self::generate_keypair()?;
            let to_address = to_signer.address();

            // 随机金额（0.1 - 10 ETH）
            let value_eth = rand::thread_rng().gen_range(0.1..10.0);

            // 创建交易
            let nonce = i as u64;
            let value = U256::from((value_eth * 1e18) as u64);
            let tx = Self::create_transaction(to_address, value, nonce);

            // 签名
            let raw_tx = Self::sign_transaction(&signer, tx).await?;

            // 发送
            match self.send_transaction(&raw_tx).await {
                Ok(tx_hash) => {
                    success_count += 1;
                    info!(
                        "[{}/{}] ✅ 交易已发送: {} ({:.2} ETH)",
                        i + 1,
                        count,
                        &tx_hash[..16],
                        value_eth
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    warn!("[{}/{}] ❌ 发送失败: {}", i + 1, count, e);
                }
            }

            // 等待间隔
            if interval_ms > 0 && i < count - 1 {
                info!("休息 {} 豪秒", interval_ms);
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        }

        // 计算总耗时
        let elapsed = start_time.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let tps = success_count as f64 / elapsed_secs;

        info!("🎉 批量生成完成！");
        info!("📊 统计信息:");
        info!("   总交易数: {}", count);
        info!("   成功: {} 笔", success_count);
        info!("   失败: {} 笔", failure_count);
        info!("   总耗时: {:.2} 秒", elapsed_secs);
        info!("   平均吞吐量: {:.2} TPS (交易/秒)", tps);
        info!(
            "   平均延迟: {:.2} ms/交易",
            elapsed_secs * 1000.0 / count as f64
        );

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    match args.command {
        Command::GenerateKey => {
            let signer = TxGenerator::generate_keypair()?;
            println!("🔑 新密钥对已生成:");
            println!();
            println!("地址:     {:?}", signer.address());
            println!("私钥:     {}", hex::encode(signer.to_bytes()));
            println!();
            println!("⚠️  请妥善保管私钥，不要泄露！");
        }

        Command::SendTx {
            private_key,
            to,
            value,
            rpc_url,
        } => {
            let generator = TxGenerator::new(rpc_url);
            let tx_hash = generator
                .generate_and_send(&private_key, &to, value)
                .await?;
            println!("✅ 交易哈希: {}", tx_hash);
        }

        Command::BatchGenerate {
            count,
            rpc_url,
            interval_ms,
        } => {
            let generator = TxGenerator::new(rpc_url);
            generator.batch_generate(count, interval_ms).await?;
        }

        Command::FullFlowTest {
            rpc_url,
            state_db_path,
            state_inspect_path,
            block_producer_dir,
            contract_json_path,
            timeout_secs,
            poll_interval_ms,
        } => {
            let generator = TxGenerator::new(rpc_url);
            generator
                .full_flow_test(
                    block_producer_dir,
                    state_db_path,
                    state_inspect_path,
                    contract_json_path,
                    timeout_secs,
                    poll_interval_ms,
                )
                .await?;
        }

        Command::CallFlowTest {
            rpc_url,
            state_db_path,
            state_inspect_path,
            block_producer_dir,
            contract_json_path,
            timeout_secs,
            poll_interval_ms,
        } => {
            let generator = TxGenerator::new(rpc_url);
            generator
                .call_flow_test(
                    block_producer_dir,
                    state_db_path,
                    state_inspect_path,
                    contract_json_path,
                    timeout_secs,
                    poll_interval_ms,
                )
                .await?;
        }

        Command::FailureFlowTest {
            rpc_url,
            state_db_path,
            state_inspect_path,
            block_producer_dir,
            contract_json_path,
            timeout_secs,
            poll_interval_ms,
        } => {
            let generator = TxGenerator::new(rpc_url);
            generator
                .failure_flow_test(
                    block_producer_dir,
                    state_db_path,
                    state_inspect_path,
                    contract_json_path,
                    timeout_secs,
                    poll_interval_ms,
                )
                .await?;
        }
    }

    Ok(())
}
