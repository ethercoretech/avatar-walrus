wrk.method = "POST"
wrk.body   = '{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{"from":"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb","to":"0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed","value":"0xde0b6b3a7640000","data":"0x","gas":"0x5208","gasPrice":"0x4a817c800","nonce":"0x0"}],"id":1}'
wrk.headers["Content-Type"] = "application/json"
