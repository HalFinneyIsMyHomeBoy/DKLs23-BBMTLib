#!/bin/bash
# Test script for HTTP-based Distributed DKG
# This script starts the HTTP relay server and then runs the distributed DKG test

set -e

echo "🔐 HTTP-based Distributed DKG Test Script"
echo "========================================="
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo is not installed"
    exit 1
fi

# Build the relay binary first
echo "🔨 Building HTTP relay server..."
cargo build --bin http_relay --release 2>&1 | grep -E "(Compiling|Finished|error)" || true

# Start the relay server in the background
echo "🚀 Starting HTTP relay server on 127.0.0.1:8080..."
cargo run --bin http_relay --release > /tmp/http_relay.log 2>&1 &
RELAY_PID=$!

# Wait for the relay to start
echo "⏳ Waiting for relay server to start..."
sleep 2

# Check if relay is running
if ! curl -s http://127.0.0.1:8080/health > /dev/null; then
    echo "❌ Error: Relay server failed to start"
    kill $RELAY_PID 2>/dev/null || true
    exit 1
fi

echo "✅ Relay server is running (PID: $RELAY_PID)"
echo ""

# Run the test
echo "🧪 Running HTTP DKG test..."
cargo run --example http_dkg_test --release

# Capture exit code
TEST_EXIT_CODE=$?

# Cleanup: kill the relay server
echo ""
echo "🧹 Cleaning up..."
kill $RELAY_PID 2>/dev/null || true
wait $RELAY_PID 2>/dev/null || true

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "✅ Test completed successfully!"
    echo ""
    echo "💾 Keyshare files saved:"
    ls -lh party_*_keyshare.json 2>/dev/null | awk '{print "   " $9 " (" $5 ")"}' || echo "   (No keyshare files found)"
    echo ""
    echo "💡 To clean up keyshare files, run: rm -f party_*_keyshare.json"
    exit 0
else
    echo "❌ Test failed with exit code $TEST_EXIT_CODE"
    echo "📋 Relay server logs:"
    tail -20 /tmp/http_relay.log
    exit $TEST_EXIT_CODE
fi

