//! Trie 构建器
//! 
//! 使用 alloy-trie 的 HashBuilder 构建 Merkle Patricia Trie

use alloy_primitives::{B256, keccak256};
use alloy_trie::{HashBuilder, Nibbles};
use alloy_rlp::Encodable;
use super::TrieError;

/// Trie 构建器包装
/// 
/// 简化 alloy-trie 的使用
pub struct TrieBuilder {
    builder: HashBuilder,
}

impl TrieBuilder {
    /// 创建新的 Trie 构建器
    pub fn new() -> Self {
        Self {
            builder: HashBuilder::default(),
        }
    }
    
    /// 添加叶子节点
    /// 
    /// # 参数
    /// - `key`: 键（通常是地址或存储槽的哈希）
    /// - `value`: RLP 编码后的值
    pub fn add_leaf(&mut self, key: B256, value: &[u8]) {
        let nibbles = Nibbles::from_bytes_unchecked(key.as_slice());
        self.builder.add_leaf(nibbles, value);
    }
    
    /// 添加分支节点（用于增量更新）
    pub fn add_branch(&mut self, key: B256, value: B256, children_are_in_trie: bool) {
        let nibbles = Nibbles::from_bytes_unchecked(key.as_slice());
        self.builder.add_branch(nibbles, value, children_are_in_trie);
    }
    
    /// 计算根哈希
    pub fn root(&mut self) -> B256 {
        self.builder.root()
    }
    
    /// 重置构建器
    pub fn reset(&mut self) {
        self.builder = HashBuilder::default();
    }
}

impl Default for TrieBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 哈希键（用于 Trie）
/// 
/// 将原始键哈希后作为 Trie 的键
pub fn hash_key(key: &[u8]) -> B256 {
    keccak256(key)
}

/// RLP 编码账户
/// 
/// 将账户信息编码为 RLP 格式：[nonce, balance, storage_root, code_hash]
pub fn rlp_encode_account(
    nonce: u64,
    balance: alloy_primitives::U256,
    storage_root: B256,
    code_hash: B256,
) -> Vec<u8> {
    let mut buf = Vec::new();
    (nonce, balance, storage_root, code_hash).encode(&mut buf);
    buf
}

/// RLP 编码存储值
pub fn rlp_encode_storage_value(value: alloy_primitives::U256) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, U256, address};
    
    #[test]
    fn test_trie_builder_empty() {
        let mut builder = TrieBuilder::new();
        let root = builder.root();
        
        // 空 Trie 的根哈希
        assert_eq!(
            root,
            B256::from([
                0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
                0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
                0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
                0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
            ])
        );
    }
    
    #[test]
    fn test_trie_builder_single_leaf() {
        let mut builder = TrieBuilder::new();
        
        let addr = address!("0000000000000000000000000000000000000001");
        let hashed_addr = keccak256(addr.as_slice());
        
        let account_rlp = rlp_encode_account(
            0,
            U256::from(1000),
            B256::ZERO,
            B256::ZERO,
        );
        
        builder.add_leaf(hashed_addr, &account_rlp);
        let root = builder.root();
        
        // 应该得到一个非空的根哈希
        assert_ne!(root, B256::ZERO);
    }
    
    #[test]
    fn test_hash_key() {
        let addr = address!("0000000000000000000000000000000000000001");
        let hashed = hash_key(addr.as_slice());
        
        // 验证哈希长度
        assert_eq!(hashed.len(), 32);
    }
}
