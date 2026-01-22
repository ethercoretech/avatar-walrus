#!/bin/bash
# RPC Gateway 统一测试脚本
# 
# 功能：
# - 健康检查
# - eth_sendTransaction 测试
# - eth_sendRawTransaction 测试  
# - Prometheus metrics 测试
# - 可选的性能压测（需要 hey 工具）
#
# 使用方法：
#   ./test_rpc.sh                          # 基础测试
#   RPC_URL=http://host:port ./test_rpc.sh # 自定义地址
#   ./test_rpc.sh --perf                   # 包含性能测试

set -e

RPC_URL="${RPC_URL:-http://127.0.0.1:8545}"
METRICS_URL="${METRICS_URL:-http://127.0.0.1:8546/metrics}"
WALRUS_ADDR="${WALRUS_ADDR:-127.0.0.1:9091}"

# 检查是否需要性能测试
RUN_PERF=false
if [[ "$1" == "--perf" ]] || [[ "$1" == "-p" ]]; then
  RUN_PERF=true
fi

echo "=========================================="
echo "🧪 RPC Gateway 测试脚本"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检查 jq 是否可用（可选）
USE_JQ=false
if command -v jq &> /dev/null; then
  USE_JQ=true
fi

# 工具函数：格式化 JSON 输出
format_json() {
  if [ "$USE_JQ" = true ]; then
    echo "$1" | jq '.'
  else
    echo "$1"
  fi
}

# 测试健康检查
echo -e "${YELLOW}[1/5] 测试健康检查...${NC}"
HEALTH_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"health","params":[],"id":1}' \
  "$RPC_URL")

if echo "$HEALTH_RESPONSE" | grep -q "OK"; then
  echo -e "${GREEN}✓ 健康检查通过${NC}"
  format_json "$HEALTH_RESPONSE"
else
  echo -e "${RED}✗ 健康检查失败${NC}"
  format_json "$HEALTH_RESPONSE"
  exit 1
fi
echo ""

# 测试 eth_sendRawTransaction
echo -e "${YELLOW}[2/5] 测试 eth_sendRawTransaction...${NC}"
RAW_TX_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0x01f87083aa36a781a384411335328252089483498fca79e0bc0548b4fc0744f467208c54132b872386f26fc1000080c001a0af9fe731ed7ddf89dbbc3966eba6492d2e434644fb246ef5e128f2021f8e0cbba053fef79bc9d54dc466906c41d552531a9f2c03d23e7e216fb2f4db21dddd9328"],"id":2}' \
  "$RPC_URL")

if echo "$RAW_TX_RESPONSE" | grep -q "0x"; then
  echo -e "${GREEN}✓ eth_sendRawTransaction 测试通过${NC}"
  if [ "$USE_JQ" = true ]; then
    TX_HASH=$(echo "$RAW_TX_RESPONSE" | jq -r '.result')
  else
    TX_HASH=$(echo "$RAW_TX_RESPONSE" | grep -o '"result":"0x[^"]*"' | cut -d'"' -f4)
  fi
  echo -e "交易哈希: ${BLUE}$TX_HASH${NC}"
else
  echo -e "${RED}✗ eth_sendRawTransaction 测试失败${NC}"
  format_json "$RAW_TX_RESPONSE"
  exit 1
fi
echo ""

# 测试 eth_sendTransaction
echo -e "${YELLOW}[3/5] 测试 eth_sendTransaction...${NC}"
TX_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{"from":"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb","to":"0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed","value":"0xde0b6b3a7640000","data":"0x","gas":"0x5208","gasPrice":"0x4a817c800","nonce":"0x0"}],"id":3}' \
  "$RPC_URL")

if echo "$TX_RESPONSE" | grep -q "0x"; then
  echo -e "${GREEN}✓ eth_sendTransaction 测试通过${NC}"
  if [ "$USE_JQ" = true ]; then
    TX_HASH=$(echo "$TX_RESPONSE" | jq -r '.result')
  else
    TX_HASH=$(echo "$TX_RESPONSE" | grep -o '"result":"0x[^"]*"' | cut -d'"' -f4)
  fi
  echo -e "交易哈希: ${BLUE}$TX_HASH${NC}"
else
  echo -e "${RED}✗ eth_sendTransaction 测试失败${NC}"
  format_json "$TX_RESPONSE"
  exit 1
fi
echo ""

# 测试 Prometheus metrics
echo -e "${YELLOW}[4/5] 测试 Prometheus metrics...${NC}"
METRICS_RESPONSE=$(curl -s "$METRICS_URL")

if echo "$METRICS_RESPONSE" | grep -q "rpc_gateway_transactions_total"; then
  echo -e "${GREEN}✓ Prometheus metrics 正常${NC}"
  echo ""
  echo "📊 关键指标预览:"
  echo "$METRICS_RESPONSE" | grep "rpc_gateway_transactions_total{" | head -2
  echo "$METRICS_RESPONSE" | grep "rpc_gateway_transaction_duration" | head -2
else
  echo -e "${RED}✗ Prometheus metrics 异常${NC}"
  echo "响应前 100 字符: ${METRICS_RESPONSE:0:100}"
  exit 1
fi
echo ""

# 基本连接检查（新增）
echo -e "${YELLOW}[5/5] 检查服务连接...${NC}"
if timeout 2 bash -c "echo > /dev/tcp/${WALRUS_ADDR%:*}/${WALRUS_ADDR#*:}" 2>/dev/null; then
  echo -e "${GREEN}✓ Walrus 服务器连接正常 (${WALRUS_ADDR})${NC}"
else
  echo -e "${YELLOW}⚠ Walrus 服务器连接失败 (${WALRUS_ADDR})${NC}"
  echo -e "${YELLOW}  这可能影响交易写入功能${NC}"
fi
echo ""

# 性能测试（可选）
if [ "$RUN_PERF" = true ]; then
  if command -v hey &> /dev/null; then
    echo -e "${YELLOW}[性能测试] 使用 hey 进行压力测试...${NC}"
    echo "发送 1000 个请求，100 并发..."
    hey -n 1000 -c 100 -m POST \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0xf86c0185012a05f2008252089400000000000000000000000000000000000000008080820a95"],"id":1}' \
      "$RPC_URL"
  else
    echo -e "${RED}✗ 性能测试需要 'hey' 工具${NC}"
    echo "安装命令: go install github.com/rakyll/hey@latest"
    exit 1
  fi
  echo ""
fi

echo "=========================================="
echo -e "${GREEN}🎉 所有测试通过！✓${NC}"
echo "=========================================="
echo ""
echo -e "${BLUE}服务信息:${NC}"
echo "  RPC 端点:     $RPC_URL"
echo "  Metrics 端点: $METRICS_URL"
echo "  Walrus 地址:  $WALRUS_ADDR"
echo ""
echo -e "${BLUE}💡 实用命令:${NC}"
echo ""
echo "  查看实时指标:"
echo "    curl $METRICS_URL"
echo ""
echo "  使用 walrus-cli 查看数据:"
echo "    cargo run --bin walrus-cli -- --addr $WALRUS_ADDR"
echo "    然后执行: GET blockchain-txs"
echo ""
echo "  运行性能测试:"
echo "    ./test_rpc.sh --perf"
echo ""
if [ "$USE_JQ" = false ]; then
  echo -e "${YELLOW}  提示: 安装 jq 可以获得更好的 JSON 输出格式${NC}"
  echo "    brew install jq  (macOS)"
  echo "    apt install jq   (Ubuntu/Debian)"
  echo ""
fi
