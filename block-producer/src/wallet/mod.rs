//! 内置钱包模块
//! 
//! 提供预配置的测试账户，用于自动交易发送。
//! 这些账户在数据库初始化时自动创建并预存充足余额。

use alloy_primitives::{Address, U256};
use std::str::FromStr;

/// 内置钱包账户配置
#[derive(Debug, Clone)]
pub struct BuiltInWallet {
    /// 账户地址
    pub address: Address,
    /// 私钥 (十六进制字符串)
    pub private_key: String,
    /// 初始余额 (ETH单位)
    pub initial_balance_eth: u64,
}

impl BuiltInWallet {
    /// 创建新的内置钱包
    pub fn new(address: &str, private_key: &str, initial_balance_eth: u64) -> Self {
        Self {
            address: Address::from_str(address).expect("Invalid address format"),
            private_key: private_key.to_string(),
            initial_balance_eth,
        }
    }

    /// 获取账户的 wei 余额
    pub fn initial_balance_wei(&self) -> U256 {
        U256::from(self.initial_balance_eth) * U256::from(10u64.pow(18))
    }
}

/// 获取所有预配置的内置钱包账户
/// 
/// 返回一组带充足余额的测试账户，用于自动交易发送
pub fn get_builtin_wallets() -> Vec<BuiltInWallet> {
    vec![
        // 主测试账户 - 10000 ETH
        BuiltInWallet::new(
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            10000,
        ),
        // 第二个测试账户 - 5000 ETH
        BuiltInWallet::new(
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
            5000,
        ),
        // 第三个测试账户 - 5000 ETH
        BuiltInWallet::new(
            "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
            "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
            5000,
        ),
    ]
}

/// 获取默认的发送账户（第一个账户）
pub fn get_default_sender() -> BuiltInWallet {
    get_builtin_wallets()
        .into_iter()
        .next()
        .expect("No builtin wallets configured")
}

/// 根据地址查找内置钱包
pub fn find_wallet_by_address(address: &Address) -> Option<BuiltInWallet> {
    get_builtin_wallets()
        .into_iter()
        .find(|wallet| &wallet.address == address)
}

/// 检查地址是否为内置钱包地址
pub fn is_builtin_address(address: &Address) -> bool {
    find_wallet_by_address(address).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = BuiltInWallet::new(
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            1000,
        );
        
        assert_eq!(wallet.initial_balance_eth, 1000);
        assert_eq!(wallet.initial_balance_wei(), U256::from(1000) * U256::from(10u64.pow(18)));
    }

    #[test]
    fn test_get_wallets() {
        let wallets = get_builtin_wallets();
        assert_eq!(wallets.len(), 3);
        
        let default = get_default_sender();
        assert_eq!(default.initial_balance_eth, 10000);
    }

    #[test]
    fn test_find_wallet() {
        let address = Address::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
        let wallet = find_wallet_by_address(&address);
        assert!(wallet.is_some());
        assert_eq!(wallet.unwrap().initial_balance_eth, 10000);
    }
}