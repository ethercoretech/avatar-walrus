#!/bin/bash

# 用法: ./scripts/send_test_transaction.sh [数量] [起始_nonce]

set -e

RPC_URL="http://127.0.0.1:8545"
# 使用内置钱包账户 (主账户 - 10000 ETH)
FROM_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TO_ADDRESS="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
GAS_LIMIT="0x5208"  # 21000
GAS_PRICE="0x4a817c800"  # 20 Gwei

# 合约部署配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_BYTECODE_FILE="$SCRIPT_DIR/contracts/MiniUSDT.json"
CONTRACT_GAS_LIMIT="0x1e8480"  # 2000000 gas for contract deployment

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

# 获取账户当前 nonce
get_current_nonce() {
    local address=$1
    local response=$(curl -s -X POST "$RPC_URL" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"method\": \"eth_getTransactionCount\",
            \"params\": [\"$address\", \"pending\"],
            \"id\": 1
        }")
    
    local nonce_hex=$(echo "$response" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
    if [[ -n "$nonce_hex" && "$nonce_hex" != "null" ]]; then
        echo $((16#${nonce_hex#0x}))
    else
        echo 0
    fi
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
                \"data\": \"0x\",
                \"gas\": \"$GAS_LIMIT\",
                \"gasPrice\": \"$GAS_PRICE\",
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

# 部署智能合约
deploy_contract() {
    local nonce=$1
    
    if [[ ! -f "$CONTRACT_BYTECODE_FILE" ]]; then
        warn "合约字节码文件不存在: $CONTRACT_BYTECODE_FILE"
        warn "请先运行: cd scripts/contracts && node compile.js"
        echo "FAILED"
        return
    fi
    
    local bytecode
    if command -v python3 >/dev/null 2>&1; then
        bytecode=$(python3 -c "import json; print(json.load(open('$CONTRACT_BYTECODE_FILE'))['bytecode'])" 2>/dev/null)
    elif command -v jq >/dev/null 2>&1; then
        bytecode=$(jq -r '.bytecode' "$CONTRACT_BYTECODE_FILE" 2>/dev/null)
    else
        bytecode=$(grep '"bytecode"' "$CONTRACT_BYTECODE_FILE" | sed 's/.*"bytecode"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
    fi
    
    if [[ -z "$bytecode" ]]; then
        error "无法从 $CONTRACT_BYTECODE_FILE 读取字节码"
        echo "FAILED"
        return
    fi
    
    info "部署 MiniUSDT 合约 (nonce: $nonce)..."
    
    local response=$(curl -s -X POST "$RPC_URL" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"method\": \"eth_sendTransaction\",
            \"params\": [{
                \"from\": \"$FROM_ADDRESS\",
                \"data\": \"$bytecode\",
                \"value\": \"0x0\",
                \"gas\": \"$CONTRACT_GAS_LIMIT\",
                \"gasPrice\": \"$GAS_PRICE\",
                \"nonce\": \"0x$(printf '%x' $nonce)\"
            }],
            \"id\": \"deploy_$nonce\"
        }")
    
    local tx_hash=$(echo "$response" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
    
    if [[ -n "$tx_hash" && "$tx_hash" != "null" ]]; then
        success "合约部署交易发送成功: $tx_hash"
        echo "$tx_hash"
    else
        local error_msg=$(echo "$response" | grep -o '"message":"[^"]*"' | cut -d'"' -f4)
        error "合约部署失败: $error_msg"
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
    
    # 检查 RPC 连接
    check_rpc
    
    # 自动获取当前 nonce (如果未手动指定)
    local start_nonce
    if [[ -n "$2" ]]; then
        start_nonce=$2
        echo "起始 nonce: $start_nonce (手动指定)"
    else
        start_nonce=$(get_current_nonce "$FROM_ADDRESS")
        echo "起始 nonce: $start_nonce (自动获取)"
    fi
    echo ""
    
    echo "开始发送 $count 笔交易..."
    echo ""
    
    local success_count=0
    local failed_count=0
    local deploy_success=0
    local deploy_failed=0
    local tx_hashes=()
    local deploy_hashes=()
    local current_nonce=$start_nonce
    
    for i in $(seq 0 $((count - 1))); do
        # 发送转账交易
        local tx_hash=$(send_transaction $current_nonce)
        if [[ "$tx_hash" != "FAILED" ]]; then
            success_count=$((success_count + 1))
            tx_hashes+=("$tx_hash")
            current_nonce=$((current_nonce + 1))
        else
            failed_count=$((failed_count + 1))
        fi
        
        # 部署合约
        local deploy_hash=$(deploy_contract $current_nonce)
        if [[ "$deploy_hash" != "FAILED" ]]; then
            deploy_success=$((deploy_success + 1))
            deploy_hashes+=("$deploy_hash")
            current_nonce=$((current_nonce + 1))
        else
            deploy_failed=$((deploy_failed + 1))
        fi
        
        # 小延迟避免过于频繁
        sleep 0.1
    done
    
    echo ""
    echo "=== 发送完成 ==="
    echo "转账交易 - 成功: $success_count 笔, 失败: $failed_count 笔"
    echo "合约部署 - 成功: $deploy_success 笔, 失败: $deploy_failed 笔"
    local total=$((success_count + deploy_success))
    local total_tx=$((count + count))
    echo "总成功率: $((total * 100 / total_tx))%"
    
    if [[ ${#tx_hashes[@]} -gt 0 ]]; then
        echo ""
        echo "转账交易哈希:"
        for hash in "${tx_hashes[@]}"; do
            echo "  - $hash"
        done
    fi
    
    if [[ ${#deploy_hashes[@]} -gt 0 ]]; then
        echo ""
        echo "合约部署交易哈希:"
        for hash in "${deploy_hashes[@]}"; do
            echo "  - $hash"
        done
    fi
}

# 运行主函数
main "$@"