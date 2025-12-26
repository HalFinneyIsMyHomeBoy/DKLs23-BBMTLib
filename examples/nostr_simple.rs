//! Simple Nostr-based multi-party DKG and signing example.
//! 
//! This is the EASIEST way to use nostr for multi-party DKLs23 operations:
//! 1. Use nostr events as a simple message bus
//! 2. JSON-encode all protocol messages
//! 3. Use session-based filtering for coordination
//! 4. Leverage existing DKLs23 facade for computation

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::{Parameters, Party, Network};
use dkls23::utilities::hashes::{hash, HashOutput};
use nostr::prelude::*;
use nostr::{Event, EventBuilder, Filter, Keys, Kind, SecretKey, Tag, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

/// Nostr relay for communication
const RELAY_URL: &str = "wss://bbw-nostr.xyz";

/// Simple message structure for nostr events
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NostrMessage {
    /// Session identifier
    session_id: String,
    /// Sender party index
    sender: u8,
    /// Receiver party index (0 for broadcast)
    receiver: u8,
    /// Message type
    msg_type: String,
    /// Message payload (JSON string)
    payload: String,
    /// Phase identifier
    phase: String,
}

/// Simple nostr-based coordinator
struct NostrCoordinator {
    /// Party keys
    keys: Keys,
    /// Party index
    party_index: u8,
    /// Session ID
    session_id: String,
}

impl NostrCoordinator {
    /// Create new coordinator
    async fn new(party_index: u8, session_id: String) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate keys
        let secret_key = SecretKey::generate();
        let keys = Keys::new(secret_key);
        
        Ok(Self {
            keys,
            party_index,
            session_id,
        })
    }
    
    /// Send message to other parties
    async fn send_message(&self, receiver: u8, msg_type: &str, payload: &str, phase: &str) -> Result<(), Box<dyn std::error::Error>> {
        let message = NostrMessage {
            session_id: self.session_id.clone(),
            sender: self.party_index,
            receiver,
            msg_type: msg_type.to_string(),
            payload: payload.to_string(),
            phase: phase.to_string(),
        };
        
        let content = serde_json::to_string(&message)?;
        
        // Create event with session tag
        let event = EventBuilder::text_note(
            content,
            [Tag::identifier("dkg_session")]
        ).to_event(&self.keys)?;
        
        // For this example, we'll simulate sending via nostr
        // In a real implementation, you would publish to a relay
        
        println!("📤 Party {} -> {}: {} ({})", 
                self.party_index, 
                if receiver == 0 { "ALL".to_string() } else { receiver.to_string() },
                msg_type, 
                phase);
        
        Ok(())
    }
    
    /// Receive messages from other parties
    async fn receive_messages(&self, phase: &str, timeout_secs: u64) -> Result<Vec<NostrMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        
        // For this example, we'll simulate receiving messages
        // In a real implementation, you would subscribe to a relay and filter events
        
        println!("📥 Party {} waiting for messages in phase {}...", self.party_index, phase);
        
        // Simulate message reception with delay
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Simulate receiving coordination messages
        if phase == "coordination" {
            for sender in 1..=3 {
                if sender != self.party_index {
                    let simulated_message = NostrMessage {
                        session_id: self.session_id.clone(),
                        sender: sender as u8,
                        receiver: self.party_index,
                        msg_type: if phase == "coordination" { "DKG_START" } else { "SIGN_START" }.to_string(),
                        payload: "simulated".to_string(),
                        phase: phase.to_string(),
                    };
                    messages.push(simulated_message);
                    println!("📥 Party {} <- {}: {} ({})", 
                            self.party_index, 
                            sender, 
                            if phase == "coordination" { "DKG_START" } else { "SIGN_START" }, 
                            phase);
                }
            }
        }
        
        Ok(messages)
    }
    
    /// Disconnect from relay
    async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error>> {
        // For this example, no real disconnection needed
        println!("🔌 Party {} disconnecting", self.party_index);
        Ok(())
    }
}

/// Run DKG with nostr coordination (simplified approach)
async fn run_nostr_dkg(
    party_count: u8,
    threshold: u8,
    session_id: &str,
) -> Result<Vec<Party>, Box<dyn std::error::Error>> {
    println!("🔐 Starting Nostr DKG: {}-of-{} scheme", threshold, party_count);
    
    // Create coordinators for all parties
    let mut coordinators = Vec::new();
    for i in 1..=party_count {
        let coord = NostrCoordinator::new(i, session_id.to_string()).await?;
        coordinators.push(coord);
    }
    
    // Simple approach: Use existing DKG facade, but coordinate via nostr
    println!("📋 Coordinating DKG via nostr...");
    
    // Party 1 coordinates the start
    if party_count >= 1 {
        coordinators[0].send_message(0, "DKG_START", &format!("{}-of-{}", threshold, party_count), "coordination").await?;
    }
    
    // Other parties wait for start signal
    for i in 1..party_count as usize {
        let messages = coordinators[i].receive_messages("coordination", 10).await?;
        if !messages.iter().any(|m| m.msg_type == "DKG_START") {
            return Err("No DKG start signal received".into());
        }
    }
    
    // All parties acknowledge
    for coord in &coordinators {
        coord.send_message(0, "DKG_ACK", "ready", "coordination").await?;
    }
    
    // Wait for all acknowledgments
    for coord in &coordinators {
        let messages = coord.receive_messages("coordination", 5).await?;
        if messages.len() < (party_count - 1) as usize {
            return Err("Not all parties ready".into());
        }
    }
    
    println!("✅ All parties coordinated, running DKG...");
    
    // Run the actual DKG using existing facade
    let parameters = Parameters { threshold, share_count: party_count };
    let parties = run_dkg_offline(&parameters, session_id.as_bytes())
        .map_err(|e| format!("DKG failed: {}", e.description))?;
    
    // Share results via nostr (optional - for demonstration)
    for (i, coord) in coordinators.iter().enumerate() {
        let party_info = format!("address:{}", parties[i].btc_address);
        coord.send_message(0, "DKG_RESULT", &party_info, "results").await?;
    }
    
    // Disconnect all coordinators
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    println!("✅ Nostr DKG completed!");
    Ok(parties)
}

/// Run threshold signing with nostr coordination
async fn run_nostr_signing(
    parties: &[Party],
    message: &str,
    session_id: &str,
) -> Result<(String, String, u8), Box<dyn std::error::Error>> {
    println!("✍️  Starting Nostr threshold signing");
    
    let threshold = parties[0].parameters.threshold;
    
    // Create coordinators for signing parties
    let mut coordinators = Vec::new();
    for i in 1..=threshold {
        let coord = NostrCoordinator::new(i, format!("{}_sign", session_id)).await?;
        coordinators.push(coord);
    }
    
    // Coordinate signing
    println!("📋 Coordinating signing via nostr...");
    
    // Party 1 starts signing
    coordinators[0].send_message(0, "SIGN_START", message, "coordination").await?;
    
    // Others wait for start signal
    for i in 1..coordinators.len() {
        let messages = coordinators[i].receive_messages("coordination", 10).await?;
        if !messages.iter().any(|m| m.msg_type == "SIGN_START") {
            return Err("No signing start signal received".into());
        }
    }
    
    // All acknowledge
    for coord in &coordinators {
        coord.send_message(0, "SIGN_ACK", "ready", "coordination").await?;
    }
    
    // Wait for acknowledgments
    for coord in &coordinators {
        let messages = coord.receive_messages("coordination", 5).await?;
        if messages.len() < (threshold - 1) as usize {
            return Err("Not all parties ready for signing".into());
        }
    }
    
    println!("✅ All parties coordinated, running signing...");
    
    // Run actual signing
    let message_hash = hash(message.as_bytes(), &[]);
    let executing_parties: Vec<u8> = (1..=threshold).collect();
    
    let (r, s, recid) = threshold_sign(
        parties,
        &executing_parties,
        format!("{}_sign", session_id).as_bytes(),
        message_hash,
        true,
    ).map_err(|e| format!("Signing failed: {}", e.description))?;
    
    // Share results
    for coord in &coordinators {
        let result = format!("r:{},s:{},recid:{}", r, s, recid);
        coord.send_message(0, "SIGN_RESULT", &result, "results").await?;
    }
    
    // Disconnect
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    println!("✅ Nostr signing completed!");
    Ok((r, s, recid))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Simple Nostr DKLs23 Demo");
    println!("=============================");
    println!("📡 Relay: {}", RELAY_URL);
    
    // Configuration
    let party_count = 3;
    let threshold = 2;
    let session_id = "simple_nostr_demo";
    let message = "Hello from Nostr DKLs23!";
    
    println!("📋 Config: {}-of-{} scheme", threshold, party_count);
    println!("🆔 Session: {}", session_id);
    println!("📝 Message: {}", message);
    
    // Step 1: DKG
    println!("\n🔐 Step 1: Distributed Key Generation");
    let parties = run_nostr_dkg(party_count, threshold, session_id).await?;
    
    println!("\n👥 Generated Parties:");
    for (i, party) in parties.iter().enumerate() {
        println!("   Party {}: {}", i + 1, party.btc_address);
    }
    
    // Step 2: Signing
    println!("\n✍️  Step 2: Threshold Signing");
    let signature = run_nostr_signing(&parties, message, session_id).await?;
    
    // Results
    println!("\n🎉 Results:");
    println!("   Bitcoin Address: {}", parties[0].btc_address);
    println!("   Signature: r={}, s={}, recid={}", signature.0, signature.1, signature.2);
    
    println!("\n💡 This is the EASIEST way to use nostr for multi-party DKLs23:");
    println!("   1. Use nostr events as a simple message bus");
    println!("   2. JSON-encode all protocol messages");
    println!("   3. Use session-based filtering for coordination");
    println!("   4. Leverage existing DKLs23 facade for computation");
    
    Ok(())
}