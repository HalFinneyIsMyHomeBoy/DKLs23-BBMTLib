# DKLs23 CLI Usage Guide

The `dkls23-cli` binary provides a command-line interface to the DKLs23 Threshold ECDSA library, enabling integration from any programming language through JSON-based input/output.

## Building the CLI

Build the CLI binary:

```bash
cargo build --release --bin dkls23-cli
```

The binary will be at `target/release/dkls23-cli`.

## Commands

### DKG (Distributed Key Generation)

Generates a set of parties for threshold signing.

**Syntax:**
```bash
dkls23-cli [--quiet] dkg --threshold <t> --share-count <n> [OPTIONS]
```

Note: The `--quiet` flag must come before the subcommand (`dkg` or `sign`).

**Options:**
- `--threshold <t>`: Minimum number of parties needed to sign (required)
- `--share-count <n>`: Total number of parties (required)
- `--session-id <hex>`: Session ID as hex-encoded string (default: empty)
- `--network <network>`: Network type: "mainnet" or "testnet3" (default: "mainnet")
- `--quiet`: Suppress progress output (only output JSON)

**Example:**
```bash
# Without quiet (shows progress messages)
dkls23-cli dkg --threshold 2 --share-count 3 --session-id 0a1b2c3d

# With quiet (only JSON output)
dkls23-cli --quiet dkg --threshold 2 --share-count 3 --session-id 0a1b2c3d
```

**Output (JSON):**
```json
{
  "success": true,
  "parties": [
    {
      "parameters": {
        "threshold": 2,
        "share_count": 3
      },
      "party_index": 1,
      "session_id": [...],
      "btc_address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
      "network": "Mainnet",
      ...
    },
    ...
  ],
  "bitcoin_address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
  "network": "Mainnet",
  "error": null
}
```

**Error Output:**
```json
{
  "success": false,
  "parties": null,
  "bitcoin_address": null,
  "network": null,
  "error": "DKG aborted by party 2: Invalid proof"
}
```

### Sign

Creates a threshold signature using previously generated parties.

**Syntax:**
```bash
dkls23-cli [--quiet] sign --executing-parties <indices> --message <message> [OPTIONS]
```

Note: The `--quiet` flag must come before the subcommand.

**Options:**
- `--executing-parties <indices>`: Comma-separated party indices (e.g., "1,2,3") (required)
- `--message <message>`: Message to sign (required)
- `--message-hex`: Treat message as hex-encoded (default: false, treats as plain text)
- `--parties <file>`: Path to JSON file containing parties (default: read from stdin)
- `--sign-id <hex>`: Sign ID as hex-encoded string (default: empty)
- `--normalize-low-s <bool>`: Normalize s value for Bitcoin compliance (default: true)
- `--quiet`: Suppress progress output

**Example 1: Sign with parties from file**
```bash
dkls23-cli --quiet sign \
  --parties parties.json \
  --executing-parties "1,2" \
  --message "Hello, World!"
```

**Example 2: Sign with parties from stdin (pipe from DKG)**
```bash
dkls23-cli --quiet dkg --threshold 2 --share-count 3 | \
  jq -r '.parties' | \
  dkls23-cli --quiet sign \
    --executing-parties "1,2" \
    --message "Hello, World!"
```

**Example 3: Sign with hex-encoded message hash**
```bash
dkls23-cli --quiet sign \
  --parties parties.json \
  --executing-parties "1,2" \
  --message "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3" \
  --message-hex
```

**Output (JSON):**
```json
{
  "success": true,
  "r": "1234567890abcdef...",
  "s": "abcdef1234567890...",
  "recid": 0,
  "error": null
}
```

**Error Output:**
```json
{
  "success": false,
  "r": null,
  "s": null,
  "recid": null,
  "error": "Signing aborted by party 2: Invalid signature component"
}
```

## Integration Examples

### Python

```python
import json
import subprocess

def run_dkg(threshold, share_count, session_id=""):
    """Run DKG and return parties."""
    cmd = [
        "dkls23-cli", "dkg",
        "--threshold", str(threshold),
        "--share-count", str(share_count),
        "--session-id", session_id,
        "--quiet"
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    output = json.loads(result.stdout)
    
    if not output["success"]:
        raise Exception(f"DKG failed: {output.get('error')}")
    
    return output["parties"]

def sign_message(parties, executing_parties, message, message_hex=False):
    """Sign a message using parties."""
    # Write parties to temporary file or pass via stdin
    parties_json = json.dumps(parties)
    
    cmd = [
        "dkls23-cli", "sign",
        "--executing-parties", ",".join(map(str, executing_parties)),
        "--message", message,
        "--quiet"
    ]
    
    if message_hex:
        cmd.append("--message-hex")
    
    result = subprocess.run(
        cmd,
        input=parties_json,
        capture_output=True,
        text=True
    )
    output = json.loads(result.stdout)
    
    if not output["success"]:
        raise Exception(f"Signing failed: {output.get('error')}")
    
    return {
        "r": output["r"],
        "s": output["s"],
        "recid": output["recid"]
    }

# Example usage
parties = run_dkg(threshold=2, share_count=3, session_id="example")
signature = sign_message(parties, [1, 2], "Hello, World!")
print(f"Signature: r={signature['r']}, s={signature['s']}")
```

### Node.js/JavaScript

```javascript
const { exec } = require('child_process');
const { promisify } = require('util');
const execAsync = promisify(exec);

async function runDKG(threshold, shareCount, sessionId = '') {
    const cmd = `dkls23-cli --quiet dkg --threshold ${threshold} --share-count ${shareCount} --session-id ${sessionId}`;
    const { stdout } = await execAsync(cmd);
    const output = JSON.parse(stdout);
    
    if (!output.success) {
        throw new Error(`DKG failed: ${output.error}`);
    }
    
    return output.parties;
}

async function signMessage(parties, executingParties, message, messageHex = false) {
    const partiesJson = JSON.stringify(parties);
    let cmd = `dkls23-cli --quiet sign --executing-parties ${executingParties.join(',')} --message "${message}"`;
    
    if (messageHex) {
        cmd += ' --message-hex';
    }
    
    const { stdout } = await execAsync(cmd, { input: partiesJson });
    const output = JSON.parse(stdout);
    
    if (!output.success) {
        throw new Error(`Signing failed: ${output.error}`);
    }
    
    return {
        r: output.r,
        s: output.s,
        recid: output.recid
    };
}

// Example usage
(async () => {
    const parties = await runDKG(2, 3, 'example');
    const signature = await signMessage(parties, [1, 2], 'Hello, World!');
    console.log(`Signature: r=${signature.r}, s=${signature.s}`);
})();
```

### Bash

```bash
#!/bin/bash

# Run DKG
PARTIES_JSON=$(dkls23-cli --quiet dkg --threshold 2 --share-count 3)
echo "DKG output: $PARTIES_JSON"

# Extract parties using jq
PARTIES=$(echo "$PARTIES_JSON" | jq -r '.parties')

# Sign a message
SIGNATURE_JSON=$(echo "$PARTIES" | dkls23-cli --quiet sign --executing-parties "1,2" --message "Hello")
echo "Signature: $SIGNATURE_JSON"

# Check for errors
if echo "$SIGNATURE_JSON" | jq -e '.success == false' > /dev/null; then
    ERROR=$(echo "$SIGNATURE_JSON" | jq -r '.error')
    echo "Error: $ERROR" >&2
    exit 1
fi
```

## Exit Codes

- `0`: Success
- `1`: Error (check JSON output for error details)

## Notes

1. **Progress Output**: By default, progress messages are printed to stderr. Use `--quiet` to suppress them and only get JSON output on stdout.

2. **Party Storage**: Parties contain sensitive cryptographic material. Store them securely and never share them publicly.

3. **Network Parameter**: Currently, the `--network` parameter in `dkg` is accepted but always generates Mainnet addresses (this is a limitation of the facade API).

4. **Message Format**: 
   - Without `--message-hex`: Message is treated as plain text and automatically hashed
   - With `--message-hex`: Message must be exactly 32 bytes (256 bits) of hex-encoded data

5. **Party Indices**: Party indices start at 1 (not 0).

6. **Threshold Requirements**: You must provide at least `threshold` number of parties in `--executing-parties` for signing to succeed.

