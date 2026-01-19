use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use std::fs::File;
use std::io::Write;

fn main() -> Result<()> {
    println!("🔑 开始生成 50 个私钥和对应的钱包地址...");

    // 基础私钥：0x2222222222222222222222222222222222222222222222222222222222220000
    let base_private_key = "0x2222222222222222222222222222222222222222222222222222222222220000";

    // 解析基础私钥（去掉 0x 前缀）
    let base_hex = base_private_key.strip_prefix("0x").unwrap();
    let mut base_bytes = hex::decode(base_hex)?;

    // 确保是 32 字节
    if base_bytes.len() != 32 {
        anyhow::bail!("基础私钥长度不正确，应该是 32 字节");
    }

    // 创建输出文件
    let mut csv_file = File::create("generated_keys.csv")?;
    writeln!(csv_file, "index,private_key,address")?;

    let mut json_file = File::create("generated_keys.json")?;
    writeln!(json_file, "[")?;

    let total = 50;
    for i in 0..total {
        // 将索引添加到最后 2 个字节（大端序，因为十六进制显示是从左到右）
        // 索引 i 对应十六进制值，例如 i=0 -> 0x0000, i=49 -> 0x0031
        let index = i as u16;
        base_bytes[30] = (index >> 8) as u8;
        base_bytes[31] = (index & 0xFF) as u8;

        // 将字节数组转换为十六进制字符串
        let private_key_hex = format!("0x{}", hex::encode(&base_bytes));

        // 从私钥创建签名器（将 Vec<u8> 转换为 [u8; 32]）
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&base_bytes);
        let signer = PrivateKeySigner::from_bytes(&key_bytes.into())?;
        let address = signer.address();

        // 写入 CSV
        writeln!(csv_file, "{},{},{:?}", i, private_key_hex, address)?;

        // 写入 JSON（除了最后一个，其他后面加逗号）
        if i < total - 1 {
            writeln!(
                json_file,
                "  {{\"index\": {}, \"private_key\": \"{}\", \"address\": \"{:?}\"}},",
                i, private_key_hex, address
            )?;
        } else {
            writeln!(
                json_file,
                "  {{\"index\": {}, \"private_key\": \"{}\", \"address\": \"{:?}\"}}",
                i, private_key_hex, address
            )?;
        }

        if (i + 1) % 10 == 0 {
            println!("✅ 已生成 {} 个密钥...", i + 1);
        }
    }

    writeln!(json_file, "]")?;

    println!("✅ 完成！已生成 {} 个密钥", total);
    println!("📄 CSV 文件: generated_keys.csv");
    println!("📄 JSON 文件: generated_keys.json");

    Ok(())
}
