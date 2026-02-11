//! 数据结构定义
//! 
//! 定义账户、存储、代码、区块等数据结构

pub mod account;
pub mod storage;
pub mod code;
pub mod block;

pub use account::{Account, EMPTY_CODE_HASH};
pub use storage::StorageSlot;
pub use block::{Block, BlockHeader, Transaction, TransactionReceipt, Log};
