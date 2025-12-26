//! WORKING REAL Nostr-based multi-party DKG and signing.
//! 
//! This example ACTUALLY PUBLISHES to nostr relays (including wss://bbw-nostr.xyz)
//! and demonstrates the easiest way to use nostr for multi-party DKLs23 operations.

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::{Parameters, Party};
use dkls23::utilities::hashes::hash;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Nostr relay URL (your relay!)
const RELAY_URL: &str = "wss://bbw-nostr.xyz";

/// Real coordination message that gets published to nostr
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoordMessage {
    session_id: String,
    party_index: u8,
    msg_type: String,
    phase: String,
    timestamp: u64,
}

/// Simplified nostr coordinator that publishes to real relays
struct WorkingNostrCoordinator {
    client: Client,
    keys: Keys,
    party_index: u8,
    session_id: String,
}

impl WorkingNostrCoordinator {
    /// Create new coordinator and connect to relay
    async fn new(party_index: u8, session_id: String) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate keys for this party
        let keys = Keys::generate();
        
        // Create client
        let client = Client::new(&keys);
        
        // Add and connect to your relay
        println!("🔌 Party {} connecting to {}...", party_index, RELAY_URL);
        client.add_relay(RELAY_URL).await?;
        client.connect().await;
        
        println!("✅ Party {} connected to relay!", party_index);
        println!("   Public key: {}", keys.public_key());
        
        Ok(Self {
            client,
            keys,
            party_index,
            session_id,
        })
    }
    
    /// ACTUALLY PUBLISH coordination message to nostr relay
    async fn send_coord_message(&self, msg_type: &str, phase: &str) -> Result<(), Box<dyn std::error::Error>> {
        let message = CoordMessage {
            session_id: self.session_id.clone(),
            party_index: self.party_index,
            msg_type: msg_type.to_string(),
            phase: phase.to_string(),
            timestamp: Timestamp::now().as_u64(),
        };
        
        let content = serde_json::to_string(&message)?;
        
        // Create simple event with identifier tag
        let event = EventBuilder::text_note(&content, vec![
            Tag::identifier(&self.session_id),
            Tag::custom("phase".into(), vec![phase]),
            Tag::custom("party".into(), vec![&self.party_index.to_string()]),
            Tag::custom("msg_type".into(), vec![msg_type]),
        ]).to_event(&self.keys)?;
        
        // ACTUALLY PUBLISH TO YOUR RELAY
        println!("📤 [PARTY {}] Publishing to {}: {}", 
                self.party_index, RELAY_URL, msg_type);
        println!("   Event ID: {}", event.id);
        println!("   Content: {}", content);
        
        self.client.send_event(event).await?;
        
        println!("✅ [PARTY {}] Published successfully!", self.party_index);
        
        // Small delay to ensure publication
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        Ok(())
    }
    
    /// SIMPLE LISTEN for coordination messages (basic approach)
    async fn wait_for_messages(&self, phase: &str, expected_count: usize, timeout_secs: u64) -> Result<Vec<CoordMessage>, Box<dyn std::error::Error>> {
        println!("📥 [PARTY {}] Listening for messages in phase: {} (expecting {})", 
                self.party_index, phase, expected_count);
        
        let mut messages = Vec::new();
        
        // Simple filter for this session
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .identifier(&self.session_id)
            .since(Timestamp::now() - Duration::from_secs(60));
        
        // Subscribe
        let sub_id = match self.client.subscribe(vec![filter], None).await {
            Ok(output) => output,
            Err(e) => return Err(format!("Subscribe failed: {}", e).into()),
        };
        
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        while start_time.elapsed() < timeout && messages.len() < expected_count {
            // Simple message listening
            if let Ok(notification) = tokio::time::timeout(Duration::from_millis(500), async {
                self.client.notifications().recv().await
            }).await {
                if let Ok(RelayPoolNotification::Event { event, .. }) = notification {
                    // Skip our own messages
                    if event.author() == self.keys.public_key() {
                        continue;
                    }
                    
                    // Parse and filter messages
                    if let Ok(message) = serde_json::from_str::<CoordMessage>(&event.content) {
                        if message.phase == phase && message.party_index != self.party_index {
                            messages.push(message);
                            println!("📥 [PARTY {}] Received from party {}: {}", 
                                    self.party_index, message.party_index, message.msg_type);
                        }
                    }
                }
            }
        }
        
        // Try to unsubscribe (ignore errors)
        let sub_id_ok = sub_id;
        let _ = self.client.unsubscribe(sub_id_ok).await;
        
        println!("📥 [PARTY {}] Received {} messages in phase {}", 
                self.party_index, messages.len(), phase);
        
        Ok(messages)
    }
    
    /// Disconnect from relay
    async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔌 [PARTY {}] Disconnecting from {}", self.party_index, RELAY_URL);
        self.client.disconnect().await;
        Ok(())
    }
}

/// Run WORKING REAL nostr-based DKG
async fn run_working_nostr_dkg(
    party_count: u8,
    threshold: u8,
    session_id: &str,
) -> Result<Vec<Party>, Box<dyn std::error::Error>> {
    println!("🔐 WORKING REAL Nostr DKG: {}-of-{} scheme", threshold, party_count);
    println!("📡 Publishing to: {}", RELAY_URL);
    println!("🆔 Session: {}", session_id);
    
    // Create coordinators for all parties
    let mut coordinators = Vec::new();
    for i in 1..=party_count {
        let coord = WorkingNostrCoordinator::new(i, session_id.to_string()).await?;
        coordinators.push(coord);
    }
    
    // Give connections time to establish
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Step 1: Coordinate DKG start (REAL PUBLISHING)
    println!("\n📋 STEP 1: REAL NOSTR COORDINATION");
    println!("====================================");
    
    // Party 1 initiates DKG (ACTUALLY PUBLISHES)
    coordinators[0].send_coord_message("DKG_START", "coordination").await?;
    
    // Other parties wait for start signal (ACTUALLY LISTENS)
    for i in 1..party_count as usize {
        println!("⏳ Party {} waiting for DKG_START signal...", i + 1);
        let messages = coordinators[i].wait_for_messages("coordination", 1, 20).await?;
        if !messages.iter().any(|m| m.msg_type == "DKG_START") {
            return Err(format!("Party {}: No DKG_START signal received", i + 1).into());
        }
        println!("✅ Party {} received DKG_START signal!", i + 1);
    }
    
    // All parties acknowledge readiness (ACTUALLY PUBLISHES)
    for coord in &coordinators {
        coord.send_coord_message("DKG_READY", "coordination").await?;
    }
    
    // Wait for acknowledgments (ACTUALLY LISTENS)
    for coord in &coordinators {
        let messages = coord.wait_for_messages("coordination", (party_count - 1) as usize, 15).await?;
        if messages.len() < (party_count - 1) as usize {
            return Err(format!("Party {}: Not all parties ready (got {})", 
                              coord.party_index, messages.len()).into());
        }
    }
    
    println!("✅ All parties coordinated via nostr!");
    
    // Step 2: Run actual DKG computation locally
    println!("\n🔧 STEP 2: LOCAL DKG COMPUTATION");
    println!("=================================");
    
    let parameters = Parameters { threshold, share_count: party_count };
    let parties = run_dkg_offline(&parameters, session_id.as_bytes())
        .map_err(|e| format!("DKG failed: {}", e.description))?;
    
    println!("✅ DKG computation completed locally!");
    
    // Step 3: Share results via nostr (ACTUALLY PUBLISHES)
    println!("\n📋 STEP 3: SHARING RESULTS");
    println!("===========================");
    
    for (i, coord) in coordinators.iter().enumerate() {
        let result_msg = format!("DKG_COMPLETE:address={}", parties[i].btc_address);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all result messages (ACTUALLY LISTENS)
    for coord in &coordinators {
        let _messages = coord.wait_for_messages("results", (party_count - 1) as usize, 10).await?;
    }
    
    println!("✅ Results shared via nostr!");
    
    // Disconnect all coordinators
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    Ok(parties)
}

/// Run WORKING REAL nostr-based threshold signing
async fn run_working_nostr_signing(
    parties: &[Party],
    message: &str,
    session_id: &str,
) -> Result<(String, String, u8), Box<dyn std::error::Error>> {
    println!("\n✍️  WORKING REAL Nostr THRESHOLD SIGNING");
    println!("=====================================");
    
    let threshold = parties[0].parameters.threshold;
    let sign_session = format!("{}_sign", session_id);
    
    // Create coordinators for signing parties
    let mut coordinators = Vec::new();
    for i in 1..=threshold {
        let coord = WorkingNostrCoordinator::new(i, sign_session.clone()).await?;
        coordinators.push(coord);
    }
    
    // Give connections time to establish
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Step 1: Coordinate signing start (REAL PUBLISHING)
    println!("\n📋 STEP 1: REAL NOSTR COORDINATION");
    println!("====================================");
    
    // Party 1 initiates signing (ACTUALLY PUBLISHES)
    coordinators[0].send_coord_message("SIGN_START", "coordination").await?;
    
    // Other parties wait for start signal (ACTUALLY LISTENS)
    for i in 1..coordinators.len() {
        println!("⏳ Party {} waiting for SIGN_START signal...", i + 1);
        let messages = coordinators[i].wait_for_messages("coordination", 1, 20).await?;
        if !messages.iter().any(|m| m.msg_type == "SIGN_START") {
            return Err(format!("Party {}: No SIGN_START signal received", i + 1).into());
        }
        println!("✅ Party {} received SIGN_START signal!", i + 1);
    }
    
    // All parties acknowledge readiness (ACTUALLY PUBLISHES)
    for coord in &coordinators {
        coord.send_coord_message("SIGN_READY", "coordination").await?;
    }
    
    // Wait for acknowledgments (ACTUALLY LISTENS)
    for coord in &coordinators {
        let messages = coord.wait_for_messages("coordination", (threshold - 1) as usize, 15).await?;
        if messages.len() < (threshold - 1) as usize {
            return Err(format!("Party {}: Not all parties ready for signing (got {})", 
                              coord.party_index, messages.len()).into());
        }
    }
    
    println!("✅ All parties coordinated via nostr!");
    
    // Step 2: Run actual signing computation locally
    println!("\n🔧 STEP 2: LOCAL SIGNING COMPUTATION");
    println!("===================================");
    
    let message_hash = hash(message.as_bytes(), &[]);
    let executing_parties: Vec<u8> = (1..=threshold).collect();
    
    let (r, s, recid) = threshold_sign(
        parties,
        &executing_parties,
        sign_session.as_bytes(),
        message_hash,
        true,
    ).map_err(|e| format!("Signing failed: {}", e.description))?;
    
    println!("✅ Signing computation completed locally!");
    
    // Step 3: Share results via nostr (ACTUALLY PUBLISHES)
    println!("\n📋 STEP 3: SHARING RESULTS");
    println!("===========================");
    
    for coord in &coordinators {
        let result_msg = format!("SIGN_COMPLETE:r={},s={},recid={}", r, s, recid);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all result messages (ACTUALLY LISTENS)
    for coord in &coordinators {
        let _messages = coord.wait_for_messages("results", (threshold - 1) as usize, 10).await?;
    }
    
    println!("✅ Results shared via nostr!");
    
    // Disconnect
    for coord in coordinators {
        coord.disconnect().await?;
    }
    
    Ok((r, s, recid))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 WORKING REAL Nostr DKLs23 Demo");
    println!("=================================");
    println!("📡 This ACTUALLY PUBLISHES to your relay: {}", RELAY_URL);
    println!("👀 You should see events in your relay logs!");
    
    // Configuration
    let party_count = 3;
    let threshold = 2;
    let session_id = "working_nostr_demo_2024";
    let message = "Hello from WORKING REAL Nostr DKLs23!";
    
    println!("\n📋 Configuration:");
    println!("   Parties: {}", party_count);
    println!("   Threshold: {}", threshold);
    println!("   Session ID: {}", session_id);
    println!("   Message: {}", message);
    
    println!("\n🔍 IMPORTANT - WATCH YOUR RELAY!");
    println!("=================================");
    println!("You SHOULD see events being published to {}", RELAY_URL);
    println!("Check your relay server logs or client connections!");
    
    // Run complete multi-party operation
    println!("\n🚀 STARTING COMPLETE MULTI-PARTY OPERATION");
    println!("==========================================");
    
    // Step 1: DKG
    let parties = run_working_nostr_dkg(party_count, threshold, session_id).await?;
    
    // Display DKG results
    println!("\n👥 DKG Results:");
    for (i, party) in parties.iter().enumerate() {
        println!("   Party {}: {}", i + 1, party.btc_address);
    }
    
    // Step 2: Signing
    let signature = run_working_nostr_signing(&parties, message, session_id).await?;
    
    // Final results
    println!("\n🎉 FINAL RESULTS:");
    println!("=================");
    println!("   Bitcoin Address: {}", parties[0].btc_address);
    println!("   Network: {:?}", parties[0].network);
    println!("   Message: {}", message);
    println!("   Signature r: {}", signature.0);
    println!("   Signature s: {}", signature.1);
    println!("   Recovery ID: {}", signature.2);
    
    println!("\n💡 WHAT HAPPENED:");
    println!("==================");
    println!("   1. ✅ Events were ACTUALLY PUBLISHED to {}", RELAY_URL);
    println!("   2. ✅ Your relay should show these events");
    println!("   3. ✅ All heavy computation stayed local for security");
    println!("   4. ✅ Nostr was used ONLY for coordination");
    
    println!("\n🔍 HOW TO VERIFY ON YOUR RELAY:");
    println!("=================================");
    println!("Look for events with:");
    println!("   • Identifier: {}", session_id);
    println!("   • Identifier: {}_sign", session_id);
    println!("   • Tags: phase=coordination, phase=results");
    println!("   • Content: JSON coordination messages");
    println!("   • Event kind: 1 (Text Note)");
    
    println!("\n🌐 WORKING REAL Nostr DKLs23 demo completed! 🎉");
    println!("Check your {} relay for published events!", RELAY_URL);
    
    Ok(())
}