//! Merkle Proof 生成与验证
//! 
//! 用于轻节点验证账户状态和存储值

use alloy_primitives::{Address, U256, B256, Bytes};
use serde::{Deserialize, Serialize};
use super::TrieError;

/// Merkle 证明
/// 
/// 包含从叶子节点到根节点的路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// 证明的键（地址或存储槽）
    pub key: Bytes,
    
    /// 证明的值
    pub value: Bytes,
    
    /// 证明路径（中间节点的哈希）
    pub proof: Vec<Bytes>,
    
    /// 根哈希
    pub root: B256,
}

impl MerkleProof {
    /// 创建新的 Merkle 证明
    pub fn new(key: Bytes, value: Bytes, proof: Vec<Bytes>, root: B256) -> Self {
        Self {
            key,
            value,
            proof,
            root,
        }
    }
    
    /// 验证证明
    pub fn verify(&self) -> Result<bool, TrieError> {
        // TODO: 实现完整的 Merkle proof 验证
        // 需要：
        // 1. 从叶子节点开始
        // 2. 沿着证明路径向上计算哈希
        // 3. 验证最终哈希是否等于根哈希
        Ok(true)
    }
}

/// 账户证明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProof {
    /// 账户地址
    pub address: Address,
    
    /// 账户的 Merkle 证明
    pub account_proof: MerkleProof,
    
    /// 存储槽证明（可选）
    pub storage_proofs: Vec<StorageProof>,
}

/// 存储证明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageProof {
    /// 存储槽键
    pub key: U256,
    
    /// 存储槽值
    pub value: U256,
    
    /// Merkle 证明
    pub proof: MerkleProof,
}

/// 证明验证器
pub struct ProofVerifier;

impl ProofVerifier {
    /// 验证账户证明
    pub fn verify_account_proof(proof: &AccountProof) -> Result<bool, TrieError> {
        // 验证账户证明
        proof.account_proof.verify()?;
        
        // 验证所有存储证明
        for storage_proof in &proof.storage_proofs {
            storage_proof.proof.verify()?;
        }
        
        Ok(true)
    }
    
    /// 验证存储证明
    pub fn verify_storage_proof(proof: &StorageProof) -> Result<bool, TrieError> {
        proof.proof.verify()
    }
}

// TODO: 实现证明生成器
// pub struct ProofGenerator<'a> {
//     db: &'a dyn StateDatabase,
// }
//
// impl<'a> ProofGenerator<'a> {
//     pub fn generate_account_proof(&self, address: &Address) -> Result<AccountProof, TrieError> {
//         // 1. 获取账户信息
//         // 2. 构建从叶子到根的路径
//         // 3. 收集路径上的节点哈希
//         todo!()
//     }
//     
//     pub fn generate_storage_proof(
//         &self,
//         address: &Address,
//         key: U256,
//     ) -> Result<StorageProof, TrieError> {
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_merkle_proof_creation() {
        let proof = MerkleProof::new(
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            vec![],
            B256::ZERO,
        );
        
        assert_eq!(proof.key.len(), 3);
        assert_eq!(proof.value.len(), 3);
    }
}
