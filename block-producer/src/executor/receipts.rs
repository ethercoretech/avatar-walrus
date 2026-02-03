//! 交易收据构建器

use alloy_primitives::{B256, Bytes};
use crate::schema::{TransactionReceipt, Log, Block, Transaction};
use crate::executor::ExecutionResult;

/// 收据构建器
pub struct ReceiptBuilder;

impl ReceiptBuilder {
    /// 构建交易收据
    pub fn build(
        tx_hash: B256,
        tx_index: u64,
        block: &Block,
        tx: &Transaction,
        result: &ExecutionResult,
        cumulative_gas_used: u64,
    ) -> TransactionReceipt {
        TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: tx_index,
            block_hash: Self::parse_block_hash(&block.hash()),
            block_number: block.header.number,
            from: tx.from_address().unwrap_or_default(),
            to: tx.to_address().ok().flatten(),
            contract_address: result.contract_address,
            gas_used: result.gas_used,
            cumulative_gas_used,
            status: if result.success { 1 } else { 0 },
            logs: Self::convert_logs(&result.logs, tx_hash, tx_index, block.header.number),
            logs_bloom: Self::compute_logs_bloom(&result.logs),
        }
    }
    
    /// 解析区块哈希
    fn parse_block_hash(hash_str: &str) -> B256 {
        let hex = hash_str.trim_start_matches("0x");
        B256::from_slice(&hex::decode(hex).unwrap_or_default())
    }
    
    /// 转换日志格式
    fn convert_logs(
        revm_logs: &[revm::primitives::Log],
        tx_hash: B256,
        tx_index: u64,
        block_number: u64,
    ) -> Vec<Log> {
        revm_logs
            .iter()
            .enumerate()
            .map(|(log_index, log)| Log {
                address: log.address,
                topics: log.data.topics().to_vec(),
                data: log.data.data.clone(),
                block_number,
                transaction_hash: tx_hash,
                transaction_index: tx_index,
                log_index: log_index as u64,
            })
            .collect()
    }
    
    /// 计算 Logs Bloom 过滤器
    fn compute_logs_bloom(_logs: &[revm::primitives::Log]) -> Bytes {
        // TODO: 实现完整的 Bloom filter 计算
        // 当前返回空 bloom
        Bytes::from(vec![0u8; 256])
    }
}
