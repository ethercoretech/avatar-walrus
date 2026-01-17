#!/bin/bash
# 测试脚本：验证 rpc-gateway 是否正常工作

set -e

RPC_URL="http://127.0.0.1:8545"

echo "🧪 测试 RPC Gateway"
echo "================================"

# 1. 健康检查
echo "1️⃣ 测试健康检查..."
curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "health",
    "params": [],
    "id": 1
  }' | jq '.'

echo ""

# 2. 发送交易
echo "2️⃣ 测试发送交易..."
TX_HASH=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendTransaction",
    "params": [{
      "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
      "to": "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
      "value": "0xde0b6b3a7640000",
      "data": "0x",
      "gas": "0x5208",
      "gasPrice": "0x4a817c800",
      "nonce": "0x0"
    }],
    "id": 2
  }' | jq -r '.result')

echo "✅ 交易已提交，哈希: $TX_HASH"
echo ""

# 3. 发送原始交易
echo "3️⃣ 测试发送原始交易..."
RAW_TX_HASH=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0x01f87083aa36a781a384411335328252089483498fca79e0bc0548b4fc0744f467208c54132b872386f26fc1000080c001a0af9fe731ed7ddf89dbbc3966eba6492d2e434644fb246ef5e128f2021f8e0cbba053fef79bc9d54dc466906c41d552531a9f2c03d23e7e216fb2f4db21dddd9328"],
    "id": 3
  }' | jq -r '.result')

echo "✅ 原始交易已提交，哈希: $RAW_TX_HASH"
echo ""

echo "🎉 所有测试通过！"
echo ""
echo "💡 提示：可以使用 walrus-cli 查看写入的数据："
echo "   cargo run --bin walrus-cli -- --addr 127.0.0.1:9091"
echo "   然后执行: GET blockchain-txs"
