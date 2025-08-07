#!/bin/bash

# Quick test script to compare proxy behavior between branches
echo "=== Proxy Behavior Comparison Test ==="
echo ""

# Function to count SOCKS5 send operations in recent logs
count_socks5_sends() {
    grep -c "SOCKS5.*send\|send.*SOCKS5" /Users/dextro/RubymineProjects/slv-rust/log.txt 2>/dev/null || echo "0"
}

# Function to check if connection is still alive
check_connection_alive() {
    if grep -q "Dropping SOCKS5 client" /Users/dextro/RubymineProjects/slv-rust/log.txt 2>/dev/null; then
        echo "Connection DROPPED"
    else
        echo "Connection ALIVE"
    fi
}

echo "Current branch: $(git branch --show-current)"
echo "SOCKS5 send operations in log: $(count_socks5_sends)"
echo "Connection status: $(check_connection_alive)"
echo ""

# Show recent SOCKS5 related log entries
echo "Recent SOCKS5 activity:"
tail -20 /Users/dextro/RubymineProjects/slv-rust/log.txt | grep -i socks5 || echo "No recent SOCKS5 activity found"
echo ""

# Quick fix test
echo "=== Suggested Fix ==="
echo "1. The SOCKS5 client in networking branch is being dropped after first use"
echo "2. Main branch keeps the connection alive for multiple packets"
echo "3. Solution: Add connection persistence or borrow main branch implementation"