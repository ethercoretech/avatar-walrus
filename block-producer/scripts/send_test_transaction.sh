#!/bin/bash

# 发送测试交易脚本
# 用法: ./scripts/send_test_transaction.sh [数量]

set -e

RPC_URL="http://127.0.0.1:8545"
FROM_ADDRESS="0x742d35Cc6634C0532925a3b844Bc9e7595f09fBc"
TO_ADDRESS="0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
GAS_LIMIT="0x5208"  # 21000
GAS_PRICE="0x4a817c800"  # 20 Gwei

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

# 检查 RPC 服务是否可用
check_rpc() {
    if ! curl -s -X POST "$RPC_URL" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"health","params":[],"id":1}' \
        >/dev/null 2>&1; then
        error "无法连接到 RPC Gateway ($RPC_URL)"
        error "请确保 RPC Gateway 正在运行"
        exit 1
    fi
    success "RPC Gateway 连接正常"
}

# 发送单个交易
send_transaction() {
    local nonce=$1
    local value=${2:-"0xde0b6b3a7640000"}  # 默认 1 ETH
    
    local response=$(curl -s -X POST "$RPC_URL" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"method\": \"eth_sendTransaction\",
            \"params\": [{
                \"from\": \"$FROM_ADDRESS\",
                \"to\": \"$TO_ADDRESS\",
                \"value\": \"$value\",
                \"gas\": \"$GAS_LIMIT\",
                \"nonce\": \"0x$(printf '%x' $nonce)\"
            }],
            \"id\": $nonce
        }")
    
    # 提取交易哈希
    local tx_hash=$(echo "$response" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
    
    if [[ -n "$tx_hash" && "$tx_hash" != "null" ]]; then
        success "交易 #$nonce 发送成功: $tx_hash"
        echo "$tx_hash"
    else
        local error_msg=$(echo "$response" | grep -o '"message":"[^"]*"' | cut -d'"' -f4)
        error "交易 #$nonce 发送失败: $error_msg"
        echo "FAILED"
    fi
}

# 主函数
main() {
    local count=${1:-1}
    
    echo "=== 发送测试交易 ==="
    echo "RPC 地址: $RPC_URL"
    echo "发送者: $FROM_ADDRESS"
    echo "接收者: $TO_ADDRESS"
    echo "交易数量: $count"
    echo ""
    
    # 检查 RPC 连接
    check_rpc
    
    echo "开始发送 $count 笔交易..."
    echo ""
    
    local success_count=0
    local failed_count=0
    local tx_hashes=()
    
    for i in $(seq 1 $count); do
        local tx_hash=$(send_transaction $i)
        if [[ "$tx_hash" != "FAILED" ]]; then
            success_count=$((success_count + 1))
            tx_hashes+=("$tx_hash")
        else
            failed_count=$((failed_count + 1))
        fi
        
        # 小延迟避免过于频繁
        sleep 0.1
    done
    
    echo ""
    echo "=== 发送完成 ==="
    echo "成功: $success_count 笔"
    echo "失败: $failed_count 笔"
    echo "成功率: $((success_count * 100 / count))%"
    
    if [[ ${#tx_hashes[@]} -gt 0 ]]; then
        echo ""
        echo "交易哈希列表:"
        for hash in "${tx_hashes[@]}"; do
            echo "  - $hash"
        done
    fi
}

# 运行主函数
main "$@"