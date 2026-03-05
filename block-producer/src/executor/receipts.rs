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
    
    /// 计算 Logs Bloom 过滤器（2048-bit，以太坊兼容）
    fn compute_logs_bloom(logs: &[revm::primitives::Log]) -> Bytes {
        use alloy_primitives::keccak256;

        // 2048 bit = 256 bytes
        let mut bloom = [0u8; 256];

        // 按以太坊规范：对地址和每个 topic 做 keccak256，
        // 取前 6 字节生成 3 个 11 bit 的索引，并在 2048 bit Bloom 中置位。
        fn add_to_bloom(bloom: &mut [u8; 256], bytes: &[u8]) {
            let hash = keccak256(bytes);
            for i in 0..3 {
                let hi = (hash[2 * i] as u16) << 8 | hash[2 * i + 1] as u16;
                let bit = (hi & 0x7ff) as usize; // 0..2047
                let byte_index = bit / 8;
                let bit_index = bit % 8;
                bloom[byte_index] |= 1u8 << bit_index;
            }
        }

        for log in logs {
            // 地址
            add_to_bloom(&mut bloom, log.address.as_slice());
            // topics
            for topic in log.data.topics() {
                add_to_bloom(&mut bloom, topic.as_slice());
            }
        }

        Bytes::from(bloom.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256, Bytes as AlloyBytes, hex::FromHex};

    fn make_log(addr_hex: &str, topic_hex: &str) -> revm::primitives::Log {
        use revm::primitives::{LogData, Log as RevmLog};

        let addr: Address = addr_hex.parse().unwrap();
        let topic_bytes = <[u8; 32]>::from_hex(topic_hex.trim_start_matches("0x")).unwrap();
        let topic = B256::from_slice(&topic_bytes);
        let data = AlloyBytes::from(vec![0u8; 0]);

        RevmLog {
            address: addr,
            data: LogData::new_unchecked(vec![topic], data),
        }
    }

    #[test]
    fn bloom_empty_logs_is_all_zero() {
        let logs: Vec<revm::primitives::Log> = Vec::new();
        let bloom = ReceiptBuilder::compute_logs_bloom(&logs);
        assert_eq!(bloom.len(), 256);
        assert!(bloom.iter().all(|b| *b == 0));
    }

    #[test]
    fn bloom_is_deterministic_for_same_logs() {
        let log = make_log(
            "0000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000001",
        );
        let logs = vec![log];

        let b1 = ReceiptBuilder::compute_logs_bloom(&logs);
        let b2 = ReceiptBuilder::compute_logs_bloom(&logs);
        assert_eq!(b1, b2);
    }

    #[test]
    fn bloom_changes_when_log_changes() {
        let log1 = make_log(
            "0000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000001",
        );
        let log2 = make_log(
            "0000000000000000000000000000000000000002",
            "0000000000000000000000000000000000000000000000000000000000000001",
        );

        let b1 = ReceiptBuilder::compute_logs_bloom(&[log1]);
        let b2 = ReceiptBuilder::compute_logs_bloom(&[log2]);
        assert_ne!(b1, b2);
    }
}
