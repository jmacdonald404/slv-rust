#!/bin/bash

# Iterative debugging script for proxy packet issues
set -e

echo "=== Proxy Debug Test Script ==="
echo "Current branch: $(git branch --show-current)"
echo ""

# Function to test a branch
test_branch() {
    local branch_name="$1"
    echo "=== Testing branch: $branch_name ==="
    
    # Checkout the branch
    git checkout "$branch_name" 2>/dev/null || {
        echo "❌ Failed to checkout branch $branch_name"
        return 1
    }
    
    # Build the project
    echo "🔨 Building project..."
    cargo build --quiet || {
        echo "❌ Build failed on branch $branch_name"
        return 1
    }
    
    echo "✅ Build successful"
    
    # Create a simple test file to check for proxy packet sending
    echo "🧪 Creating proxy packet test..."
    
    # Run a simple grep to check if SOCKS5 send logging exists
    if cargo run --bin udp_test 2>&1 | head -20 | grep -i "socks5.*send\|send.*socks5" || true; then
        echo "✅ Found SOCKS5 send logs"
    else
        echo "⚠️ No SOCKS5 send logs found in first 20 lines"
    fi
    
    echo ""
}

# Test main branch first
test_branch "main"

# Test networking branch
test_branch "networking"

echo "=== Analysis Complete ==="
echo "Check the output above to compare proxy behavior between branches"