/// RPC 错误码定义
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum RpcError {
    SerializationError = -32000,
    WalrusConnectionFailed = -32002,
    WalrusWriteFailed = -32003,
    RequestTimeout = -32008,
    InternalError = -32603,
}

impl RpcError {
    fn message(&self) -> &'static str {
        match self {
            RpcError::SerializationError => "序列化失败",
            RpcError::WalrusConnectionFailed => "Walrus 连接失败",
            RpcError::WalrusWriteFailed => "写入失败",
            RpcError::RequestTimeout => "请求超时",
            RpcError::InternalError => "内部错误",
        }
    }

    pub fn into_error_object(
        self,
        detail: impl Into<String>,
    ) -> jsonrpsee::types::ErrorObjectOwned {
        jsonrpsee::types::ErrorObjectOwned::owned(
            self as i32,
            format!("{}: {}", self.message(), detail.into()),
            None::<String>,
        )
    }
}
