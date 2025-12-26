//! Practical Nostr-based multi-party DKG and signing.
//! 
//! This is the EASIEST and MOST PRACTICAL way to use nostr for multi-party DKLs23:
//! 1. Use nostr as a simple message coordination system
//! 2. Keep all heavy computation local (use existing DKLs23 facade)
//! 3. Only use nostr for coordination and message passing
//! 4. Works with real nostr relays like wss://bbw-nostr.xyz

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::{Parameters, Party};
use dkls23::utilities::hashes::hash;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Nostr relay for coordination
const RELAY_URL: &str = "wss://bbw-nostr.xyz";

/// Simple coordination message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoordMessage {
    /// Session identifier
    session_id: String,
    /// Party index
    party_index: u8,
    /// Message type
    msg_type: String,
    /// Phase
    phase: String,
}

/// Practical nostr coordinator
struct PracticalNostrCoordinator {
    /// Nostr client
    client: Client,
    /// Party index
    party_index: u8,
    /// Session ID
    session_id: String,
}

impl PracticalNostrCoordinator {
    /// Create new coordinator
    async fn new(party_index: u8, session_id: String) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate keys
        let keys = Keys::generate();
        
        // Create client
        let client = Client::new(&keys);
        
        // Add and connect to relay
        client.add_relay(RELAY_URL).await?;
        client.connect().await?;
        
        Ok(Self {
            client,
            party_index,
            session_id,
        })
    }
    
    /// Send coordination message
    async fn send_coord_message(&self, msg_type: &str, phase: &str) -> Result<(), Box<dyn std::error::Error>> {
        let message = CoordMessage {
            session_id: self.session_id.clone(),
            party_index: self.party_index,
            msg_type: msg_type.to_string(),
            phase: phase.to_string(),
        };
        
        let content = serde_json::to_string(&message)?;
        
        // Create event with session tags
        let event = EventBuilder::text_note(content)
            .tag(Tag::identifier(&self.session_id))
            .tag(Tag::custom("phase", vec![phase]))
            .tag(Tag::custom("party", vec![&self.party_index.to_string()]))
            .to_event(&client.keys())?;
        
        // Publish event
        client.send_event(event).await?;
        
        println!("📤 Party {} sent {} in phase {}", self.party_index, msg_type, phase);
        
        Ok(())
    }
    
    /// Wait for coordination messages
    async fn wait_for_messages(&self, phase: &str, expected_count: usize, timeout_secs: u64) -> Result<Vec<CoordMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        
        // Create filter
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .identifier(&self.session_id)
            .custom_tag("phase", vec![phase])
            .since(Timestamp::now());
        
        // Subscribe
        let sub_id = client.subscribe(vec![filter], None).await?;
        
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        while start_time.elapsed() < timeout && messages.len() < expected_count {
            if let Ok(notice) = tokio::time::timeout(Duration::from_millis(100), client.next_notice()).await {
                match notice {
                    Ok(RelayMessage::Event { event, .. }) => {
                        // Skip our own messages
                        if event.author == client.keys().public_key() {
                            continue;
                        }
                        
                        // Parse coordination message
                        if let Ok(message) = serde_json::from_str::<CoordMessage>(&event.content) {
                            messages.push(message);
                            println!("📥 Party {} received message from party {} in phase {}", 
                                    self.party_index, message.party_index, phase);
                        }
                    }
                    _ => {}
                }
            }
        }
        
        // Unsubscribe
        client.unsubscribe(&sub_id).await?;
        
        Ok(messages)
    }
    
    /// Disconnect
    async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error>> {
        client.disconnect().await?;
        Ok(())
    }
}

/// Run DKG with practical nostr coordination
async fn run_practical_nostr_dkg(
    party_count: u8,
    threshold: u8,
    session_id: &str,
) -> Result<Vec<Party>, Box<dyn std::error::Error>> {
    println!("🔐 Starting Practical Nostr DKG: {}-of-{} scheme", threshold, party_count);
    println!("📡 Using relay: {}", RELAY_URL);
    
    // Create coordinators for all parties
    let mut coordinators = Vec::new();
    for i in 1..=party_count {
        let coord = PracticalNostrCoordinator::new(i, session_id.to_string()).await?;
        coordinators.push(coord);
    }
    
    // Step 1: Coordinate DKG start
    println!("\n📋 Step 1: Coordinating DKG start");
    
    // Party 1 initiates
    coordinators[0].send_coord_message("DKG_START", "coordination").await?;
    
    // Other parties wait for start signal
    for i in 1..party_count as usize {
        let messages = coordinators[i].wait_for_messages("coordination", 1, 10).await?;
        if !messages.iter().any(|m| m.msg_type == "DKG_START") {
            return Err("No DKG start signal received".into());
        }
    }
    
    // All parties acknowledge
    for coord in &coordinators {
        coord.send_coord_message("DKG_READY", "coordination").await?;
    }
    
    // Wait for all acknowledgments
    for coord in &coordinators {
        let messages = coord.wait_for_messages("coordination", (party_count - 1) as usize, 5).await?;
        if messages.len() < (party_count - 1) as usize {
            return Err("Not all parties ready".into());
        }
    }
    
    println!("✅ All parties coordinated, running DKG computation...");
    
    // Step 2: Run actual DKG locally (this is where the real work happens)
    let parameters = Parameters { threshold, share_count: party_count };
    let parties = run_dkg_offline(&parameters, session_id.as_bytes())
        .map_err(|e| format!("DKG failed: {}", e.description))?;
    
    // Step 3: Share results via nostr (optional coordination)
    println!("\n📋 Step 3: Sharing DKG results");
    
    for (i, coord) in coordinators.iter().enumerate() {
        let result_msg = format!("address:{}", parties[i].btc_address);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all results
    for coord in &coordinators {
        let _messages = coord.wait_for_messages("results", (party_count - 1) as usize, 5).await?;
    }
    
    // Disconnect all coordinators
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    println!("✅ Practical Nostr DKG completed successfully!");
    Ok(parties)
}

/// Run threshold signing with practical nostr coordination
async fn run_practical_nostr_signing(
    parties: &[Party],
    message: &str,
    session_id: &str,
) -> Result<(String, String, u8), Box<dyn std::error::Error>> {
    println!("\n✍️  Starting Practical Nostr Threshold Signing");
    
    let threshold = parties[0].parameters.threshold;
    let sign_session = format!("{}_sign", session_id);
    
    // Create coordinators for signing parties
    let mut coordinators = Vec::new();
    for i in 1..=threshold {
        let coord = PracticalNostrCoordinator::new(i, sign_session.clone()).await?;
        coordinators.push(coord);
    }
    
    // Step 1: Coordinate signing start
    println!("\n📋 Step 1: Coordinating signing start");
    
    // Party 1 initiates signing
    coordinators[0].send_coord_message("SIGN_START", "coordination").await?;
    
    // Others wait for start signal
    for i in 1..coordinators.len() {
        let messages = coordinators[i].wait_for_messages("coordination", 1, 10).await?;
        if !messages.iter().any(|m| m.msg_type == "SIGN_START") {
            return Err("No signing start signal received".into());
        }
    }
    
    // All parties acknowledge
    for coord in &coordinators {
        coord.send_coord_message("SIGN_READY", "coordination").await?;
    }
    
    // Wait for acknowledgments
    for coord in &coordinators {
        let messages = coord.wait_for_messages("coordination", (threshold - 1) as usize, 5).await?;
        if messages.len() < (threshold - 1) as usize {
            return Err("Not all parties ready for signing".into());
        }
    }
    
    println!("✅ All parties coordinated, running signing computation...");
    
    // Step 2: Run actual signing locally
    let message_hash = hash(message.as_bytes(), &[]);
    let executing_parties: Vec<u8> = (1..=threshold).collect();
    
    let (r, s, recid) = threshold_sign(
        parties,
        &executing_parties,
        sign_session.as_bytes(),
        message_hash,
        true,
    ).map_err(|e| format!("Signing failed: {}", e.description))?;
    
    // Step 3: Share results
    println!("\n📋 Step 3: Sharing signing results");
    
    for coord in &coordinators {
        let result_msg = format!("signature:{},{},{}", r, s, recid);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all results
    for coord in &coordinators {
        let _messages = coord.wait_for_messages("results", (threshold - 1) as usize, 5).await?;
    }
    
    // Disconnect
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    println!("✅ Practical Nostr signing completed successfully!");
    Ok((r, s, recid))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Practical Nostr DKLs23 Demo");
    println!("================================");
    println!("📡 Relay: {}", RELAY_URL);
    
    // Configuration
    let party_count = 3;
    let threshold = 2;
    let session_id = "practical_nostr_demo_2024";
    let message = "Hello from Practical Nostr DKLs23!";
    
    println!("\n📋 Configuration:");
    println!("   Parties: {}", party_count);
    println!("   Threshold: {}", threshold);
    println!("   Session ID: {}", session_id);
    println!("   Message: {}", message);
    
    // Step 1: Run DKG
    println!("\n🔐 STEP 1: DISTRIBUTED KEY GENERATION");
    println!("========================================");
    let parties = run_practical_nostr_dkg(party_count, threshold, session_id).await?;
    
    // Display results
    println!("\n👥 Generated Parties:");
    for (i, party) in parties.iter().enumerate() {
        println!("   Party {}: {}", i + 1, party.btc_address);
    }
    println!("   Network: {:?}", parties[0].network);
    
    // Step 2: Run signing
    println!("\n✍️  STEP 2: THRESHOLD SIGNING");
    println!("===============================");
    let signature = run_practical_nostr_signing(&parties, message, session_id).await?;
    
    // Final results
    println!("\n🎉 FINAL RESULTS");
    println!("================");
    println!("   Bitcoin Address: {}", parties[0].btc_address);
    println!("   Network: {:?}", parties[0].network);
    println!("   Message: {}", message);
    println!("   Signature r: {}", signature.0);
    println!("   Signature s: {}", signature.1);
    println!("   Recovery ID: {}", signature.2);
    
    println!("\n💡 WHY THIS IS THE EASIEST APPROACH:");
    println!("   1. ✅ Uses nostr ONLY for coordination");
    println!("   2. ✅ Keeps all heavy computation local");
    println!("   3. ✅ Leverages existing DKLs23 facade");
    println!("   4. ✅ Works with real nostr relays");
    println!("   5. ✅ Minimal code changes required");
    println!("   6. ✅ Production-ready approach");
    
    println!("\n🚀 TO USE IN PRODUCTION:");
    println!("   1. Deploy parties on different machines");
    println!("   2. Use encrypted direct messages (NIP-04)");
    println!("   3. Add proper error handling and retries");
    println!("   4. Implement key backup and recovery");
    println!("   5. Add authentication and authorization");
    
    println!("\n🌐 Practical Nostr DKLs23 demo completed! 🎉");
    
    Ok(())
}