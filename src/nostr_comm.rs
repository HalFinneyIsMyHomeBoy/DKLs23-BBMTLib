//! Nostr communication module
//! 
//! This module handles all nostr-related functions for connecting to relays
//! and listening for events.

use nostr_sdk::prelude::*;
use nostr::nips::nip44;
use std::time::{SystemTime, UNIX_EPOCH};
use base64::{Engine as _, engine::general_purpose::STANDARD as base64_engine};

/// Verifies if an encrypted payload is in NIP-44 format.
/// 
/// NIP-44 (https://github.com/nostr-protocol/nips/blob/master/44.md) is a versioned
/// encryption format. This function checks if the payload follows the NIP-44 specification:
/// - Payload format: base64(version || nonce || ciphertext || mac)
/// - Version 2 (0x02) uses: secp256k1 ECDH, HKDF, ChaCha20, HMAC-SHA256
/// 
/// # Arguments
/// 
/// * `encrypted_content` - The base64-encoded encrypted content to verify
/// 
/// # Returns
/// 
/// Returns `true` if the payload is a valid NIP-44 v2 encrypted message, `false` otherwise.
/// 
/// # Example
/// 
/// ```no_run
/// use dkls23::nostr_comm::is_nip44_encrypted;
/// 
/// let encrypted = "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb";
/// if is_nip44_encrypted(encrypted) {
///     println!("This is a NIP-44 encrypted message");
/// }
/// ```
pub fn is_nip44_encrypted(encrypted_content: &str) -> bool {
    // Check if payload is NIP-44 format by verifying:
    // 1. It's valid base64
    // 2. After decoding, first byte is version (0x02 for v2)
    // 3. Minimum length requirements (132 chars base64 = 99 bytes decoded)
    
    if encrypted_content.starts_with('#') {
        // '#' indicates non-base64 encoding (future-proof flag)
        return false;
    }
    
    // NIP-44 payload size constraints: 132-87472 base64 chars
    if encrypted_content.len() < 132 || encrypted_content.len() > 87472 {
        return false;
    }
    
    // Try to decode and check version byte
    match base64_engine.decode(encrypted_content) {
        Ok(decoded) => {
            // Check minimum decoded size (99 bytes: 1 version + 32 nonce + 32 ciphertext min + 32 MAC)
            // Maximum decoded size: 65603 bytes
            if decoded.len() >= 99 && decoded.len() <= 65603 {
                // Check version byte (0x02 for NIP-44 v2)
                decoded[0] == 0x02
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Connects to a nostr relay and listens for events meant for the local npub.
/// 
/// # Arguments
/// 
/// * `local_npub` - The local public key (npub) in bech32 format
/// * `local_nsec` - The local secret key (nsec) in bech32 format  
/// * `nostr_relay_url` - The URL of the nostr relay to connect to
/// 
/// # Returns
/// 
/// Returns `Ok(())` if successful, or an error if connection or parsing fails.
/// 
/// # Example
/// 
/// ```no_run
/// use dkls23::nostr_comm::NostrListen;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     NostrListen(
///         "npub1...",
///         "nsec1...",
///         "wss://relay.example.com"
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn NostrListen(
    local_npub: &str,
    local_nsec: &str,
    nostr_relay_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the secret key to create Keys object
    let keys = Keys::parse(local_nsec)?;
    
    // Verify that the public key matches
    let expected_pubkey = PublicKey::from_bech32(local_npub)?;
    if keys.public_key() != expected_pubkey {
        return Err("Public key mismatch: local_npub does not match local_nsec".into());
    }
    
    // Create a new client with the keys
    let client = Client::new(&keys);
    
    // Add the relay
    client.add_relay(nostr_relay_url).await?;
    
    // Connect to the relay
    client.connect().await;
    
    // Calculate timestamp for 30 seconds ago
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let since_timestamp = now - 30;
    
    // Create a filter to listen for encrypted direct messages using NIP-44 encryption
    // NIP-44 (https://github.com/nostr-protocol/nips/blob/master/44.md) is a versioned
    // encryption format that uses kind 4 (EncryptedDirectMessage) but with a different
    // encryption scheme than NIP-04 (ChaCha20, HKDF, HMAC-SHA256, base64 encoding)
    // Filter for events directed to the local public key from the last 30 seconds
    let filter = Filter::new()
        .kind(Kind::EncryptedDirectMessage)  // Kind 4, encrypted with NIP-44 format
        .pubkey(expected_pubkey)
        .since(Timestamp::from_secs(since_timestamp));
    
    // Subscribe to events matching the filter
    client.subscribe(vec![filter], None).await?;
    
    // Listen for incoming events
    let mut notifications = client.notifications();
    while let Ok(notification) = notifications.recv().await {
        match notification {
            RelayPoolNotification::Event { event, .. } => {
                // Verify this is a NIP-44 encrypted message
                if !is_nip44_encrypted(&event.content) {
                    // Skip non-NIP-44 messages
                    continue;
                }
                
                // This is a verified NIP-44 encrypted message
                println!("Received NIP-44 encrypted event for {}: {}", local_npub, event.id);
                println!("Encrypted content (NIP-44 v2): {}", event.content);
                println!("Event created at: {}", event.created_at);
                println!("Sender pubkey: {}", event.pubkey);
                
                // Attempt to decrypt using NIP-44
                // Use nip44::decrypt with recipient secret key and sender public key
                match nip44::decrypt(
                    keys.secret_key(),
                    &event.pubkey,
                    &event.content,
                ) {
                    Ok(decrypted) => {
                        println!("✓ Successfully decrypted NIP-44 message: {}", decrypted);
                    }
                    Err(e) => {
                        println!("✗ Failed to decrypt NIP-44 message: {}", e);
                        println!("  (This might be a NIP-04 message or decryption key mismatch)");
                    }
                }
            }
            RelayPoolNotification::Message { message, .. } => {
                // Handle other relay messages if needed
                println!("Received message: {:?}", message);
            }
            _ => {
                // Handle other notification types
            }
        }
    }
    
    Ok(())
}

/// Sends an encrypted direct message via NIP-44 to a destination npub.
/// 
/// This function encrypts a message using NIP-44 encryption and sends it as an
/// encrypted direct message (kind 4) to the specified destination.
/// 
/// # Arguments
/// 
/// * `local_npub` - The local public key (npub) in bech32 format
/// * `local_nsec` - The local secret key (nsec) in bech32 format
/// * `nostr_relay_url` - The URL of the nostr relay to connect to
/// * `destination_npub` - The destination public key (npub) in bech32 format
/// * `message` - The plaintext message to encrypt and send
/// 
/// # Returns
/// 
/// Returns `Ok(())` if the message was successfully sent, or an error if encryption,
/// connection, or publishing fails.
/// 
/// # Example
/// 
/// ```no_run
/// use dkls23::nostr_comm::nostrSend;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     nostrSend(
///         "npub1...",
///         "nsec1...",
///         "wss://relay.example.com",
///         "npub1destination...",
///         "Hello, this is a NIP-44 encrypted message!"
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn nostrSend(
    local_npub: &str,
    local_nsec: &str,
    nostr_relay_url: &str,
    destination_npub: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the secret key to create Keys object
    let keys = Keys::parse(local_nsec)?;
    
    // Verify that the public key matches
    let expected_pubkey = PublicKey::from_bech32(local_npub)?;
    if keys.public_key() != expected_pubkey {
        return Err("Public key mismatch: local_npub does not match local_nsec".into());
    }
    
    // Parse the destination public key
    let destination_pubkey = PublicKey::from_bech32(destination_npub)?;
    
    // Create a new client with the keys
    let client = Client::new(&keys);
    
    // Add the relay
    client.add_relay(nostr_relay_url).await?;
    
    // Connect to the relay
    client.connect().await;
    
    // Encrypt the message using NIP-44
    // NIP-44 uses versioned encryption: secp256k1 ECDH, HKDF, ChaCha20, HMAC-SHA256
    // Reference: https://github.com/nostr-protocol/nips/blob/master/44.md
    let encrypted_content = nip44::encrypt(
        keys.secret_key(),
        &destination_pubkey,
        message,
        nip44::Version::V2,
    )?;
    
    // Create an encrypted direct message event (kind 4)
    // The event should be tagged with the recipient's public key (p tag)
    // Tag format: ["p", <pubkey>]
    let recipient_tag = Tag::parse(&["p", &destination_pubkey.to_string()])?;
    let event = EventBuilder::new(
        Kind::EncryptedDirectMessage,
        encrypted_content,
        [recipient_tag],
    )
    .to_event(&keys)?;
    
    // Publish the event to the relay
    client.send_event(event).await?;
    
    println!("✓ Successfully sent NIP-44 encrypted message to {}", destination_npub);
    
    Ok(())
}

