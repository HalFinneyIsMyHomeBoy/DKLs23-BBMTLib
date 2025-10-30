# Building and Installing the DKLs23 CLI

## Prerequisites

You need Rust and Cargo installed. If you don't have them:

```bash
# Install Rust via rustup (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Or use your package manager
sudo apt install cargo rustc
```

## Building

### Option 1: Build for Development (Debug)

```bash
cargo build --bin dkls23-cli
```

The binary will be at: `target/debug/dkls23-cli`

### Option 2: Build for Release (Optimized)

```bash
cargo build --release --bin dkls23-cli
```

The binary will be at: `target/release/dkls23-cli`

### Option 3: Install System-Wide

```bash
cargo install --path . --bin dkls23-cli
```

This installs the binary to `~/.cargo/bin/dkls23-cli` (or `~/.local/bin/dkls23-cli` depending on your setup).

Make sure `~/.cargo/bin` (or `~/.local/bin`) is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# Add to ~/.bashrc or ~/.zshrc to make permanent
```

## Running the CLI

### Method 1: Run from Build Directory

```bash
# After building in debug mode
./target/debug/dkls23-cli dkg --threshold 2 --share-count 3 --quiet

# After building in release mode
./target/release/dkls23-cli dkg --threshold 2 --share-count 3 --quiet
```

### Method 2: Run After Installation

```bash
# If installed via cargo install
dkls23-cli dkg --threshold 2 --share-count 3 --quiet
```

### Method 3: Add to PATH Temporarily

```bash
# For debug build
export PATH="$PWD/target/debug:$PATH"
dkls23-cli dkg --threshold 2 --share-count 3 --quiet

# For release build
export PATH="$PWD/target/release:$PATH"
dkls23-cli dkg --threshold 2 --share-count 3 --quiet
```

## Quick Test

Once built, test it works:

```bash
# Test DKG
./target/release/dkls23-cli dkg --threshold 2 --share-count 3 --quiet | jq '.success'

# Should output: true
```

## Troubleshooting

### "command not found"

- Make sure you've built the binary first
- Use the full path to the binary: `./target/release/dkls23-cli`
- Or install it: `cargo install --path . --bin dkls23-cli`

### "cargo: command not found"

- Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Restart your terminal after installation
- Verify: `cargo --version`

### Build Errors

- Make sure all dependencies are installed
- Try cleaning and rebuilding: `cargo clean && cargo build --release --bin dkls23-cli`

