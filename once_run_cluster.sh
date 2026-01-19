#!/usr/bin/env bash

# Walrus 集群一次性运行脚本
# 运行 walrus 三端口节点集群和 rpc-gateway 单端口节点

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DISTRIBUTED_WALRUS_DIR="$PROJECT_ROOT/distributed-walrus"
RPC_GATEWAY_DIR="$PROJECT_ROOT/rpc-gateway"
DATA_DIR="$DISTRIBUTED_WALRUS_DIR/data"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 清理函数
cleanup() {
    echo ""
    warn "正在停止集群并清理..."
    
    # 杀死所有 walrus 节点进程
    info "停止 Walrus 节点进程..."
    pkill -f "distributed-walrus.*--node-id" 2>/dev/null || true
    
    # 杀死 rpc-gateway 进程
    info "停止 RPC Gateway 进程..."
    pkill -f "rpc-gateway" 2>/dev/null || true
    
    # 等待进程结束
    sleep 2
    
    # 清理数据目录
    if [ -d "$DATA_DIR" ]; then
        info "清理集群数据目录: $DATA_DIR"
        rm -rf "$DATA_DIR"
        success "数据目录已清理"
    fi
    
    success "集群已停止并清理完成"
    exit 0
}

# 捕获中断信号
trap cleanup INT TERM

# 检查端口是否被占用
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# 等待端口可用
wait_for_port() {
    local port=$1
    local timeout=${2:-30}
    local elapsed=0
    
    info "等待端口 $port 启动..."
    while [ $elapsed -lt $timeout ]; do
        if check_port $port; then
            success "端口 $port 已就绪"
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
    
    error "等待端口 $port 超时"
    return 1
}

echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}  启动 Walrus 集群 + RPC Gateway${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

# 清理旧数据
if [ -d "$DATA_DIR" ]; then
    info "清理旧数据目录..."
    rm -rf "$DATA_DIR"
fi

# 启动 Walrus 节点 1 (引导节点)
info "启动 Walrus 节点 1 (Raft: 6001, Client: 9091)..."
cd "$DISTRIBUTED_WALRUS_DIR"
cargo run --bin distributed-walrus -- \
    --node-id 1 \
    --raft-port 6001 \
    --client-port 9091 &
NODE1_PID=$!

# 等待节点 1 启动
if ! wait_for_port 9091 30; then
    error "节点 1 启动失败"
    cleanup
fi
sleep 2

# 启动 Walrus 节点 2
info "启动 Walrus 节点 2 (Raft: 6002, Client: 9092)..."
cargo run --bin distributed-walrus -- \
    --node-id 2 \
    --raft-port 6002 \
    --client-port 9092 \
    --join 127.0.0.1:6001 &
NODE2_PID=$!

# 启动 Walrus 节点 3
info "启动 Walrus 节点 3 (Raft: 6003, Client: 9093)..."
cargo run --bin distributed-walrus -- \
    --node-id 3 \
    --raft-port 6003 \
    --client-port 9093 \
    --join 127.0.0.1:6001 &
NODE3_PID=$!

# 等待所有节点就绪
wait_for_port 9092 30
wait_for_port 9093 30

echo ""
success "Walrus 集群已启动！"
echo ""
info "Walrus 节点信息:"
echo "  - 节点 1: 127.0.0.1:9091 (Raft: 6001)"
echo "  - 节点 2: 127.0.0.1:9092 (Raft: 6002)"
echo "  - 节点 3: 127.0.0.1:9093 (Raft: 6003)"
echo ""

# 启动 RPC Gateway
info "启动 RPC Gateway (端口: 8545)..."
cd "$RPC_GATEWAY_DIR"
# RUST_LOG=INFO
RUST_LOG=INFO cargo run --bin rpc-gateway -- \
    --walrus-addr 127.0.0.1:9091 \
    --rpc-port 8545 &
RPC_PID=$!

# 等待 RPC Gateway 就绪
if ! wait_for_port 8545 30; then
    error "RPC Gateway 启动失败"
    cleanup
fi

echo ""
success "RPC Gateway 已启动！"
echo ""
info "RPC Gateway 信息:"
echo "  - 地址: http://127.0.0.1:8545"
echo "  - 连接到 Walrus: 127.0.0.1:9091"
echo ""

# 显示使用提示
echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}  使用提示${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

echo -e "${GREEN}1. 连接 Walrus CLI 查看交易:${NC}"
echo "   cd distributed-walrus/"
echo "   cargo run --bin walrus-cli -- --addr 127.0.0.1:9091"
echo "   然后执行: GET blockchain-txs"
echo ""

echo -e "${GREEN}2. 查看 Walrus 日志:${NC}"
echo "   查看所有节点日志:"
echo "     tail -f $PROJECT_ROOT/.walrus_logs/node_*.log"
echo "   查看指定节点日志 (例如节点 1):"
echo "     tail -f $PROJECT_ROOT/.walrus_logs/node_1.log"
echo ""

echo -e "${GREEN}3. 使用 curl 发送交易:${NC}"
echo "   发送普通交易:"
echo '   curl -X POST http://127.0.0.1:8545 \'
echo '     -H "Content-Type: application/json" \'
echo '     -d '"'"'{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{"from":"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb","to":"0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed","value":"0xde0b6b3a7640000","data":"0x","gas":"0x5208","gasPrice":"0x4a817c800","nonce":"0x0"}],"id":1}'"'"
echo ""
echo "   发送裸交易:"
echo '   curl -X POST http://127.0.0.1:8545 \'
echo '     -H "Content-Type: application/json" \'
echo '     -d '"'"'{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0x01f87083aa36a781a384411335328252089483498fca79e0bc0548b4fc0744f467208c54132b872386f26fc1000080c001a0af9fe731ed7ddf89dbbc3966eba6492d2e434644fb246ef5e128f2021f8e0cbba053fef79bc9d54dc466906c41d552531a9f2c03d23e7e216fb2f4db21dddd9328"],"id":1}'"'"
echo ""

echo -e "${YELLOW}提示: 按 Ctrl+C 停止集群并清理数据${NC}"
echo ""

# 等待所有后台进程
wait
