/// RPC 错误码定义
/// 
/// 统一管理所有 RPC 错误场景，确保错误码的稳定性与可维护性
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum RpcError {
    /// 序列化交易失败
    SerializationError = -32000,
    /// 反序列化交易失败
    DeserializationError = -32001,
    /// Walrus 服务连接异常
    WalrusConnectionFailed = -32002,
    /// 写入 Walrus 失败
    WalrusWriteFailed = -32003,
    /// 从 Walrus 读取失败
    WalrusReadFailed = -32004,
    /// 无效的十六进制数据
    InvalidHexFormat = -32005,
    /// 无效的交易格式 (RLP 解析失败)
    InvalidTransaction = -32006,
    /// 找不到指定的 Topic
    TopicNotFound = -32007,
    /// 请求超时
    RequestTimeout = -32008,
    /// 内部服务器错误
    InternalError = -32603,
}

impl RpcError {
    /// 获取错误码对应的标准化错误描述
    pub fn to_message(&self) -> &'static str {
        match self {
            RpcError::SerializationError => "序列化交易失败",
            RpcError::DeserializationError => "反序列化交易失败",
            RpcError::WalrusConnectionFailed => "Walrus 服务连接异常",
            RpcError::WalrusWriteFailed => "写入 Walrus 失败",
            RpcError::WalrusReadFailed => "从 Walrus 读取失败",
            RpcError::InvalidHexFormat => "无效的十六进制数据",
            RpcError::InvalidTransaction => "无效的交易格式 (RLP 解析失败)",
            RpcError::TopicNotFound => "找不到指定的 Topic",
            RpcError::RequestTimeout => "请求超时",
            RpcError::InternalError => "内部服务器错误",
        }
    }

    /// 将错误码转换为 JSON-RPC ErrorObject
    /// 
    /// # Arguments
    /// * `detail` - 详细错误信息，用于补充标准错误描述
    /// 
    /// # Returns
    /// 返回可直接用于 JSON-RPC 响应的 ErrorObjectOwned
    pub fn into_error_object(self, detail: impl Into<String>) -> jsonrpsee::types::ErrorObjectOwned {
        jsonrpsee::types::ErrorObjectOwned::owned(
            self as i32,
            format!("{}: {}", self.to_message(), detail.into()),
            None::<String>,
        )
    }

    /// 创建简单错误对象（无额外详情）
    /// 
    /// # Returns
    /// 返回只包含标准错误描述的 ErrorObjectOwned
    pub fn into_simple_error_object(self) -> jsonrpsee::types::ErrorObjectOwned {
        jsonrpsee::types::ErrorObjectOwned::owned(
            self as i32,
            self.to_message().to_string(),
            None::<String>,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_values() {
        assert_eq!(RpcError::SerializationError as i32, -32000);
        assert_eq!(RpcError::WalrusConnectionFailed as i32, -32002);
        assert_eq!(RpcError::InternalError as i32, -32603);
    }

    #[test]
    fn test_error_messages() {
        assert_eq!(RpcError::InvalidHexFormat.to_message(), "无效的十六进制数据");
        assert_eq!(RpcError::WalrusWriteFailed.to_message(), "写入 Walrus 失败");
    }

    #[test]
    fn test_error_object_creation() {
        let error_obj = RpcError::SerializationError.into_error_object("测试详情");
        assert_eq!(error_obj.code(), -32000);
        assert!(error_obj.message().contains("序列化交易失败"));
        assert!(error_obj.message().contains("测试详情"));
    }

    #[test]
    fn test_simple_error_object() {
        let error_obj = RpcError::InvalidTransaction.into_simple_error_object();
        assert_eq!(error_obj.code(), -32006);
        assert_eq!(error_obj.message(), "无效的交易格式 (RLP 解析失败)");
    }
}
