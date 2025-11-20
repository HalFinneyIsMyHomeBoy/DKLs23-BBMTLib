#!/bin/bash

# Script to generate nsec/npub pairs and save DKLs23 keyshares
# Usage: ./generate_keyshares.sh <threshold> <share_count>

set -e

THRESHOLD=${1:-2}
SHARE_COUNT=${2:-3}
CLI_BIN="./target/release/dkls23-cli"
KEYGEN_BIN="./target/release/generate_nostr_keys"
OUTPUT_DIR="keyshares"

# Check if CLI binary exists
if [ ! -f "$CLI_BIN" ]; then
    echo "Error: CLI binary not found at $CLI_BIN"
    echo "Please build it first: cargo build --release --bin dkls23-cli"
    exit 1
fi

# Check if key generator binary exists
if [ ! -f "$KEYGEN_BIN" ]; then
    echo "Error: Key generator binary not found at $KEYGEN_BIN"
    echo "Please build it first: cargo build --release --bin generate_nostr_keys"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "🔐 Generating $SHARE_COUNT nsec/npub pairs..."
echo "📋 Threshold: $THRESHOLD, Share count: $SHARE_COUNT"
echo ""

# Generate nsec/npub pairs using Rust binary
NPUBS=()
NSECS=()

for i in $(seq 1 $SHARE_COUNT); do
    echo "Generating keys for party $i..."
    
    # Generate Nostr key pair using Rust binary
    KEY_JSON=$($KEYGEN_BIN 2>/dev/null)
    
    if [ $? -ne 0 ]; then
        echo "Error: Failed to generate keys"
        exit 1
    fi
    
    NSEC=$(echo "$KEY_JSON" | jq -r '.nsec')
    NPUB=$(echo "$KEY_JSON" | jq -r '.npub')
    
    NSECS+=("$NSEC")
    NPUBS+=("$NPUB")
    
    echo "  Party $i: npub=$NPUB"
    echo "  Party $i: nsec=$NSEC (saved securely)"
done

echo ""
echo "🔑 Running DKG with generated npubs..."

# Join npubs and nsecs with commas
NPUBS_STR=$(IFS=','; echo "${NPUBS[*]}")
NSECS_STR=$(IFS=','; echo "${NSECS[*]}")

# Run keygen (DKG) and capture output (progress goes to stdout, JSON is at the end)
# share_count is automatically derived from the number of partynpubs
DKG_FULL_OUTPUT=$($CLI_BIN --quiet keygen --threshold "$THRESHOLD" --include-parties --partynpubs "$NPUBS_STR" --partynsecs "$NSECS_STR" 2>&1)

if [ $? -ne 0 ]; then
    echo "Error: DKG failed"
    exit 1
fi

# Extract JSON from output (JSON block starts with '{' and ends with '}')
DKG_OUTPUT=$(echo "$DKG_FULL_OUTPUT" | awk '/^\{/,/^}$/')

# Check if DKG was successful
SUCCESS=$(echo "$DKG_OUTPUT" | jq -r '.success // false' 2>/dev/null)

if [ "$SUCCESS" != "true" ]; then
    ERROR=$(echo "$DKG_OUTPUT" | jq -r '.error // "Unknown error"')
    echo "Error: DKG failed - $ERROR"
    exit 1
fi

echo "✅ DKG completed successfully!"
echo ""

# Extract parties array and save each to a file
echo "💾 Saving keyshares to files..."

PARTY_COUNT=$(echo "$DKG_OUTPUT" | jq '.parties | length')

for i in $(seq 0 $((PARTY_COUNT - 1))); do
    PARTY_INDEX=$((i + 1))
    NPUB="${NPUBS[$i]}"
    
    # Extract party data
    PARTY_JSON=$(echo "$DKG_OUTPUT" | jq ".parties[$i]")
    
    # Create filename from npub (remove special chars, use first 20 chars for readability)
    FILENAME=$(echo "$NPUB" | tr -d '/\n\r' | cut -c1-20)
    FILEPATH="$OUTPUT_DIR/${FILENAME}.json"
    
    # Save party to file
    echo "$PARTY_JSON" | jq '.' > "$FILEPATH"
    
    echo "  ✅ Party $PARTY_INDEX saved to: $FILEPATH"
    echo "     npub: $NPUB"
done

echo ""
echo "🎉 Complete! Keyshares saved in: $OUTPUT_DIR/"
echo ""
echo "Summary:"
echo "  Threshold: $THRESHOLD"
echo "  Share count: $SHARE_COUNT"
echo "  Files created: $PARTY_COUNT"
echo ""
echo "⚠️  IMPORTANT: Keep nsec keys secure and never share them!"
echo "   nsec keys: ${NSECS[@]}"

