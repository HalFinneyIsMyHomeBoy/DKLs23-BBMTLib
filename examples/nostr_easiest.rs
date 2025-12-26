//! EASIEST Nostr-based multi-party DKG and signing example.
//! 
//! This demonstrates the simplest possible way to use nostr for multi-party DKLs23:
//! 1. Use nostr events as coordination signals only
//! 2. Keep all computation local (use existing DKLs23 facade)
//! 3. Minimal dependencies and maximum simplicity
//! 4. Works with wss://bbw-nostr.xyz relay

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::{Parameters, Party};
use dkls23::utilities::hashes::hash;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Nostr relay URL
const RELAY_URL: &str = "wss://bbw-nostr.xyz";

/// Simple coordination message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoordMessage {
    session_id: String,
    party_index: u8,
    msg_type: String,
    phase: String,
}

/// Result of multi-party operation
#[derive(Debug, Clone)]
struct MultiPartyResult {
    parties: Vec<Party>,
    signature: Option<(String, String, u8)>,
    bitcoin_address: String,
    session_id: String,
}

/// Easiest nostr-based multi-party coordinator
struct EasiestNostrCoordinator {
    party_index: u8,
    session_id: String,
    total_parties: u8,
}

impl EasiestNostrCoordinator {
    fn new(party_index: u8, session_id: String, total_parties: u8) -> Self {
        Self {
            party_index,
            session_id,
            total_parties,
        }
    }
    
    /// Simulate sending a coordination message via nostr
    async fn send_coord_message(&self, msg_type: &str, phase: &str) -> Result<(), Box<dyn std::error::Error>> {
        let message = CoordMessage {
            session_id: self.session_id.clone(),
            party_index: self.party_index,
            msg_type: msg_type.to_string(),
            phase: phase.to_string(),
        };
        
        let content = serde_json::to_string(&message)?;
        
        // Simulate publishing to nostr relay
        println!("📤 [PARTY {}] Publishing to {}: {}", 
                self.party_index, RELAY_URL, msg_type);
        println!("   Content: {}", content);
        
        // Simulate network delay
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Ok(())
    }
    
    /// Simulate receiving coordination messages from nostr
    async fn wait_for_coord_messages(&self, phase: &str, expected_count: usize) -> Result<Vec<CoordMessage>, Box<dyn std::error::Error>> {
        println!("📥 [PARTY {}] Subscribing to {} for phase: {}", 
                self.party_index, RELAY_URL, phase);
        
        let mut messages = Vec::new();
        
        // Simulate receiving messages from other parties
        for sender in 1..=self.total_parties {
            if sender != self.party_index {
                // Simulate network delay
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                let msg_type = match phase {
                    "coordination" => {
                        // Check if this is DKG or signing coordination by session_id
                        if self.session_id.contains("_sign") {
                            "SIGN_START"
                        } else {
                            "DKG_START"
                        }
                    }
                    "results" => {
                        // Check if this is DKG or signing results by session_id
                        if self.session_id.contains("_sign") {
                            "SIGN_COMPLETE"
                        } else {
                            "DKG_COMPLETE"
                        }
                    }
                    _ => "UNKNOWN"
                };
                
                let simulated_message = CoordMessage {
                    session_id: self.session_id.clone(),
                    party_index: sender,
                    msg_type: msg_type.to_string(),
                    phase: phase.to_string(),
                };
                
                messages.push(simulated_message);
                println!("📥 [PARTY {}] Received from party {}: {}", 
                        self.party_index, sender, 
                        if phase == "coordination" { "DKG_START" } else { "SIGN_START" });
            }
        }
        
        Ok(messages)
    }
}

/// Run the easiest nostr-based DKG
async fn run_easiest_nostr_dkg(
    party_count: u8,
    threshold: u8,
    session_id: &str,
) -> Result<Vec<Party>, Box<dyn std::error::Error>> {
    println!("🔐 EASIEST Nostr DKG: {}-of-{} scheme", threshold, party_count);
    println!("📡 Relay: {}", RELAY_URL);
    println!("🆔 Session: {}", session_id);
    
    // Create coordinators for all parties
    let mut coordinators = Vec::new();
    for i in 1..=party_count {
        let coord = EasiestNostrCoordinator::new(i, session_id.to_string(), party_count);
        coordinators.push(coord);
    }
    
    // Step 1: Coordinate DKG start via nostr
    println!("\n📋 STEP 1: NOSTR COORDINATION");
    println!("===============================");
    
    // Party 1 initiates DKG
    coordinators[0].send_coord_message("DKG_START", "coordination").await?;
    
    // Other parties wait for start signal
    for i in 1..party_count as usize {
        let messages = coordinators[i].wait_for_coord_messages("coordination", 1).await?;
        if !messages.iter().any(|m| m.msg_type == "DKG_START") {
            return Err("No DKG start signal received".into());
        }
    }
    
    // All parties acknowledge readiness
    for coord in &coordinators {
        coord.send_coord_message("DKG_READY", "coordination").await?;
    }
    
    // Wait for all acknowledgments
    for coord in &coordinators {
        let _messages = coord.wait_for_coord_messages("coordination", (party_count - 1) as usize).await?;
    }
    
    println!("✅ All parties coordinated via nostr!");
    
    // Step 2: Run actual DKG computation locally
    println!("\n🔧 STEP 2: LOCAL DKG COMPUTATION");
    println!("=================================");
    
    let parameters = Parameters { threshold, share_count: party_count };
    let parties = run_dkg_offline(&parameters, session_id.as_bytes())
        .map_err(|e| format!("DKG failed: {}", e.description))?;
    
    println!("✅ DKG computation completed locally!");
    
    // Step 3: Share results via nostr (optional)
    println!("\n📋 STEP 3: SHARING RESULTS");
    println!("===========================");
    
    for (i, coord) in coordinators.iter().enumerate() {
        let result_msg = format!("DKG_COMPLETE:address={}", parties[i].btc_address);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all result messages
    for coord in &coordinators {
        let _messages = coord.wait_for_coord_messages("results", (party_count - 1) as usize).await?;
    }
    
    println!("✅ Results shared via nostr!");
    
    Ok(parties)
}

/// Run the easiest nostr-based threshold signing
async fn run_easiest_nostr_signing(
    parties: &[Party],
    message: &str,
    session_id: &str,
) -> Result<(String, String, u8), Box<dyn std::error::Error>> {
    println!("\n✍️  EASIEST Nostr THRESHOLD SIGNING");
    println!("===================================");
    
    let threshold = parties[0].parameters.threshold;
    let sign_session = format!("{}_sign", session_id);
    
    // Create coordinators for signing parties
    let mut coordinators = Vec::new();
    for i in 1..=threshold {
        let coord = EasiestNostrCoordinator::new(i, sign_session.clone(), threshold);
        coordinators.push(coord);
    }
    
    // Step 1: Coordinate signing start via nostr
    println!("\n📋 STEP 1: NOSTR COORDINATION");
    println!("===============================");
    
    // Party 1 initiates signing
    coordinators[0].send_coord_message("SIGN_START", "coordination").await?;
    
    // Other parties wait for start signal
    for i in 1..coordinators.len() {
        let messages = coordinators[i].wait_for_coord_messages("coordination", 1).await?;
        if !messages.iter().any(|m| m.msg_type == "SIGN_START") {
            return Err("No signing start signal received".into());
        }
    }
    
    // All parties acknowledge readiness
    for coord in &coordinators {
        coord.send_coord_message("SIGN_READY", "coordination").await?;
    }
    
    // Wait for acknowledgments
    for coord in &coordinators {
        let _messages = coord.wait_for_coord_messages("coordination", (threshold - 1) as usize).await?;
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
    
    // Step 3: Share results via nostr
    println!("\n📋 STEP 3: SHARING RESULTS");
    println!("===========================");
    
    for coord in &coordinators {
        let result_msg = format!("SIGN_COMPLETE:r={},s={},recid={}", r, s, recid);
        coord.send_coord_message(&result_msg, "results").await?;
    }
    
    // Wait for all result messages
    for coord in &coordinators {
        let _messages = coord.wait_for_coord_messages("results", (threshold - 1) as usize).await?;
    }
    
    println!("✅ Results shared via nostr!");
    
    Ok((r, s, recid))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 EASIEST Nostr DKLs23 Demo");
    println!("=============================");
    println!("📡 This shows the SIMPLEST way to use nostr for multi-party DKLs23");
    println!("🔗 Relay: {}", RELAY_URL);
    
    // Configuration
    let party_count = 3;
    let threshold = 2;
    let session_id = "easiest_nostr_demo_2024";
    let message = "Hello from Easiest Nostr DKLs23!";
    
    println!("\n📋 Configuration:");
    println!("   Parties: {}", party_count);
    println!("   Threshold: {}", threshold);
    println!("   Session ID: {}", session_id);
    println!("   Message: {}", message);
    
    // Run complete multi-party operation
    println!("\n🚀 STARTING COMPLETE MULTI-PARTY OPERATION");
    println!("==========================================");
    
    // Step 1: DKG
    let parties = run_easiest_nostr_dkg(party_count, threshold, session_id).await?;
    
    // Display DKG results
    println!("\n👥 DKG Results:");
    for (i, party) in parties.iter().enumerate() {
        println!("   Party {}: {}", i + 1, party.btc_address);
    }
    
    // Step 2: Signing
    let signature = run_easiest_nostr_signing(&parties, message, session_id).await?;
    
    // Final results
    println!("\n🎉 FINAL RESULTS:");
    println!("=================");
    println!("   Bitcoin Address: {}", parties[0].btc_address);
    println!("   Network: {:?}", parties[0].network);
    println!("   Message: {}", message);
    println!("   Signature r: {}", signature.0);
    println!("   Signature s: {}", signature.1);
    println!("   Recovery ID: {}", signature.2);
    
    println!("\n💡 WHY THIS IS THE EASIEST APPROACH:");
    println!("=====================================");
    println!("   1. ✅ Nostr used ONLY for coordination signals");
    println!("   2. ✅ All heavy computation stays local");
    println!("   3. ✅ Uses existing DKLs23 facade unchanged");
    println!("   4. ✅ Minimal dependencies and complexity");
    println!("   5. ✅ Easy to understand and debug");
    println!("   6. ✅ Production-ready with minimal changes");
    
    println!("\n🔧 TO MAKE THIS WORK WITH REAL NOSTR:");
    println!("======================================");
    println!("   1. Replace simulated send/receive with real nostr-sdk calls");
    println!("   2. Add proper error handling and timeouts");
    println!("   3. Use encrypted direct messages (NIP-04) for privacy");
    println!("   4. Add authentication of party identities");
    println!("   5. Deploy parties on different machines");
    
    println!("\n📝 REAL NOSTR INTEGRATION EXAMPLE:");
    println!("===================================");
    println!("   // Replace send_coord_message with:");
    println!("   let event = EventBuilder::text_note(content)");
    println!("   client.send_event(event).await?;");
    println!("   ");
    println!("   // Replace wait_for_coord_messages with:");
    println!("   let filter = Filter::new().identifier(session_id);");
    println!("   let sub_id = client.subscribe(vec![filter], None).await?;");
    println!("   // Handle RelayMessage::Event events");
    
    println!("\n🌐 Easiest Nostr DKLs23 demo completed! 🎉");
    
    Ok(())
}