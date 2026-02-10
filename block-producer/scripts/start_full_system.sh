#!/bin/bash

# 一键启动完整的区块链系统
# 包括: Walrus 集群 + RPC Gateway + Block Producer

set -e

# 配置
PROJECT_ROOT="/opt/rust/project/avatar-walrus"
SCRIPTS_DIR="$PROJECT_ROOT/block-producer/scripts"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# 全局变量
PIDS=()

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

highlight() {
    echo -e "${PURPLE}[SYSTEM]${NC} $1"
}

# 检查依赖
check_dependencies() {
    info "检查系统依赖..."
    
    # 检查 Rust
    if ! command -v rustc >/dev/null 2>&1; then
        error "未找到 Rust 编译器"
        echo "请先安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # 检查基本工具
    for tool in curl jq lsof; do
        if ! command -v $tool >/dev/null 2>&1; then
            error "缺少必要工具: $tool"
            echo "请安装: sudo apt-get install $tool"
            exit 1
        fi
    done
    
    success "依赖检查通过"
}

# 检查端口占用
check_ports() {
    info "检查端口占用..."
    
    local ports=(8545 9091 9092 9093 6001 6002 6003)
    local occupied=()
    
    for port in "${ports[@]}"; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            occupied+=($port)
        fi
    done
    
    if [[ ${#occupied[@]} -gt 0 ]]; then
        warn "以下端口已被占用: ${occupied[*]}"
        echo "建议停止占用这些端口的进程，或修改配置"
        read -p "是否继续启动? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# 启动 Walrus 集群
start_walrus_cluster() {
    info "启动 Walrus 集群..."
    
    cd "$PROJECT_ROOT"
    
    # 使用现有的启动脚本
    if [[ -f "./scripts/start_walrus_cluster.sh" ]]; then
        ./scripts/start_walrus_cluster.sh start
    else
        error "找不到 Walrus 集群启动脚本"
        exit 1
    fi
    
    # 等待集群完全启动
    info "等待 Walrus 集群初始化..."
    sleep 5
    
    # 验证集群状态
    if ./scripts/start_walrus_cluster.sh status | grep -q "运行中"; then
        success "Walrus 集群启动成功"
    else
        error "Walrus 集群启动失败"
        exit 1
    fi
}

# 启动 RPC Gateway
start_rpc_gateway() {
    info "启动 RPC Gateway..."
    
    cd "$PROJECT_ROOT/rpc-gateway"
    
    # 编译（如果需要）
    if [[ ! -f "./target/release/rpc-gateway" ]]; then
        info "编译 RPC Gateway..."
        cargo build --release
    fi
    
    # 后台启动
    nohup ./target/release/rpc-gateway \
        --walrus-addr 127.0.0.1:9091 \
        --rpc-host 127.0.0.1 \
        --rpc-port 8545 \
        > "$PROJECT_ROOT/.logs/rpc-gateway.log" 2>&1 &
    
    local pid=$!
    PIDS+=("rpc-gateway:$pid")
    
    # 等待启动
    sleep 3
    
    # 验证启动
    if curl -s -X POST "http://127.0.0.1:8545" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}' \
        >/dev/null 2>&1; then
        success "RPC Gateway 启动成功 (PID: $pid)"
    else
        error "RPC Gateway 启动失败"
        exit 1
    fi
}

# 启动 Block Producer
start_block_producer() {
    info "启动 Block Producer..."
    
    cd "$PROJECT_ROOT/block-producer"
    
    # 创建日志目录
    mkdir -p "$PROJECT_ROOT/.logs"
    
    # 编译（如果需要）
    if [[ ! -f "./target/release/block-producer" ]]; then
        info "编译 Block Producer..."
        cargo build --release
    fi
    
    # 后台启动
    nohup ./target/release/block-producer \
        --walrus-addr 127.0.0.1:9091 \
        --topic blockchain-txs \
        --block-interval 5 \
        --max-txs-per-block 10000 \
        > "$PROJECT_ROOT/.logs/block-producer.log" 2>&1 &
    
    local pid=$!
    PIDS+=("block-producer:$pid")
    
    # 等待启动
    sleep 3
    
    # 检查进程是否存在
    if ps -p $pid >/dev/null 2>&1; then
        success "Block Producer 启动成功 (PID: $pid)"
    else
        error "Block Producer 启动失败"
        exit 1
    fi
}

# 保存 PID 信息
save_pids() {
    local pid_file="$PROJECT_ROOT/.system_pids"
    echo "# 系统进程 PID 文件 (自动生成)" > "$pid_file"
    echo "# 格式: 组件名:PID" >> "$pid_file"
    
    for pid_info in "${PIDS[@]}"; do
        echo "$pid_info" >> "$pid_file"
    done
    
    info "PID 信息已保存到: $pid_file"
}

# 显示系统状态
show_status() {
    echo ""
    highlight "=== 系统启动完成 ==="
    echo ""
    
    echo "运行的组件:"
    echo "  ✓ Walrus 集群 (3 节点)"
    echo "  ✓ RPC Gateway (端口 8545)"
    echo "  ✓ Block Producer (端口 9091)"
    echo ""
    
    echo "访问端点:"
    echo "  RPC Gateway: http://127.0.0.1:8545"
    echo "  Walrus Node 1: 127.0.0.1:9091"
    echo "  Walrus Node 2: 127.0.0.1:9092"
    echo "  Walrus Node 3: 127.0.0.1:9093"
    echo ""
    
    echo "数据目录:"
    echo "  Block Producer 数据: $PROJECT_ROOT/block-producer/data/"
    echo "  Walrus 数据: $PROJECT_ROOT/distributed-walrus/data/"
    echo "  系统日志: $PROJECT_ROOT/.logs/"
    echo ""
    
    echo "测试命令:"
    echo "  发送测试交易: $SCRIPTS_DIR/send_test_transaction.sh 5"
    echo "  监控区块生成: $SCRIPTS_DIR/monitor_blocks.sh"
    echo "  验证数据库: $SCRIPTS_DIR/verify_database.sh"
    echo ""
    
    success "系统已准备就绪！"
}

# 停止系统
stop_system() {
    info "停止系统..."
    
    local pid_file="$PROJECT_ROOT/.system_pids"
    
    if [[ -f "$pid_file" ]]; then
        while IFS=: read -r component pid; do
            if [[ -n "$component" && -n "$pid" && "$component" != "#"* ]]; then
                if kill -0 $pid 2>/dev/null; then
                    kill $pid
                    info "已停止 $component (PID: $pid)"
                fi
            fi
        done < "$pid_file"
        rm -f "$pid_file"
    fi
    
    # 停止 Walrus 集群
    cd "$PROJECT_ROOT"
    if [[ -f "./scripts/start_walrus_cluster.sh" ]]; then
        ./scripts/start_walrus_cluster.sh stop
    fi
    
    success "系统已停止"
}

# 信号处理
cleanup() {
    echo ""
    warn "收到中断信号，正在停止系统..."
    stop_system
    exit 0
}

trap cleanup INT TERM

# 主函数
main() {
    local action=${1:-start}
    
    case "$action" in
        start)
            echo "=== 启动完整区块链系统 ==="
            echo ""
            
            # 创建日志目录
            mkdir -p "$PROJECT_ROOT/.logs"
            
            # 执行启动流程
            check_dependencies
            check_ports
            start_walrus_cluster
            start_rpc_gateway
            start_block_producer
            save_pids
            show_status
            
            echo ""
            info "按 Ctrl+C 停止系统"
            
            # 保持运行
            while true; do
                sleep 10
                
                # 检查关键进程是否还在运行
                local alive_count=0
                for pid_info in "${PIDS[@]}"; do
                    local pid=$(echo "$pid_info" | cut -d: -f2)
                    if ps -p $pid >/dev/null 2>&1; then
                        alive_count=$((alive_count + 1))
                    fi
                done
                
                if [[ $alive_count -lt ${#PIDS[@]} ]]; then
                    warn "检测到部分组件已停止，系统可能不稳定"
                fi
            done
            ;;
            
        stop)
            stop_system
            ;;
            
        status)
            echo "=== 系统状态 ==="
            echo ""
            
            # 检查进程
            local pid_file="$PROJECT_ROOT/.system_pids"
            if [[ -f "$pid_file" ]]; then
                echo "运行中的组件:"
                while IFS=: read -r component pid; do
                    if [[ -n "$component" && -n "$pid" && "$component" != "#"* ]]; then
                        if ps -p $pid >/dev/null 2>&1; then
                            echo "  ✓ $component (PID: $pid) 运行中"
                        else
                            echo "  ✗ $component (PID: $pid) 已停止"
                        fi
                    fi
                done < "$pid_file"
            else
                echo "未找到系统 PID 文件"
            fi
            
            echo ""
            # 检查 Walrus 集群
            cd "$PROJECT_ROOT"
            if [[ -f "./scripts/start_walrus_cluster.sh" ]]; then
                echo "Walrus 集群状态:"
                ./scripts/start_walrus_cluster.sh status
            fi
            ;;
            
        *)
            echo "用法: $0 [start|stop|status]"
            echo ""
            echo "命令:"
            echo "  start   - 启动完整系统"
            echo "  stop    - 停止系统"
            echo "  status  - 查看系统状态"
            exit 1
            ;;
    esac
}

# 运行主函数
main "$@"