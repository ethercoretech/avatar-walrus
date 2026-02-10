#!/bin/bash

# Block Producer 实时监控脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

highlight() {
    echo -e "${PURPLE}[MONITOR]${NC} $1"
}

# 检查 Block Producer 是否运行
check_block_producer() {
    if ! pgrep -f "block-producer" >/dev/null 2>&1; then
        warn "Block Producer 未运行"
        echo "请先启动 Block Producer:"
        echo "  cd block-producer && cargo run --release"
        exit 1
    fi
}

# 获取最新的区块信息
get_latest_block_info() {
    # 这里可以通过 RPC 或直接读取日志来获取信息
    # 当前实现通过监控日志文件
    
    local log_patterns=(
        "区块 #[0-9]+ 已生成"
        "✓ 执行完成.*成功.*失败"
        "状态根.*0x[0-9a-f]+"
        "Gas 使用.*[0-9]+"
        "交易数量.*[0-9]+"
    )
    
    echo "监控模式 - 实时显示 Block Producer 输出"
    echo "=========================================="
    echo ""
    
    # 监控日志输出
    if [[ -f "target/debug/block-producer.log" ]]; then
        tail -f target/debug/block-producer.log 2>/dev/null | while read line; do
            # 区块生成
            if echo "$line" | grep -q "区块 #[0-9]* 已生成"; then
                local block_num=$(echo "$line" | grep -o "#[0-9]*" | tr -d '#')
                highlight "新区块 #$block_num 生成"
            fi
            
            # 执行完成
            if echo "$line" | grep -q "✓ 执行完成"; then
                local success=$(echo "$line" | grep -o "[0-9]* 成功")
                local failed=$(echo "$line" | grep -o "[0-9]* 失败")
                echo "  执行结果: $success, $failed"
            fi
            
            # 状态根
            if echo "$line" | grep -q "状态根.*0x"; then
                local state_root=$(echo "$line" | grep -o "0x[0-9a-f]*")
                echo "  状态根: $state_root"
            fi
            
            # Gas 使用
            if echo "$line" | grep -q "Gas 使用"; then
                local gas=$(echo "$line" | grep -o "[0-9]*$")
                echo "  Gas 使用: $gas"
            fi
        done
    else
        # 如果没有日志文件，直接监控 stdout
        echo "请在 Block Producer 终端中查看实时输出"
        echo "关注以下关键词:"
        echo "  - 区块 #[数字] 已生成"
        echo "  - ✓ 执行完成"
        echo "  - 状态根: 0x..."
        echo "  - Gas 使用: [数字]"
    fi
}

# 显示统计信息
show_stats() {
    echo ""
    echo "=== 系统状态 ==="
    echo "Block Producer 进程: $(pgrep -f "block-producer" | wc -l) 个实例"
    echo "RPC Gateway 进程: $(pgrep -f "rpc-gateway" | wc -l) 个实例"
    echo "Walrus 节点进程: $(pgrep -f "distributed-walrus" | wc -l) 个实例"
    echo ""
    
    # 端口检查
    echo "=== 端口状态 ==="
    for port in 8545 9091 9092 9093; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            echo "端口 $port: ${GREEN}开放${NC}"
        else
            echo "端口 $port: ${RED}关闭${NC}"
        fi
    done
}

# 主函数
main() {
    echo "=== Block Producer 监控工具 ==="
    echo ""
    
    # 检查进程
    check_block_producer
    
    # 显示初始统计
    show_stats
    
    echo ""
    info "开始监控 Block Producer 输出..."
    echo "按 Ctrl+C 停止监控"
    echo ""
    
    # 开始监控
    get_latest_block_info
}

# 信号处理
trap 'echo ""; info "监控已停止"; exit 0' INT TERM

# 运行主函数
main "$@"