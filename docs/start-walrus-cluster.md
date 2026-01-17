# Walrus 集群启动

## 启动集群（3 节点）

### 终端 1：启动 Node 1（引导节点）

```bash
cd distributed-walrus

cargo run --bin distributed-walrus -- \
  --node-id 1 \
  --raft-port 6001 \
  --client-port 9091
```

### 终端 2：启动 Node 2

```bash
cd distributed-walrus

cargo run --bin distributed-walrus -- \
  --node-id 2 \
  --raft-port 6002 \
  --client-port 9092 \
  --join 127.0.0.1:6001
```

### 终端 3：启动 Node 3

```bash
cd distributed-walrus

cargo run --bin distributed-walrus -- \
  --node-id 3 \
  --raft-port 6003 \
  --client-port 9093 \
  --join 127.0.0.1:6001
```

## 使用 CLI 客户端

```bash
cd distributed-walrus

cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
```

### CLI 命令示例

```
🦭 > REGISTER my-topic
🦭 > PUT my-topic 0x48656c6c6f
🦭 > GET my-topic
🦭 > STATE my-topic
🦭 > METRICS
```

## 端口说明

| 节点 | Raft 端口 | 客户端端口 |
|------|----------|-----------|
| Node 1 | 6001 | 9091 |
| Node 2 | 6002 | 9092 |
| Node 3 | 6003 | 9093 |

## 连接到其他节点

```bash
# 连接到 Node 2
cargo run --bin walrus-cli -- --addr 127.0.0.1:9092

# 连接到 Node 3
cargo run --bin walrus-cli -- --addr 127.0.0.1:9093
```

## 停止集群

在每个终端按 `Ctrl+C`

## 清理数据

```bash
rm -rf distributed-walrus/data/
```
