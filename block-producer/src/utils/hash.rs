//! 哈希工具

use alloy_primitives::{B256, keccak256 as alloy_keccak256};
use sha2::{Sha256, Digest};
use crate::schema::{Transaction, BlockHeader};

/// Keccak256 哈希（以太坊标准）
pub fn keccak256_hash(data: &[u8]) -> B256 {
    alloy_keccak256(data)
}

/// SHA256 哈希
pub fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 计算交易哈希
pub fn compute_tx_hash(tx: &Transaction) -> Result<B256, String> {
    let json = serde_json::to_string(tx)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
    
    Ok(keccak256_hash(json.as_bytes()))
}

/// 计算区块哈希
pub fn compute_block_hash(header: &BlockHeader) -> Result<String, String> {
    let json = serde_json::to_string(header)
        .map_err(|e| format!("Failed to serialize block header: {}", e))?;
    
    let hash = sha256_hash(json.as_bytes());
    Ok(format!("0x{}", hex::encode(hash)))
}

/// 将十六进制字符串转换为字节数组
pub fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, String> {
    let hex_clean = hex_str.trim_start_matches("0x").trim_start_matches("0X");
    hex::decode(hex_clean)
        .map_err(|e| format!("Invalid hex string: {}", e))
}

/// 将字节数组转换为十六进制字符串
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keccak256() {
        let data = b"hello world";
        let hash = keccak256_hash(data);
        
        // 验证哈希长度
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_sha256() {
        let data = b"hello world";
        let hash = sha256_hash(data);
        
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_hex_conversion() {
        let original = vec![0x01, 0x02, 0x03, 0xff];
        let hex_str = bytes_to_hex(&original);
        let decoded = hex_to_bytes(&hex_str).unwrap();
        
        assert_eq!(original, decoded);
    }
}
