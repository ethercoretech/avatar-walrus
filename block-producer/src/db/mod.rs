//! 数据库抽象层
//! 
//! 提供账户、存储、代码的统一读写接口

pub mod traits;
pub mod kvdb;
pub mod cache;

pub use traits::{StateDatabase, DbError};
pub use kvdb::WalrusStateDB;
pub use cache::StateCache;
