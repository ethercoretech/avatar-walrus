use crate::controller::NodeController;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

const MAX_FRAME_LEN: usize = 64 * 1024;

/// 将hexstring (如 "0x1234" 或 "0X1234") 解析为字节数组
fn parse_hexstring(s: &str) -> Result<Vec<u8>> {
    let hex_part = s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| anyhow!("hexstring must start with 0x or 0X"))?;

    // 处理奇数长度的hex字符串，在前面补0
    let hex_clean = if hex_part.len() % 2 == 1 {
        format!("0{}", hex_part)
    } else {
        hex_part.to_string()
    };

    let mut bytes = Vec::new();
    for i in (0..hex_clean.len()).step_by(2) {
        let byte_str = &hex_clean[i..i + 2];
        let byte = u8::from_str_radix(byte_str, 16)
            .map_err(|e| anyhow!("invalid hex byte '{}': {}", byte_str, e))?;
        bytes.push(byte);
    }

    Ok(bytes)
}

/// 将字节数组编码为hexstring格式 (如 "0x1234")
fn encode_hexstring(bytes: &[u8]) -> String {
    format!("0x{}", bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>())
}

pub async fn start_client_listener(
    controller: Arc<NodeController>,
    bind_addr: String,
) -> Result<()> {
    let listener = TcpListener::bind(&bind_addr).await?;
    info!("Client listener bound on {}", bind_addr);

    loop {
        let (socket, addr) = listener.accept().await?;
        let controller_clone = controller.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, controller_clone).await {
                warn!("Client connection {} closed with error: {}", addr, e);
            }
        });
    }
}

async fn handle_connection(mut socket: TcpStream, controller: Arc<NodeController>) -> Result<()> {
    loop {
        let mut len_buf = [0u8; 4];
        if let Err(e) = socket.read_exact(&mut len_buf).await {
            // Graceful EOF ends the loop; bubble up real errors.
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(());
            }
            return Err(e.into());
        }

        let frame_len = u32::from_le_bytes(len_buf) as usize;
        if frame_len == 0 || frame_len > MAX_FRAME_LEN {
            send_response(&mut socket, "ERR invalid frame length").await?;
            continue;
        }

        let mut buf = vec![0u8; frame_len];
        socket.read_exact(&mut buf).await?;
        let text = match String::from_utf8(buf) {
            Ok(s) => s,
            Err(_) => {
                send_response(&mut socket, "ERR invalid utf-8").await?;
                continue;
            }
        };

        let response = match handle_command(text.trim_end(), controller.clone()).await {
            Ok(msg) => msg,
            Err(e) => format!("ERR {}", e),
        };

        send_response(&mut socket, &response).await?;
    }
}

async fn handle_command(line: &str, controller: Arc<NodeController>) -> Result<String> {
    let mut parts = line.splitn(3, ' ');
    let Some(op) = parts.next() else {
        return Err(anyhow!("empty command"));
    };
    tracing::info!("client command received: {}", line);

    match op {
        "REGISTER" => {
            let topic = parts
                .next()
                .ok_or_else(|| anyhow!("REGISTER requires a topic"))?;
            controller.ensure_topic(topic).await?;
            Ok("OK".into())
        }
        "PUT" => {
            let topic = parts
                .next()
                .ok_or_else(|| anyhow!("PUT requires a topic"))?;
            let payload = parts
                .next()
                .ok_or_else(|| anyhow!("PUT requires a payload"))?;

            // 只接受hexstring格式（必须以0x或0X开头）
            if !payload.starts_with("0x") && !payload.starts_with("0X") {
                return Err(anyhow!("payload must be hexstring format (start with 0x or 0X)"));
            }

            // 解析hexstring为字节数组并直接存储
            let bytes = parse_hexstring(payload)?;
            controller.append_for_topic(topic, bytes).await?;
            Ok("OK".into())
        }
        "GET" => {
            let topic = parts
                .next()
                .ok_or_else(|| anyhow!("GET requires a topic"))?;
            match controller.read_one_for_topic_shared(topic).await? {
                Some(bytes) => {
                    // 总是将字节数组编码为hexstring格式返回
                    let hexstring = encode_hexstring(&bytes);
                    Ok(format!("OK {}", hexstring))
                }
                None => Ok("EMPTY".into()),
            }
        }
        "STATE" => {
            let topic = parts
                .next()
                .ok_or_else(|| anyhow!("STATE requires a topic"))?;
            Ok(controller.topic_snapshot(topic)?)
        }
        "METRICS" => Ok(controller.get_metrics()?),
        _ => Err(anyhow!("unknown command")),
    }
}

async fn send_response(socket: &mut TcpStream, message: &str) -> Result<()> {
    let bytes = message.as_bytes();
    let len = bytes.len() as u32;
    socket.write_all(&len.to_le_bytes()).await?;
    socket.write_all(bytes).await?;
    Ok(())
}
