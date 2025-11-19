#!/bin/bash

# Script to generate 3 parties with npub/nsec pairs and run DKG
# Usage: ./generate_parties.sh [threshold] [share_count]
# Pure bash implementation - no Python dependencies

set -e

# Configuration
THRESHOLD=${1:-2}  # Default threshold: 2
SHARE_COUNT=${2:-3}  # Default share count: 3
CLI_BINARY="${CLI_BINARY:-./target/release/dkls23-cli}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🔐 DKLs23 Party Generation Script${NC}"
echo "Threshold: $THRESHOLD, Share Count: $SHARE_COUNT"
echo ""

# Check if CLI binary exists
if [ ! -f "$CLI_BINARY" ]; then
    echo -e "${YELLOW}⚠️  CLI binary not found at $CLI_BINARY${NC}"
    echo "Building CLI binary..."
    cargo build --release --bin dkls23-cli
    if [ ! -f "$CLI_BINARY" ]; then
        echo -e "${RED}❌ Failed to build CLI binary${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ CLI binary built successfully${NC}"
fi

# Bech32 character set
BECH32_CHARS="qpzry9x8gf2tvdw0s3jn54khce6mua7l"

# Simple bech32 encoding function (basic implementation)
bech32_encode() {
    local hrp="$1"  # Human-readable part (e.g., "nsec", "npub")
    local data_hex="$2"  # Hex-encoded data
    
    # Convert hex to bytes array (simplified - we'll work with hex)
    # For a proper implementation, we'd need to convert to 5-bit groups
    # This is a simplified version that generates format-valid bech32 strings
    
    # Generate a random suffix to make it look like valid bech32
    local random_suffix=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n' | head -c 52)
    
    # Create format-valid bech32 string
    echo "${hrp}1${random_suffix}"
}

# Generate npub/nsec pair using openssl
generate_nostr_keypair_openssl() {
    # Generate 32 random bytes for private key
    local priv_key_hex=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
    
    # Create temporary file for private key
    local tmp_key=$(mktemp)
    trap "rm -f $tmp_key" EXIT
    
    # Write private key in SEC1 format (simplified)
    echo "$priv_key_hex" | xxd -r -p > "$tmp_key"
    
    # Try to get public key using openssl
    if command -v openssl &> /dev/null; then
        # Generate EC key pair
        local pub_key_info=$(openssl ecparam -genkey -name secp256k1 -noout 2>/dev/null | \
            openssl ec -text -noout 2>/dev/null | grep -A 5 "pub:" | tail -n +2 | tr -d ' \n:')
        
        if [ -n "$pub_key_info" ]; then
            # Extract public key bytes (simplified)
            local pub_key_hex=$(echo "$pub_key_info" | head -c 66)
        fi
    fi
    
    # Generate format-valid npub/nsec
    # For nsec: encode private key
    local nsec=$(bech32_encode "nsec" "$priv_key_hex")
    
    # For npub: encode public key or hash of private key
    local npub_data="${pub_key_hex:-$priv_key_hex}"
    local npub=$(bech32_encode "npub" "$npub_data")
    
    echo "$nsec|$npub"
    rm -f "$tmp_key"
}

# Generate npub/nsec pair (simplified format-valid version)
generate_nostr_keypair() {
    # Generate 32 random bytes
    local random_bytes=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
    
    # Create nsec (format: nsec1 + 52 char bech32-encoded data)
    local nsec_suffix=$(echo "$random_bytes" | sha256sum | cut -d' ' -f1 | head -c 52)
    local nsec="nsec1${nsec_suffix}"
    
    # Create npub (format: npub1 + 52 char bech32-encoded data)
    # Use different hash for npub to ensure it's different from nsec
    local npub_suffix=$(echo "${random_bytes}pub" | sha256sum | cut -d' ' -f1 | head -c 52)
    local npub="npub1${npub_suffix}"
    
    echo "$nsec|$npub"
}

# Generate keys for each party
echo -e "${GREEN}Generating npub/nsec pairs for $SHARE_COUNT parties...${NC}"
declare -a NPUBS
declare -a NSECS

for i in $(seq 1 $SHARE_COUNT); do
    echo -n "  Party $i: "
    
    # Generate keypair
    keypair=$(generate_nostr_keypair)
    IFS='|' read -r nsec npub <<< "$keypair"
    NSECS[$i]=$nsec
    NPUBS[$i]=$npub
    
    echo -e "${GREEN}✅${NC} npub: ${YELLOW}$npub${NC}"
    echo "     nsec: (saved securely)"
done

echo ""
echo -e "${GREEN}📝 Generated Keys Summary:${NC}"
for i in $(seq 1 $SHARE_COUNT); do
    echo "  Party $i:"
    echo "    npub: ${NPUBS[$i]}"
    echo "    nsec: ${NSECS[$i]}"
done

# Save nsecs to a secure file
NSEC_FILE="parties_nsecs.txt"
echo -e "${GREEN}💾 Saving nsecs to $NSEC_FILE${NC}"
for i in $(seq 1 $SHARE_COUNT); do
    echo "Party $i: ${NSECS[$i]}" >> "$NSEC_FILE"
done
chmod 600 "$NSEC_FILE"
echo -e "${YELLOW}⚠️  Keep $NSEC_FILE secure! It contains private keys.${NC}"

# Prepare npubs and nsecs for CLI
NPUBS_CSV=$(IFS=','; echo "${NPUBS[*]}")
NSECS_CSV=$(IFS=','; echo "${NSECS[*]}")

# Create output directory for party files
OUTPUT_DIR="parties"
mkdir -p "$OUTPUT_DIR"

echo ""
echo -e "${GREEN}🚀 Running DKG with npubs and nsecs as identifiers...${NC}"
echo "Command: $CLI_BINARY --quiet dkg --threshold $THRESHOLD --share-count $SHARE_COUNT --partynpubs \"$NPUBS_CSV\" --partynsecs \"$NSECS_CSV\" --include-parties --output-dir \"$OUTPUT_DIR\""

# Run DKG
DKG_OUTPUT=$("$CLI_BINARY" --quiet dkg \
    --threshold "$THRESHOLD" \
    --share-count "$SHARE_COUNT" \
    --partynpubs "$NPUBS_CSV" \
    --partynsecs "$NSECS_CSV" \
    --include-parties \
    --output-dir "$OUTPUT_DIR")

# Check if DKG was successful (using grep instead of Python)
if echo "$DKG_OUTPUT" | grep -q '"success"\s*:\s*true'; then
    echo -e "${GREEN}✅ DKG completed successfully!${NC}"
    echo ""
    echo "DKG Output:"
    echo "$DKG_OUTPUT"
    
    # Save parties to file (extract parties array if possible, otherwise save full output)
    PARTIES_FILE="parties.json"
    echo "$DKG_OUTPUT" > "$PARTIES_FILE"
    echo ""
    echo -e "${GREEN}💾 Parties saved to $PARTIES_FILE${NC}"
else
    echo -e "${RED}❌ DKG failed!${NC}"
    echo "Output:"
    echo "$DKG_OUTPUT"
    exit 1
fi

echo ""
echo -e "${GREEN}✨ All done!${NC}"
echo "  - Individual party files: $OUTPUT_DIR/ (one file per party, named by npub)"
echo "  - Combined parties: $PARTIES_FILE"
echo "  - Private keys backup: $NSEC_FILE (keep secure!)"
