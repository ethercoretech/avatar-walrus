#!/usr/bin/env bash

# Walrus 集群启动脚本
# 用法: ./start_walrus_cluster.sh [start|stop|restart|status|logs|clean]

set -e

# 配置
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISTRIBUTED_WALRUS_DIR="$PROJECT_ROOT/distributed-walrus"
PID_DIR="$PROJECT_ROOT/.walrus_pids"
LOG_DIR="$PROJECT_ROOT/.walrus_logs"

# 节点配置
NODE1_ID=1
NODE1_RAFT_PORT=6001
NODE1_CLIENT_PORT=9091

NODE2_ID=2
NODE2_RAFT_PORT=6002
NODE2_CLIENT_PORT=9092

NODE3_ID=3
NODE3_RAFT_PORT=6003
NODE3_CLIENT_PORT=9093

BOOTSTRAP_ADDR="127.0.0.1:$NODE1_RAFT_PORT"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
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

# 创建必要的目录
init_dirs() {
    mkdir -p "$PID_DIR"
    mkdir -p "$LOG_DIR"
}

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

# 启动单个节点
start_node() {
    local node_id=$1
    local raft_port=$2
    local client_port=$3
    local join_addr=$4
    
    local pid_file="$PID_DIR/node_${node_id}.pid"
    local log_file="$LOG_DIR/node_${node_id}.log"
    
    # 检查节点是否已经运行
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p $pid > /dev/null 2>&1; then
            warn "节点 $node_id 已经在运行 (PID: $pid)"
            return 0
        fi
    fi
    
    # 检查端口是否被占用
    if check_port $raft_port; then
        error "Raft 端口 $raft_port 已被占用"
        return 1
    fi
    if check_port $client_port; then
        error "客户端端口 $client_port 已被占用"
        return 1
    fi
    
    # 构建启动命令
    local cmd="cargo run --bin distributed-walrus -- \
        --node-id $node_id \
        --raft-port $raft_port \
        --client-port $client_port"
    
    if [ -n "$join_addr" ]; then
        cmd="$cmd --join $join_addr"
    fi
    
    # 启动节点
    info "启动节点 $node_id (Raft: $raft_port, Client: $client_port)..."
    cd "$DISTRIBUTED_WALRUS_DIR"
    
    # 在后台运行并记录 PID
    nohup $cmd > "$log_file" 2>&1 &
    local pid=$!
    echo $pid > "$pid_file"
    
    success "节点 $node_id 已启动 (PID: $pid)"
    info "日志文件: $log_file"
}

# 停止单个节点
stop_node() {
    local node_id=$1
    local pid_file="$PID_DIR/node_${node_id}.pid"
    
    if [ ! -f "$pid_file" ]; then
        warn "节点 $node_id 未运行"
        return 0
    fi
    
    local pid=$(cat "$pid_file")
    
    if ! ps -p $pid > /dev/null 2>&1; then
        warn "节点 $node_id 进程不存在 (PID: $pid)"
        rm -f "$pid_file"
        return 0
    fi
    
    info "停止节点 $node_id (PID: $pid)..."
    kill $pid
    
    # 等待进程结束
    local timeout=10
    local elapsed=0
    while ps -p $pid > /dev/null 2>&1 && [ $elapsed -lt $timeout ]; do
        sleep 1
        elapsed=$((elapsed + 1))
    done
    
    # 如果还没停止，强制杀死
    if ps -p $pid > /dev/null 2>&1; then
        warn "强制停止节点 $node_id..."
        kill -9 $pid
    fi
    
    rm -f "$pid_file"
    success "节点 $node_id 已停止"
}

# 获取节点状态
node_status() {
    local node_id=$1
    local raft_port=$2
    local client_port=$3
    local pid_file="$PID_DIR/node_${node_id}.pid"
    
    echo -n "节点 $node_id: "
    
    if [ ! -f "$pid_file" ]; then
        echo -e "${RED}未运行${NC}"
        return 1
    fi
    
    local pid=$(cat "$pid_file")
    
    if ! ps -p $pid > /dev/null 2>&1; then
        echo -e "${RED}已停止${NC} (进程不存在)"
        return 1
    fi
    
    if check_port $raft_port && check_port $client_port; then
        echo -e "${GREEN}运行中${NC} (PID: $pid, Raft: $raft_port, Client: $client_port)"
        return 0
    else
        echo -e "${YELLOW}启动中...${NC} (PID: $pid)"
        return 2
    fi
}

# 启动集群
start_cluster() {
    info "启动 Walrus 集群..."
    init_dirs
    
    # 启动节点 1 (引导节点)
    start_node $NODE1_ID $NODE1_RAFT_PORT $NODE1_CLIENT_PORT
    
    # 等待节点 1 的端口就绪
    if ! wait_for_port $NODE1_CLIENT_PORT 30; then
        error "节点 1 启动失败"
        return 1
    fi
    
    # 稍微等待以确保 Raft 完全初始化
    sleep 2
    
    # 启动节点 2
    start_node $NODE2_ID $NODE2_RAFT_PORT $NODE2_CLIENT_PORT $BOOTSTRAP_ADDR
    
    # 启动节点 3
    start_node $NODE3_ID $NODE3_RAFT_PORT $NODE3_CLIENT_PORT $BOOTSTRAP_ADDR
    
    # 等待所有节点就绪
    info "等待集群初始化..."
    wait_for_port $NODE2_CLIENT_PORT 30
    wait_for_port $NODE3_CLIENT_PORT 30
    
    echo ""
    success "Walrus 集群已启动！"
    echo ""
    info "客户端端口:"
    echo "  - 节点 1: 127.0.0.1:$NODE1_CLIENT_PORT"
    echo "  - 节点 2: 127.0.0.1:$NODE2_CLIENT_PORT"
    echo "  - 节点 3: 127.0.0.1:$NODE3_CLIENT_PORT"
    echo ""
    info "使用 CLI 连接:"
    echo "  cargo run --bin walrus-cli -- --addr 127.0.0.1:$NODE1_CLIENT_PORT"
    echo ""
    info "查看日志:"
    echo "  $0 logs [1|2|3]"
}

# 停止集群
stop_cluster() {
    info "停止 Walrus 集群..."
    
    stop_node $NODE3_ID
    stop_node $NODE2_ID
    stop_node $NODE1_ID
    
    success "Walrus 集群已停止"
}

# 重启集群
restart_cluster() {
    stop_cluster
    sleep 2
    start_cluster
}

# 显示集群状态
cluster_status() {
    echo "Walrus 集群状态:"
    echo "=================="
    node_status $NODE1_ID $NODE1_RAFT_PORT $NODE1_CLIENT_PORT
    node_status $NODE2_ID $NODE2_RAFT_PORT $NODE2_CLIENT_PORT
    node_status $NODE3_ID $NODE3_RAFT_PORT $NODE3_CLIENT_PORT
}

# 查看日志
show_logs() {
    local node_id=${1:-all}
    
    if [ "$node_id" = "all" ]; then
        info "显示所有节点日志 (Ctrl+C 退出)..."
        tail -f "$LOG_DIR"/node_*.log
    else
        local log_file="$LOG_DIR/node_${node_id}.log"
        if [ ! -f "$log_file" ]; then
            error "日志文件不存在: $log_file"
            return 1
        fi
        info "显示节点 $node_id 日志 (Ctrl+C 退出)..."
        tail -f "$log_file"
    fi
}

# 清理数据
clean_data() {
    warn "清理集群数据..."
    
    # 检查集群是否运行
    if [ -d "$PID_DIR" ] && [ "$(ls -A $PID_DIR 2>/dev/null)" ]; then
        error "请先停止集群再清理数据"
        return 1
    fi
    
    cd "$DISTRIBUTED_WALRUS_DIR"
    
    if [ -d "data" ]; then
        info "删除 data/ 目录..."
        rm -rf data/*
        success "数据已清理"
    else
        info "没有数据需要清理"
    fi
}

# 显示帮助
show_help() {
    cat << EOF
Walrus 集群管理脚本

用法: $0 <command>

命令:
  start       启动 3 节点集群
  stop        停止集群
  restart     重启集群
  status      显示集群状态
  logs [N]    查看日志 (N=1/2/3 查看指定节点, 不指定查看所有)
  clean       清理集群数据 (需要先停止集群)
  help        显示此帮助信息

示例:
  $0 start              # 启动集群
  $0 status             # 查看状态
  $0 logs 1             # 查看节点 1 日志
  $0 stop               # 停止集群
  $0 clean              # 清理数据

EOF
}

# 主函数
main() {
    local cmd=${1:-help}
    
    case "$cmd" in
        start)
            start_cluster
            ;;
        stop)
            stop_cluster
            ;;
        restart)
            restart_cluster
            ;;
        status)
            cluster_status
            ;;
        logs)
            show_logs ${2:-all}
            ;;
        clean)
            clean_data
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            error "未知命令: $cmd"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 运行主函数
main "$@"
