//! 合约字节码数据结构

use alloy_primitives::{Address, B256, Bytes};
use serde::{Deserialize, Serialize};

/// 合约字节码条目
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
    /// 合约地址
    pub address: Address,
    
    /// 字节码哈希
    pub code_hash: B256,
    
    /// 字节码内容
    pub code: Bytes,
}

impl CodeEntry {
    /// 创建新的代码条目
    pub fn new(address: Address, code: Bytes) -> Self {
        use alloy_primitives::keccak256;
        let code_hash = keccak256(&code);
        
        Self {
            address,
            code_hash,
            code,
        }
    }
    
    /// 从已知哈希创建
    pub fn with_hash(address: Address, code_hash: B256, code: Bytes) -> Self {
        Self {
            address,
            code_hash,
            code,
        }
    }
    
    /// 获取字节码大小
    pub fn size(&self) -> usize {
        self.code.len()
    }
    
    /// 检查是否为空代码
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
    
    /// 验证代码哈希
    pub fn verify_hash(&self) -> bool {
        use alloy_primitives::keccak256;
        keccak256(&self.code) == self.code_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    
    #[test]
    fn test_code_entry() {
        let addr = address!("0000000000000000000000000000000000000001");
        let code = Bytes::from(vec![0x60, 0x80, 0x60, 0x40]); // PUSH1 0x80 PUSH1 0x40
        
        let entry = CodeEntry::new(addr, code.clone());
        
        assert_eq!(entry.address, addr);
        assert_eq!(entry.size(), 4);
        assert!(!entry.is_empty());
        assert!(entry.verify_hash());
    }
    
    #[test]
    fn test_empty_code() {
        let addr = address!("0000000000000000000000000000000000000001");
        let entry = CodeEntry::new(addr, Bytes::new());
        
        assert!(entry.is_empty());
        assert_eq!(entry.size(), 0);
    }
}
