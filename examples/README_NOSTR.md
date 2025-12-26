# Nostr-based Multi-Party DKLs23 Examples

This directory contains examples demonstrating how to use nostr as a communication layer for multi-party DKLs23 Threshold ECDSA operations.

## 🏆 EASIEST Approach: `nostr_easiest.rs`

The **simplest and most practical** way to use nostr for multi-party operations is demonstrated in `nostr_easiest.rs`. This approach:

1. **Uses nostr ONLY for coordination signals** - No need to modify the core DKLs23 protocol
2. **Keeps all heavy computation local** - Uses existing DKLs23 facade unchanged
3. **JSON-encodes coordination messages** - Easy to serialize and deserialize
4. **Uses session-based filtering** - Clean separation between different operations
5. **Minimal dependencies** - Only requires nostr-sdk for coordination

### How it works:

```rust
// 1. Create nostr coordinators for each party
let coord = NostrCoordinator::new(party_index, session_id).await?;

// 2. Coordinate operation start via nostr
coord.send_message(0, "DKG_START", "2-of-3", "coordination").await?;

// 3. Wait for all parties to be ready
let messages = coord.receive_messages("coordination", 10).await?;

// 4. Run actual DKG using existing facade
let parties = run_dkg_offline(&parameters, session_id.as_bytes())?;

// 5. Share results via nostr (optional)
coord.send_message(0, "DKG_RESULT", &party_info, "results").await?;
```

## Running the Example

### Prerequisites

Add the required dependencies to your `Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...
nostr = "0.34"
tokio = { version = "1.0", features = ["full"] }
futures = "0.3"
```

### Run the easiest example:

```bash
cargo run --example nostr_easiest
```

### Expected output:

```
🌐 Simple Nostr DKLs23 Demo
=============================
📡 Relay: wss://bbw-nostr.xyz
📋 Config: 2-of-3 scheme
🆔 Session: simple_nostr_demo
📝 Message: Hello from Nostr DKLs23!

🔐 Step 1: Distributed Key Generation
📤 Party 1 -> ALL: DKG_START (coordination)
📥 Party 2 <- Party 1: DKG_START (coordination)
📥 Party 3 <- Party 1: DKG_START (coordination)
📤 Party 1 -> ALL: DKG_ACK (coordination)
📤 Party 2 -> ALL: DKG_ACK (coordination)
📤 Party 3 -> ALL: DKG_ACK (coordination)
✅ All parties coordinated, running DKG...
🔐 Starting Distributed Key Generation...
✅ DKG completed successfully!
✅ Nostr DKG completed!

👥 Generated Parties:
   Party 1: bc1qexample...
   Party 2: bc1qexample...
   Party 3: bc1qexample...

✍️  Step 2: Threshold Signing
📤 Party 1 -> ALL: SIGN_START (coordination)
📥 Party 2 <- Party 1: SIGN_START (coordination)
✅ All parties coordinated, running signing...
✍️  Starting Threshold Signature...
✅ Threshold signature completed!
✅ Nostr signing completed!

🎉 Results:
   Bitcoin Address: bc1qexample...
   Signature: r=..., s=..., recid=...

💡 This is the EASIEST way to use nostr for multi-party DKLs23:
   1. Use nostr ONLY for coordination signals
   2. Keep all heavy computation local (use existing DKLs23 facade)
   3. JSON-encode coordination messages for simplicity
   4. Use session-based filtering for clean separation
   5. Minimal code changes required
```

## Advanced Approach: `nostr_multi_party.rs`

For more complex scenarios, `nostr_multi_party.rs` demonstrates:

- **Detailed phase-by-phase nostr messaging** - Each DKG phase uses nostr
- **Type-safe message structures** - Different message types for different phases
- **Advanced filtering** - Multiple dimensions of message routing
- **Concurrent session handling** - Support for multiple simultaneous operations

## Key Benefits of Using Nostr

### 1. **Decentralized Communication**
- No central server required
- Uses existing nostr relay network
- Resilient to single points of failure

### 2. **Simple Integration**
- Minimal changes to existing DKLs23 code
- JSON-based message format
- Session-based coordination

### 3. **Real-world Usability**
- Parties can be anywhere in the world
- Asynchronous communication support
- Message persistence via relay

### 4. **Privacy & Security**
- Encrypted direct messages possible
- Public key-based authentication
- No IP address exposure required

## Customization Options

### Different Relays
Change the relay URL in the code:

```rust
const RELAY_URL: &str = "wss://your-relay.com";
```

### Different Party Configurations
Modify the parameters:

```rust
let party_count = 5;  // More parties
let threshold = 3;    // Higher threshold
```

### Custom Session IDs
Use unique session identifiers:

```rust
let session_id = "my_custom_session_2024";
```

## Production Considerations

For production use, consider:

1. **Error Handling** - More robust error recovery
2. **Message Encryption** - Use NIP-04 encrypted messages
3. **Authentication** - Verify party identities
4. **Timeout Management** - Adjust timeouts based on network conditions
5. **Message Persistence** - Handle relay disconnections gracefully

## Troubleshooting

### Connection Issues
- Verify relay accessibility
- Check network connectivity
- Try different relays

### Message Delivery
- Increase timeout values
- Verify session ID consistency
- Check message filtering

### Performance
- Use local relays for testing
- Optimize message sizes
- Consider batching messages

## Next Steps

1. **Test with multiple machines** - Run parties on different devices
2. **Implement encrypted messaging** - Use NIP-04 for privacy
3. **Add authentication** - Verify party identities
4. **Create a web interface** - Build a frontend for the protocol
5. **Integrate with Bitcoin** - Use generated signatures for transactions

This approach provides the easiest path to multi-party DKLs23 operations using nostr while maintaining the security and reliability of the core protocol.