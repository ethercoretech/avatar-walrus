#!/bin/bash

# 验证 Block Producer 数据库状态

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查数据库文件
check_database_files() {
    local data_dir="./data"
    
    echo "=== 数据库文件检查 ==="
    
    if [[ ! -d "$data_dir" ]]; then
        warn "数据目录不存在: $data_dir"
        mkdir -p "$data_dir"
        info "已创建数据目录"
    fi
    
    # 查找 Redb 文件
    local redb_files=$(find "$data_dir" -name "*.redb" 2>/dev/null || echo "")
    
    if [[ -z "$redb_files" ]]; then
        warn "未找到 Redb 数据库文件"
        echo "数据库将在第一次区块生成时自动创建"
        return 1
    fi
    
    echo "找到数据库文件:"
    for file in $redb_files; do
        local size=$(du -h "$file" 2>/dev/null | cut -f1)
        local modified=$(stat -c %y "$file" 2>/dev/null | cut -d' ' -f1)
        echo "  - $(basename "$file") (大小: $size, 修改时间: $modified)"
    done
    
    return 0
}

# 检查数据库内容 (通过 Block Producer API)
check_database_content() {
    echo ""
    echo "=== 数据库内容检查 ==="
    
    # 尝试通过 RPC 查询最新区块
    local latest_block=$(curl -s -X POST "http://127.0.0.1:8545" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        2>/dev/null | jq -r '.result' 2>/dev/null || echo "null")
    
    if [[ "$latest_block" != "null" && -n "$latest_block" ]]; then
        local block_num=$((16#${latest_block#0x}))
        success "最新区块号: $block_num (0x$latest_block)"
    else
        warn "无法获取最新区块信息"
        echo "可能原因:"
        echo "  1. Block Producer 尚未生成任何区块"
        echo "  2. RPC 接口未实现 eth_blockNumber 方法"
    fi
    
    # 检查账户余额 (如果支持)
    local test_account="0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc"
    echo ""
    echo "测试账户: $test_account"
    # 这里可以根据实际支持的 RPC 方法来查询余额
}

# 检查 Walrus 存储
check_walrus_storage() {
    echo ""
    echo "=== Walrus 存储检查 ==="
    
    # 检查 Walrus CLI 是否可用
    if ! command -v walrus-cli >/dev/null 2>&1; then
        warn "Walrus CLI 未安装或不在 PATH 中"
        echo "跳过 Walrus 存储检查"
        return
    fi
    
    # 检查 blockchain-txs topic
    echo "检查 blockchain-txs topic..."
    local topic_state=$(timeout 5 walrus-cli --addr 127.0.0.1:9091 STATE blockchain-txs 2>&1 || echo "error")
    
    if echo "$topic_state" | grep -q "ERR"; then
        warn "无法访问 blockchain-txs topic"
        echo "可能需要手动创建: REGISTER blockchain-txs"
    elif echo "$topic_state" | grep -q "entries"; then
        local entries=$(echo "$topic_state" | grep -o '"entries":[0-9]*' | cut -d':' -f2)
        success "Topic blockchain-txs 包含 $entries 条记录"
    else
        info "Topic blockchain-txs 状态正常"
    fi
}

# 性能测试
run_performance_test() {
    echo ""
    echo "=== 性能测试 ==="
    
    # 检查系统资源
    echo "系统资源使用:"
    echo "  CPU: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)%"
    echo "  内存: $(free -h | awk '/^Mem:/ {print $3 "/" $2}')"
    echo "  磁盘: $(df -h . | awk 'NR==2 {print $5 " used on " $6}')"
    
    # 检查进程状态
    echo ""
    echo "关键进程状态:"
    for process in "block-producer" "rpc-gateway" "distributed-walrus"; do
        local count=$(pgrep -f "$process" | wc -l)
        if [[ $count -gt 0 ]]; then
            success "$process: $count 个进程运行中"
        else
            error "$process: 未运行"
        fi
    done
}

# 主函数
main() {
    echo "=== Block Producer 数据库验证工具 ==="
    echo ""
    
    # 检查工作目录
    if [[ ! -f "./Cargo.toml" ]] || [[ ! -d "./src" ]]; then
        error "请在 block-producer 目录下运行此脚本"
        exit 1
    fi
    
    # 执行各项检查
    check_database_files
    check_database_content
    check_walrus_storage
    run_performance_test
    
    echo ""
    success "数据库验证完成"
    
    # 提供下一步建议
    echo ""
    echo "=== 建议 ==="
    if [[ $? -eq 0 ]]; then
        echo "✓ 数据库状态正常"
        echo "✓ 可以开始发送交易进行测试"
        echo ""
        echo "快速测试命令:"
        echo "  ./scripts/send_test_transaction.sh 5"
    else
        echo "! 需要解决上述问题后再进行测试"
    fi
}

# 运行主函数
main "$@"