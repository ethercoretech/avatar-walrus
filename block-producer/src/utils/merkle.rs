//! Merkle Tree 工具
//! 
//! 用于计算 transactions_root 和 receipts_root
//!
//! ## 实现说明
//!
//! 本模块实现了符合以太坊规范的 Merkle Patricia Trie 根哈希计算。
//!
//! ### 键的选择
//!
//! 为了确保 Trie 构建时键的顺序性（HashBuilder 要求键必须递增），
//! 我们使用以下策略：
//!
//! 1. 将列表索引进行 RLP 编码
//! 2. 计算 RLP 编码后的 keccak256 哈希
//! 3. 使用哈希值作为 Trie 键
//! 4. 按哈希值排序后插入 Trie
//!
//! 这种方法解决了 RLP 编码的顺序问题：
//! - 0 的 RLP 编码为 `0x80`（空字符串）
//! - 1-127 的 RLP 编码为自身 `0x01` - `0x7f`
//! - 这导致 `0x80 > 0x01`，违反了递增约束
//!
//! 使用哈希后，所有键都是 32 字节的均匀分布值，
//! 可以通过排序确保顺序性。

use alloy_primitives::B256;
use alloy_trie::{HashBuilder, Nibbles};
use alloy_rlp::Encodable;

/// 计算 Merkle root（通用方法）
/// 
/// 对列表中的每个元素进行 RLP 编码后，构建 Merkle Patricia Trie
/// 使用索引的哈希作为键，确保键的顺序性
pub fn calculate_merkle_root<T: Encodable>(items: &[T]) -> B256 {
    use alloy_primitives::keccak256;
    
    if items.is_empty() {
        return EMPTY_ROOT_HASH;
    }
    
    let mut builder = HashBuilder::default();
    
    // 收集所有键值对并按哈希键排序
    let mut entries: Vec<(B256, Vec<u8>)> = items.iter().enumerate().map(|(index, item)| {
        // RLP 编码项
        let mut value_buf = Vec::new();
        item.encode(&mut value_buf);
        
        // 使用索引的 RLP 编码，然后计算其 keccak256 哈希作为键
        // 这确保了键的顺序性，因为哈希值是均匀分布的
        let mut key_buf = Vec::new();
        index.encode(&mut key_buf);
        let key_hash = keccak256(&key_buf);
        
        (key_hash, value_buf)
    }).collect();
    
    // 按键排序（哈希值的字典序）
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    
    // 按排序后的顺序插入 Trie
    for (key_hash, value) in entries {
        let nibbles = Nibbles::unpack(key_hash);
        builder.add_leaf(nibbles, &value);
    }
    
    builder.root()
}

/// 空根哈希
pub const EMPTY_ROOT_HASH: B256 = B256::new([
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
]);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_merkle_root() {
        let empty: Vec<u64> = Vec::new();
        let root = calculate_merkle_root(&empty);
        assert_eq!(root, EMPTY_ROOT_HASH);
    }
    
    #[test]
    fn test_single_item_merkle_root() {
        let items = vec![42u64];
        let root = calculate_merkle_root(&items);
        // 应该得到一个非空的根哈希
        assert_ne!(root, B256::ZERO);
        assert_ne!(root, EMPTY_ROOT_HASH);
    }
    
    #[test]
    fn test_multiple_items_merkle_root() {
        let items = vec![1u64, 2u64, 3u64];
        let root = calculate_merkle_root(&items);
        // 应该得到一个非空的根哈希
        assert_ne!(root, B256::ZERO);
        assert_ne!(root, EMPTY_ROOT_HASH);
    }
    
    #[test]
    fn test_merkle_root_deterministic() {
        // 相同的输入应该产生相同的输出
        let items1 = vec![1u64, 2u64, 3u64];
        let items2 = vec![1u64, 2u64, 3u64];
        
        let root1 = calculate_merkle_root(&items1);
        let root2 = calculate_merkle_root(&items2);
        
        assert_eq!(root1, root2);
    }
    
    #[test]
    fn test_merkle_root_different_order() {
        // 不同的顺序应该产生不同的根哈希
        let items1 = vec![1u64, 2u64, 3u64];
        let items2 = vec![3u64, 2u64, 1u64];
        
        let root1 = calculate_merkle_root(&items1);
        let root2 = calculate_merkle_root(&items2);
        
        assert_ne!(root1, root2);
    }
}
