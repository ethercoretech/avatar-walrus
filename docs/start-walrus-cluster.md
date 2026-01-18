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


# Walrus 集群启动脚本说明

本目录提供了两个脚本来启动和管理 Walrus 分布式集群。

## 📋 脚本概览

### 1. `start_walrus_cluster.sh` - 生产级管理脚本

**功能齐全的集群管理工具，适合开发和测试环境。**

#### 特性
- ✅ 后台运行，日志输出到文件
- ✅ PID 管理和进程监控
- ✅ 优雅启动/停止（自动等待端口就绪）
- ✅ 实时状态查看
- ✅ 日志查看（支持单节点或全部）
- ✅ 数据清理功能
- ✅ 端口占用检测
- ✅ 彩色输出

#### 使用方法

```bash
# 启动集群
./scripts/start_walrus_cluster.sh start

# 查看状态
./scripts/start_walrus_cluster.sh status

# 查看日志
./scripts/start_walrus_cluster.sh logs       # 所有节点
./scripts/start_walrus_cluster.sh logs 1     # 仅节点 1
./scripts/start_walrus_cluster.sh logs 2     # 仅节点 2
./scripts/start_walrus_cluster.sh logs 3     # 仅节点 3

# 停止集群
./scripts/start_walrus_cluster.sh stop

# 重启集群
./scripts/start_walrus_cluster.sh restart

# 清理数据（需先停止集群）
./scripts/start_walrus_cluster.sh clean

# 显示帮助
./scripts/start_walrus_cluster.sh help
```

#### 输出示例

启动集群：
```
[INFO] 启动 Walrus 集群...
[INFO] 启动节点 1 (Raft: 6001, Client: 9091)...
[SUCCESS] 节点 1 已启动 (PID: 12345)
[INFO] 等待端口 9091 启动...
[SUCCESS] 端口 9091 已就绪
...
[SUCCESS] Walrus 集群已启动！

[INFO] 客户端端口:
  - 节点 1: 127.0.0.1:9091
  - 节点 2: 127.0.0.1:9092
  - 节点 3: 127.0.0.1:9093

[INFO] 使用 CLI 连接:
  cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
```

查看状态：
```
Walrus 集群状态:
==================
节点 1: 运行中 (PID: 12345, Raft: 6001, Client: 9091)
节点 2: 运行中 (PID: 12346, Raft: 6002, Client: 9092)
节点 3: 运行中 (PID: 12347, Raft: 6003, Client: 9093)
```

#### 生成的文件

```
avatar-walrus/
├── .walrus_pids/          # PID 文件（自动生成，已加入 .gitignore）
│   ├── node_1.pid
│   ├── node_2.pid
│   └── node_3.pid
└── .walrus_logs/          # 日志文件（自动生成，已加入 .gitignore）
    ├── node_1.log
    ├── node_2.log
    └── node_3.log
```

---

### 2. `quick_start.sh` - 快速启动脚本

**简化版启动脚本，适合快速测试和调试。**

#### 特性
- ✅ 前台运行，日志直接输出到终端
- ✅ 快速启动，无需等待
- ✅ Ctrl+C 一键停止所有节点
- ✅ 适合开发调试

#### 使用方法

```bash
# 启动集群（前台运行）
./scripts/quick_start.sh

# 按 Ctrl+C 停止集群
```

#### 输出示例

```
🦭 启动 Walrus 集群...

[节点 1] 启动中... (Raft: 6001, Client: 9091)
[节点 2] 启动中... (Raft: 6002, Client: 9092)
[节点 3] 启动中... (Raft: 6003, Client: 9093)

集群节点进程 ID:
  节点 1: 12345
  节点 2: 12346
  节点 3: 12347

提示: 按 Ctrl+C 停止集群

[节点日志实时输出到终端...]
```

---

## 🆚 脚本对比

| 功能 | start_walrus_cluster.sh | quick_start.sh |
|------|-------------------------|----------------|
| 后台运行 | ✅ | ❌ |
| 日志文件 | ✅ | ❌ |
| 状态查看 | ✅ | ❌ |
| 优雅停止 | ✅ | ✅ (Ctrl+C) |
| 进程管理 | ✅ | 基础 |
| 端口检测 | ✅ | ❌ |
| 数据清理 | ✅ | ❌ |
| 适用场景 | 开发/测试 | 快速调试 |

---

## 📡 集群配置

### 节点配置

| 节点 | Node ID | Raft 端口 | 客户端端口 |
|------|---------|-----------|-----------|
| 节点 1 | 1 | 6001 | 9091 |
| 节点 2 | 2 | 6002 | 9092 |
| 节点 3 | 3 | 6003 | 9093 |

### 连接集群

```bash
# 使用 walrus-cli 连接任意节点
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091  # 节点 1
cargo run --bin walrus-cli -- --addr 127.0.0.1:9092  # 节点 2
cargo run --bin walrus-cli -- --addr 127.0.0.1:9093  # 节点 3

# 或者使用 TCP 客户端（任何语言）
# 协议: [4 bytes length][UTF-8 command]
```

---

## 🔧 故障排查

### 端口被占用

```bash
# 查看端口占用情况
lsof -i :9091
lsof -i :6001

# 或者使用脚本自动检测
./scripts/start_walrus_cluster.sh status
```

### 节点无法启动

1. 检查日志文件：
   ```bash
   ./scripts/start_walrus_cluster.sh logs 1
   ```

2. 确保没有旧进程残留：
   ```bash
   ./scripts/start_walrus_cluster.sh stop
   ```

3. 清理数据后重试：
   ```bash
   ./scripts/start_walrus_cluster.sh clean
   ./scripts/start_walrus_cluster.sh start
   ```

### 集群无法连接

1. 确认所有节点运行正常：
   ```bash
   ./scripts/start_walrus_cluster.sh status
   ```

2. 等待集群完全初始化（通常需要 5-10 秒）

3. 检查网络连接：
   ```bash
   nc -zv 127.0.0.1 9091
   nc -zv 127.0.0.1 9092
   nc -zv 127.0.0.1 9093
   ```

---

## 🎯 推荐使用场景

### 开发测试
使用 `start_walrus_cluster.sh`：
- 在后台运行集群
- 可以随时查看状态和日志
- 开发其他组件时（如 rpc-gateway、block-producer）需要稳定的后台集群

### 快速验证
使用 `quick_start.sh`：
- 快速测试 Walrus 功能
- 调试集群问题
- 查看实时日志输出

### 生产环境
推荐使用 Docker Compose：
```bash
cd distributed-walrus
make cluster-up
```

---

## 📚 相关文档

- [分布式 Walrus 架构](../distributed-walrus/README.md)
- [区块链整体设计](../docs/blockchain-design.md)
- [Walrus 集群快速启动指南](../docs/start-walrus-cluster.md)
- [CLI 使用指南](../distributed-walrus/docs/cli.md)

---

## 💡 提示

1. **首次启动可能较慢**：因为需要编译 Rust 项目
2. **数据持久化**：集群数据存储在 `distributed-walrus/data/` 目录
3. **日志级别**：通过 `RUST_LOG` 环境变量控制，例如：
   ```bash
   RUST_LOG=debug ./scripts/start_walrus_cluster.sh start
   ```
4. **性能测试**：使用 release 模式编译以获得最佳性能：
   ```bash
   # 修改脚本中的 cargo run 为 cargo run --release
   ```

---

**祝你使用愉快！🦭**
