//! 区块和交易数据结构
//! 
//! 扩展原有的 Block 和 Transaction，添加 EVM 执行需要的字段

use alloy_primitives::{Address, U256, B256, Bytes};
use alloy_rlp::{Encodable, BufMut};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 交易数据结构（扩展版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    // === 原有字段（保持兼容性） ===
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub data: String,
    pub gas: String,
    pub nonce: String,
    
    /// 交易哈希（可选）
    pub hash: Option<String>,
    
    /// Gas 价格（可选，用于 EVM 执行）
    pub gas_price: Option<String>,
    
    /// 链 ID（可选）
    pub chain_id: Option<u64>,
    
    /// EIP-1559: 最大 gas 费用（可选）
    pub max_fee_per_gas: Option<String>,
    
    /// EIP-1559: 最大优先费用（可选）
    pub max_priority_fee_per_gas: Option<String>,
}

impl Transaction {
    /// 转换为 alloy 地址类型
    pub fn from_address(&self) -> Result<Address, String> {
        self.from.parse()
            .map_err(|e| format!("Invalid from address: {}", e))
    }
    
    /// 转换为 alloy 地址类型（可选）
    pub fn to_address(&self) -> Result<Option<Address>, String> {
        match &self.to {
            Some(addr) => addr.parse()
                .map(Some)
                .map_err(|e| format!("Invalid to address: {}", e)),
            None => Ok(None),
        }
    }
    
    /// 解析值（以 wei 为单位）
    pub fn value_wei(&self) -> Result<U256, String> {
        let hex = self.value.trim_start_matches("0x");
        U256::from_str_radix(hex, 16)
            .map_err(|e| format!("Invalid value: {}", e))
    }
    
    /// 解析 gas 限制
    pub fn gas_limit(&self) -> Result<u64, String> {
        let hex = self.gas.trim_start_matches("0x");
        u64::from_str_radix(hex, 16)
            .map_err(|e| format!("Invalid gas: {}", e))
    }
    
    /// 解析 nonce
    pub fn nonce_value(&self) -> Result<u64, String> {
        let hex = self.nonce.trim_start_matches("0x");
        u64::from_str_radix(hex, 16)
            .map_err(|e| format!("Invalid nonce: {}", e))
    }
    
    /// 解析 data 字段
    pub fn data_bytes(&self) -> Result<Bytes, String> {
        let hex = self.data.trim_start_matches("0x");
        hex::decode(hex)
            .map(Bytes::from)
            .map_err(|e| format!("Invalid data: {}", e))
    }
    
    /// 检查是否为合约部署交易
    pub fn is_create(&self) -> bool {
        self.to.is_none()
    }
}

// 为 Transaction 实现 RLP 编码
impl Encodable for Transaction {
    fn encode(&self, out: &mut dyn BufMut) {
        // 简化实现：将交易作为列表编码
        alloy_rlp::Header {
            list: true,
            payload_length: self.length() - 1,
        }
        .encode(out);
        
        self.from.encode(out);
        // 处理 Option<String> 类型
        match &self.to {
            Some(to) => to.encode(out),
            None => (&[] as &[u8]).encode(out), // 空字节数组表示 None
        }
        self.value.encode(out);
        self.data.encode(out);
        self.gas.encode(out);
        self.nonce.encode(out);
    }
    
    fn length(&self) -> usize {
        let payload_length = self.from.length()
            + match &self.to {
                Some(to) => to.length(),
                None => (&[] as &[u8]).length(),
            }
            + self.value.length()
            + self.data.length()
            + self.gas.length()
            + self.nonce.length();
        payload_length + alloy_rlp::length_of_length(payload_length)
    }
}

/// 区块头（扩展版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// 区块号
    pub number: u64,
    
    /// 父区块哈希
    pub parent_hash: String,
    
    /// 时间戳（使用 Unix 时间戳存储，兼容 bincode）
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    
    /// 交易数量
    pub tx_count: usize,
    
    /// 交易根哈希（Merkle root）
    pub transactions_root: String,
    
    /// 状态根哈希（执行后更新）
    pub state_root: Option<String>,
    
    /// Gas 使用量（执行后填充）
    pub gas_used: Option<u64>,
    
    /// Gas 限制
    pub gas_limit: Option<u64>,
    
    /// 收据根哈希（执行后填充）
    pub receipts_root: Option<String>,
}

/// 区块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// 计算区块哈希
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let data = serde_json::to_string(&self.header).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("0x{:x}", hasher.finalize())
    }
    
    /// 获取区块号
    pub fn number(&self) -> u64 {
        self.header.number
    }
    
    /// 获取交易数量
    pub fn tx_count(&self) -> usize {
        self.transactions.len()
    }
}

/// 交易收据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// 交易哈希
    pub transaction_hash: B256,
    
    /// 交易索引
    pub transaction_index: u64,
    
    /// 区块哈希
    pub block_hash: B256,
    
    /// 区块号
    pub block_number: u64,
    
    /// 发送方
    pub from: Address,
    
    /// 接收方（合约部署时为 None）
    pub to: Option<Address>,
    
    /// 合约地址（合约部署交易）
    pub contract_address: Option<Address>,
    
    /// Gas 使用量
    pub gas_used: u64,
    
    /// 累计 Gas 使用量
    pub cumulative_gas_used: u64,
    
    /// 执行状态（1 = 成功，0 = 失败）
    pub status: u8,
    
    /// 事件日志
    pub logs: Vec<Log>,
    
    /// Logs Bloom 过滤器
    pub logs_bloom: Bytes,
}

/// 事件日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    /// 合约地址
    pub address: Address,
    
    /// Topics（索引字段）
    pub topics: Vec<B256>,
    
    /// Data（非索引字段）
    pub data: Bytes,
    
    /// 区块号
    pub block_number: u64,
    
    /// 交易哈希
    pub transaction_hash: B256,
    
    /// 交易索引
    pub transaction_index: u64,
    
    /// 日志索引
    pub log_index: u64,
}

// 为 TransactionReceipt 实现 RLP 编码
impl Encodable for TransactionReceipt {
    fn encode(&self, out: &mut dyn BufMut) {
        // 简化实现：将收据作为列表编码
        alloy_rlp::Header {
            list: true,
            payload_length: self.length() - 1,
        }
        .encode(out);
        
        self.status.encode(out);
        self.cumulative_gas_used.encode(out);
        self.logs_bloom.encode(out);
        self.logs.encode(out);
    }
    
    fn length(&self) -> usize {
        let payload_length = self.status.length()
            + self.cumulative_gas_used.length()
            + self.logs_bloom.length()
            + self.logs.length();
        payload_length + alloy_rlp::length_of_length(payload_length)
    }
}

// 为 Log 实现 RLP 编码
impl Encodable for Log {
    fn encode(&self, out: &mut dyn BufMut) {
        alloy_rlp::Header {
            list: true,
            payload_length: self.length() - 1,
        }
        .encode(out);
        
        self.address.encode(out);
        self.topics.encode(out);
        self.data.encode(out);
    }
    
    fn length(&self) -> usize {
        let payload_length = self.address.length()
            + self.topics.length()
            + self.data.length();
        payload_length + alloy_rlp::length_of_length(payload_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_parsing() {
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0xde0b6b3a7640000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas: "0x5208".to_string(), // 21000
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()), // 1 Gwei
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        assert!(tx.from_address().is_ok());
        assert!(tx.to_address().is_ok());
        assert_eq!(tx.value_wei().unwrap(), U256::from(1_000_000_000_000_000_000u64));
        assert_eq!(tx.gas_limit().unwrap(), 21000);
        assert_eq!(tx.nonce_value().unwrap(), 0);
        assert!(!tx.is_create());
    }
    
    #[test]
    fn test_contract_creation() {
        let tx = Transaction {
            from: "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: None, // 合约部署
            value: "0x0".to_string(),
            data: "0x6080604052".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        assert!(tx.is_create());
    }
}
