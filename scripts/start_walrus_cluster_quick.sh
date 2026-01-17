#!/usr/bin/env bash

# Walrus 集群快速启动脚本（简化版）
# 用于开发环境快速启动，日志输出到终端

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISTRIBUTED_WALRUS_DIR="$PROJECT_ROOT/distributed-walrus"

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🦭 启动 Walrus 集群...${NC}"
echo ""

cd "$DISTRIBUTED_WALRUS_DIR"

# 启动节点 1 (引导节点)
echo -e "${GREEN}[节点 1]${NC} 启动中... (Raft: 6001, Client: 9091)"
cargo run --bin distributed-walrus -- \
  --node-id 1 \
  --raft-port 6001 \
  --client-port 9091 &
NODE1_PID=$!

# 等待节点 1 启动
sleep 5

# 启动节点 2
echo -e "${GREEN}[节点 2]${NC} 启动中... (Raft: 6002, Client: 9092)"
cargo run --bin distributed-walrus -- \
  --node-id 2 \
  --raft-port 6002 \
  --client-port 9092 \
  --join 127.0.0.1:6001 &
NODE2_PID=$!

# 启动节点 3
echo -e "${GREEN}[节点 3]${NC} 启动中... (Raft: 6003, Client: 9093)"
cargo run --bin distributed-walrus -- \
  --node-id 3 \
  --raft-port 6003 \
  --client-port 9093 \
  --join 127.0.0.1:6001 &
NODE3_PID=$!

echo ""
echo -e "${BLUE}集群节点进程 ID:${NC}"
echo "  节点 1: $NODE1_PID"
echo "  节点 2: $NODE2_PID"
echo "  节点 3: $NODE3_PID"
echo ""
echo -e "${YELLOW}提示: 按 Ctrl+C 停止集群${NC}"
echo ""

# 清理函数
cleanup() {
    echo ""
    echo -e "${YELLOW}停止集群...${NC}"
    kill $NODE1_PID $NODE2_PID $NODE3_PID 2>/dev/null || true
    wait $NODE1_PID $NODE2_PID $NODE3_PID 2>/dev/null || true
    echo -e "${GREEN}集群已停止${NC}"
    exit 0
}

# 捕获 Ctrl+C
trap cleanup INT TERM

# 等待所有后台进程
wait
