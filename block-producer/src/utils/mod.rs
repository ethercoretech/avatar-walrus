//! 工具模块

pub mod serialization;
pub mod hash;
pub mod merkle;

// 重新导出常用的 merkle 功能
pub use merkle::{calculate_merkle_root, EMPTY_ROOT_HASH};
