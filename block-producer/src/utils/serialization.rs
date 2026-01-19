//! 序列化工具

use serde::{Serialize, de::DeserializeOwned};

/// 序列化为字节数组（使用 bincode）
pub fn serialize_to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    bincode::serialize(value)
        .map_err(|e| format!("Serialization error: {}", e))
}

/// 从字节数组反序列化（使用 bincode）
pub fn deserialize_from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    bincode::deserialize(bytes)
        .map_err(|e| format!("Deserialization error: {}", e))
}

/// 序列化为 JSON 字符串
pub fn serialize_to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|e| format!("JSON serialization error: {}", e))
}

/// 从 JSON 字符串反序列化
pub fn deserialize_from_json<T: DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json)
        .map_err(|e| format!("JSON deserialization error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: u64,
        text: String,
    }
    
    #[test]
    fn test_bincode_serialization() {
        let data = TestData {
            value: 42,
            text: "test".to_string(),
        };
        
        let bytes = serialize_to_bytes(&data).unwrap();
        let deserialized: TestData = deserialize_from_bytes(&bytes).unwrap();
        
        assert_eq!(data, deserialized);
    }
    
    #[test]
    fn test_json_serialization() {
        let data = TestData {
            value: 42,
            text: "test".to_string(),
        };
        
        let json = serialize_to_json(&data).unwrap();
        let deserialized: TestData = deserialize_from_json(&json).unwrap();
        
        assert_eq!(data, deserialized);
    }
}
