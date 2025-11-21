# DKLs23 Examples

This directory contains examples demonstrating how to use the DKLs23 Threshold ECDSA library.

## Examples

### 1. Simple Demo (`simple_demo.rs`)
A basic introduction to the library that shows:
- Library parameters and security settings
- Basic cryptographic operations
- Available features

**Run with:**
```bash
cargo run --example simple_demo
```

### 2. Basic Usage (`basic_usage.rs`)
A complete example that demonstrates:
- Setting up a 2-of-3 threshold scheme
- Performing Distributed Key Generation (DKG)
- Creating a threshold signature
- Verifying the signature

**Run with:**
```bash
cargo run --example basic_usage
```

### 3. Distributed DKG (`distributed_dkg.rs`)
An example demonstrating distributed DKG where each party runs as a separate process.
This shows how to:
- Initialize each party independently
- Generate keyshares locally on each party's machine
- Exchange messages between parties (simulated in this example)
- Complete DKG with parties running on different machines

**Key Features:**
- Each party maintains its own state machine
- Messages are serializable (can be sent over network)
- Parties can run concurrently on different machines
- Each party generates its own keyshare locally

**Run with:**
```bash
cargo run --example distributed_dkg
```

**Note:** In a real deployment, each party would:
1. Run as a separate process/machine
2. Communicate via network (HTTP, gRPC, WebSockets, etc.)
3. Serialize messages using serde (JSON, bincode, MessagePack, etc.)
4. Handle network errors and retries appropriately

## Prerequisites

Before running the examples, make sure you have:

1. **Rust installed:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Dependencies built:**
   ```bash
   cargo build
   ```

## Understanding the Examples

### Threshold Schemes
- **2-of-3**: Any 2 out of 3 parties can create a signature
- **3-of-5**: Any 3 out of 5 parties can create a signature
- **t-of-n**: Any t out of n parties can create a signature

### Key Concepts
- **Distributed Key Generation (DKG)**: Multiple parties generate shares of a secret key
- **Threshold Signing**: Only a subset of parties (threshold) needed to sign
- **Zero-knowledge Proofs**: Prove knowledge without revealing secrets
- **Oblivious Transfer**: Secure communication between parties

### Security
- **256-bit security**: Equivalent to standard ECDSA
- **80-bit statistical security**: Protection against statistical attacks
- **Cryptographic commitments**: Prevent cheating in protocols

## Next Steps

After running the examples:

1. **Read the documentation**: `cargo doc --open`
2. **Explore the source code**: Check `src/protocols/` for implementation details
3. **Run tests**: `cargo test` to see more examples
4. **Build your application**: Use the library in your own projects

## Troubleshooting

If you encounter issues:

1. **Rust not found**: Install Rust using the command above
2. **Compilation errors**: Run `cargo clean && cargo build`
3. **Test failures**: Check that all dependencies are properly installed
4. **Memory issues**: The examples use significant memory for cryptographic operations

## More Information

- [DKLs23 Paper](https://eprint.iacr.org/2023/765.pdf)
- [Library Documentation](https://docs.rs/dkls23)
- [GitHub Repository](https://github.com/0xCarbon/DKLs23)






