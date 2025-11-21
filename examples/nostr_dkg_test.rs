//! Nostr-based Distributed DKG Test using NIP-04
//!
//! This example demonstrates distributed DKG where parties communicate via Nostr relays
//! using NIP-04 encrypted direct messages (event kind 4).
//! All messages are encrypted end-to-end using NIP-04 encryption.
//!
//! Uses nostr-sdk from https://github.com/rust-nostr/nostr

use dkls23::protocols::dkg_distributed::*;
use dkls23::protocols::Parameters;
use dkls23::facade::threshold_sign;
use dkls23::utilities::hashes::hash;
use serde_json::Value;
use std::time::Duration;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const NOSTR_RELAY: &str = "wss://bbw-nostr.xyz";

// NIP-04 uses event kind 4 for encrypted direct messages

/// Nostr client wrapper for DKG communication using NIP-04
struct NostrRelayClient {
    client: nostr_sdk::Client,
    keys: nostr_sdk::Keys,
    session_id: String,
    party_index: u8,
    received_messages: Arc<RwLock<HashMap<String, Vec<Value>>>>,
    // Store other parties' public keys for encryption
    party_pubkeys: Arc<RwLock<HashMap<u8, nostr_sdk::PublicKey>>>,
}

impl NostrRelayClient {
    /// Create a new Nostr client for a party
    async fn new(party_index: u8, session_id: &[u8]) -> Result<Self, String> {
        // Generate a unique keypair for this party session
        let keys = nostr_sdk::Keys::generate();
        
        // Create client - keys implement IntoNostrSigner
        let client = nostr_sdk::Client::new(keys.clone());
        
        // Add and connect to relay
        client.add_relay(NOSTR_RELAY).await
            .map_err(|e| format!("Failed to add relay: {}", e))?;
        
        client.connect().await;

        let session_id_hex = hex::encode(session_id);
        
        Ok(Self {
            client,
            keys,
            session_id: session_id_hex,
            party_index,
            received_messages: Arc::new(RwLock::new(HashMap::new())),
            party_pubkeys: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Exchange public keys with other parties
    /// This must be called before sending encrypted messages
    async fn exchange_pubkeys(&self, share_count: u8) -> Result<(), String> {
        // Add our own public key immediately
        {
            let mut pubkeys = self.party_pubkeys.write().await;
            pubkeys.insert(self.party_index, self.keys.public_key());
        }
        
        // Publish our public key as a kind 0 (metadata) event with session info
        let pubkey_json = serde_json::json!({
            "party_index": self.party_index,
            "pubkey": self.keys.public_key().to_string(),
            "session_id": self.session_id,
        });
        
        let content = serde_json::to_string(&pubkey_json)
            .map_err(|e| format!("Serialization error: {}", e))?;
        
        let mut tags = nostr_sdk::Tags::new();
        tags.push(nostr_sdk::Tag::parse(format!("d:{}:{}", self.session_id, self.party_index).chars())
            .map_err(|e| format!("Failed to parse tag: {}", e))?);
        
        // Use kind 0 for metadata
        let event_builder = nostr_sdk::EventBuilder::new(
            nostr_sdk::Kind::Metadata,
            content,
        )
        .tags(tags);
        
        let event = event_builder.sign(&self.keys).await
            .map_err(|e| format!("Failed to sign pubkey event: {:?}", e))?;
        
        self.client.send_event(&event).await
            .map_err(|e| format!("Failed to publish pubkey: {}", e))?;
        
        // Wait for other parties' public keys
        let filter = nostr_sdk::Filter::new()
            .kind(nostr_sdk::Kind::Metadata);
        
        let _ = self.client.subscribe(filter, None).await;
        
        let start = std::time::Instant::now();
        let party_pubkeys = self.party_pubkeys.clone();
        let session_id = self.session_id.clone();
        let client_clone = self.client.clone();
        
        // Spawn task to collect pubkeys
        tokio::spawn(async move {
            let mut notifications = client_clone.notifications();
            while let Ok(notification) = notifications.recv().await {
                if let nostr_sdk::RelayPoolNotification::Event { event, .. } = notification {
                    if let Ok(pubkey_data) = serde_json::from_str::<Value>(&event.content) {
                        if let (Some(party_idx), Some(pubkey_str)) = (
                            pubkey_data["party_index"].as_u64(),
                            pubkey_data["session_id"].as_str(),
                        ) {
                            if pubkey_str == session_id {
                                if let Some(pubkey_str) = pubkey_data["pubkey"].as_str() {
                                    if let Ok(pubkey) = nostr_sdk::PublicKey::from_hex(pubkey_str) {
                                        let mut pubkeys = party_pubkeys.write().await;
                                        pubkeys.insert(party_idx as u8, pubkey);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        
        // Wait for all pubkeys (including our own)
        loop {
            let pubkeys = self.party_pubkeys.read().await;
            if pubkeys.len() >= share_count as usize {
                return Ok(());
            }
            
            if start.elapsed().as_secs() > 30 {
                return Err(format!(
                    "Timeout waiting for public keys. Got {}/{}",
                    pubkeys.len(),
                    share_count
                ));
            }
            
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    /// Post a message to the Nostr relay using NIP-04 encryption
    async fn post_message(
        &self,
        phase: u8,
        sender: u8,
        receiver: u8,
        data: &Value,
    ) -> Result<(), String> {
        // Get receiver's public key
        let receiver_pubkey = {
            let pubkeys = self.party_pubkeys.read().await;
            pubkeys.get(&receiver)
                .ok_or_else(|| format!("Receiver {} public key not found", receiver))?
                .clone()
        };

        // Create message payload with phase and session info
        let message_payload = serde_json::json!({
            "session_id": self.session_id,
            "phase": phase,
            "sender": sender,
            "receiver": receiver,
            "data": data,
        });
        
        let plaintext = serde_json::to_string(&message_payload)
            .map_err(|e| format!("Serialization error: {}", e))?;

        // Encrypt using NIP-04
        use nostr::nips::nip04;
        let encrypted_content = nip04::encrypt(
            &self.keys.secret_key(),
            &receiver_pubkey,
            &plaintext,
        )
        .map_err(|e| format!("Encryption error: {}", e))?;

        // Create tags for NIP-04
        let mut tags = nostr_sdk::Tags::new();
        // 'p' tag with receiver's public key (required for NIP-04)
        tags.push(nostr_sdk::Tag::parse(format!("p:{}", receiver_pubkey.to_hex()).chars())
            .map_err(|e| format!("Failed to parse p tag: {}", e))?);
        // 'd' tag for filtering: d:session_id:phase:sender:receiver
        let d_tag_value = format!(
            "{}:{}:{}:{}",
            self.session_id, phase, sender, receiver
        );
        tags.push(nostr_sdk::Tag::parse(format!("d:{}", d_tag_value).chars())
            .map_err(|e| format!("Failed to parse d tag: {}", e))?);

        // Build and publish NIP-04 encrypted direct message (kind 4)
        let event_builder = nostr_sdk::EventBuilder::new(
            nostr_sdk::Kind::EncryptedDirectMessage,
            encrypted_content,
        )
        .tags(tags);

        let event = event_builder.sign(&self.keys).await
            .map_err(|e| format!("Failed to sign event: {:?}", e))?;

        // Publish event
        self.client
            .send_event(&event)
            .await
            .map_err(|e| format!("Failed to publish event: {}", e))?;
        
        eprintln!("   ✅ Party {} sent NIP-04 message to party {} (phase {})", 
            self.party_index, receiver, phase);

        Ok(())
    }

    /// Start listening for messages in a specific phase (NIP-04 encrypted)
    async fn start_listening(&self, phase: u8, receiver: u8) -> Result<(), String> {
        // Create filter for NIP-04 encrypted direct messages (kind 4)
        // For NIP-04, the 'p' tag contains the recipient's public key
        // Filter by kind only - we'll check the 'p' tag in the handler
        let filter = nostr_sdk::Filter::new()
            .kind(nostr_sdk::Kind::EncryptedDirectMessage);

        let received = self.received_messages.clone();
        let client_clone = self.client.clone();
        let session_id = self.session_id.clone();
        let keys = self.keys.clone();
        let party_index = self.party_index;
        let our_pubkey_hex = self.keys.public_key().to_hex();
        
        // Subscribe to events
        eprintln!("   🔍 Party {} subscribing to NIP-04 messages (phase {}) with pubkey {}", 
            party_index, phase, our_pubkey_hex);
        let _ = client_clone.subscribe(filter.clone(), None).await;
        
        // Give subscription a moment to be active
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Spawn task to handle incoming events
        let party_idx_debug = party_index;
        tokio::spawn(async move {
            let mut notifications = client_clone.notifications();
            eprintln!("   🎧 Party {} notification handler started", party_idx_debug);
            
            while let Ok(notification) = notifications.recv().await {
                // Handle different notification types
                match notification {
                    nostr_sdk::RelayPoolNotification::Event { event, .. } => {
                        // Process the event
                        process_event(
                            &event,
                            &our_pubkey_hex,
                            &session_id,
                            phase,
                            receiver,
                            &keys,
                            &received,
                            party_index,
                        ).await;
                    }
                    nostr_sdk::RelayPoolNotification::Message { message, .. } => {
                        // Check if it's an EVENT message
                        if let nostr_sdk::RelayMessage::Event { event, .. } = message {
                            process_event(
                                &event,
                                &our_pubkey_hex,
                                &session_id,
                                phase,
                                receiver,
                                &keys,
                                &received,
                                party_index,
                            ).await;
                        }
                    }
                    _ => {
                        // Other notification types - ignore
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Get messages for a receiver in a specific phase
    async fn get_messages(
        &self,
        phase: u8,
        receiver: u8,
    ) -> Result<Vec<Value>, String> {
        let key = format!("{}_{}", phase, receiver);
        let msgs = self.received_messages.read().await;
        Ok(msgs.get(&key).cloned().unwrap_or_default())
    }

    /// Query for events that might have been published before we subscribed
    /// Note: This is a placeholder - we'll rely on the subscription stream
    async fn query_past_events(&self, _phase: u8) -> Result<(), String> {
        // Give a moment for events to propagate through the relay
        tokio::time::sleep(Duration::from_millis(2000)).await;
        eprintln!("   ⏳ Party {} waiting for events to propagate...", self.party_index);
        Ok(())
    }

    /// Wait for messages to arrive (polling)
    /// Note: start_listening should be called before this
    async fn wait_for_messages(
        &self,
        phase: u8,
        receiver: u8,
        expected_count: usize,
        max_wait_secs: u64,
    ) -> Result<Vec<Value>, String> {
        // Query for past events first
        self.query_past_events(phase).await?;
        
        let start = std::time::Instant::now();
        
        loop {
            let messages = self.get_messages(phase, receiver).await?;
            eprintln!("   📊 Party {} has {}/{} messages for phase {} receiver {}", 
                self.party_index, messages.len(), expected_count, phase, receiver);
            
            if messages.len() >= expected_count {
                return Ok(messages);
            }
            
            if start.elapsed().as_secs() > max_wait_secs {
                return Err(format!(
                    "Timeout waiting for messages. Got {}/{}",
                    messages.len(),
                    expected_count
                ));
            }
            
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }
}

/// Process a received event
async fn process_event(
    event: &nostr_sdk::Event,
    our_pubkey_hex: &str,
    session_id: &str,
    phase: u8,
    receiver: u8,
    keys: &nostr_sdk::Keys,
    received: &Arc<RwLock<HashMap<String, Vec<Value>>>>,
    party_index: u8,
) {
    // Check if this event is for us by checking the 'p' tag
    let mut is_for_us = false;
    for tag in event.tags.iter() {
        let tag_vec = tag.clone().to_vec();
        if tag_vec.len() > 0 && tag_vec[0] == "p" {
            if tag_vec.len() > 1 && tag_vec[1] == our_pubkey_hex {
                is_for_us = true;
                break;
            }
        }
    }
    
    if !is_for_us {
        // This event is not for us, skip it
        return;
    }
    
    eprintln!("   📬 Party {} received event (kind: {:?}, pubkey: {})", 
        party_index, event.kind, event.pubkey);
    
    // Decrypt the content using NIP-04
    let sender_pubkey = event.pubkey;
    let encrypted_content = &event.content;
    
    use nostr::nips::nip04;
    match nip04::decrypt(
        &keys.secret_key(),
        &sender_pubkey,
        encrypted_content,
    ) {
        Ok(plaintext) => {
            eprintln!("   🔓 Party {} successfully decrypted message", party_index);
            // Parse the decrypted JSON
            if let Ok(message_payload) = serde_json::from_str::<Value>(&plaintext) {
                eprintln!("   📋 Party {} parsed payload: session_id={}, phase={}, receiver={}", 
                    party_index,
                    message_payload["session_id"].as_str().unwrap_or("?"),
                    message_payload["phase"].as_u64().unwrap_or(0),
                    message_payload["receiver"].as_u64().unwrap_or(0));
                
                // Check if this message is for us
                if let (Some(msg_session_id), Some(msg_phase), Some(msg_receiver)) = (
                    message_payload["session_id"].as_str(),
                    message_payload["phase"].as_u64(),
                    message_payload["receiver"].as_u64(),
                ) {
                    if msg_session_id == session_id 
                        && msg_phase == phase as u64 
                        && msg_receiver == receiver as u64 
                    {
                        // Extract the actual data
                        if let Some(data) = message_payload.get("data") {
                            let mut msgs = received.write().await;
                            let key = format!("{}_{}", phase, receiver);
                            msgs.entry(key).or_insert_with(Vec::new).push(data.clone());
                            eprintln!("   ✅ Party {} stored message for phase {} from sender {}", 
                                party_index, phase, message_payload["sender"].as_u64().unwrap_or(0));
                        }
                    } else {
                        eprintln!("   ⚠️  Party {} message doesn't match: session_id={} (expected {}), phase={} (expected {}), receiver={} (expected {})", 
                            party_index,
                            msg_session_id, session_id,
                            msg_phase, phase,
                            msg_receiver, receiver);
                    }
                }
            } else {
                eprintln!("   ⚠️  Party {} failed to parse decrypted JSON", party_index);
            }
        }
        Err(e) => {
            // Decryption failed - might be from a different sender or corrupted
            eprintln!("   ⚠️  Party {} decryption failed: {} (sender: {})", 
                party_index, e, sender_pubkey);
        }
    }
}

/// Run a single party through the DKG protocol using Nostr
async fn run_party_nostr(
    parameters: &Parameters,
    party_index: u8,
    session_id: &[u8],
) -> Result<dkls23::protocols::Party, String> {
    let client = NostrRelayClient::new(party_index, session_id).await?;
    let mut party = DkgPartyState::new(parameters, party_index, session_id);

    println!("   🎯 Party {} starting DKG via Nostr (NIP-04)...", party_index);
    println!("   🔑 Party {} pubkey: {}", party_index, client.keys.public_key());

    // Exchange public keys with other parties first
    println!("   📡 Party {} exchanging public keys...", party_index);
    client.exchange_pubkeys(parameters.share_count).await?;
    println!("   ✅ Party {} received all public keys", party_index);

    // ===== PHASE 1 =====
    // Start listening BEFORE sending messages
    println!("   👂 Party {} starting to listen for Phase 1 messages...", party_index);
    client.start_listening(1, party_index).await?;
    tokio::time::sleep(Duration::from_millis(1000)).await; // Give subscription time to be active
    
    let phase1_msgs = party.start_phase1()
        .map_err(|e| format!("Party {} Phase 1 failed: {}", e.index, e.description))?;
    
    // Send messages to relay
    println!("   📤 Party {} sending {} Phase 1 messages...", party_index, phase1_msgs.len());
    for msg in &phase1_msgs {
        let data = serde_json::to_value(msg)
            .map_err(|e| format!("Serialization error: {}", e))?;
        client.post_message(1, msg.sender, msg.receiver, &data).await?;
    }
    
    // Give messages time to propagate through the relay
    println!("   ⏳ Party {} waiting for messages to propagate...", party_index);
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Wait for and receive messages from other parties
    let received_json = client
        .wait_for_messages(1, party_index, parameters.share_count as usize, 60)
        .await?;
    
    let mut received: Vec<Phase1Message> = Vec::new();
    for json in received_json {
        let msg: Phase1Message = serde_json::from_value(json)
            .map_err(|e| format!("Deserialization error: {}", e))?;
        received.push(msg);
    }
    
    party.process_phase1_messages(&received)
        .map_err(|e| format!("Party {} process Phase 1 failed: {}", e.index, e.description))?;
    println!("   ✅ Party {} completed Phase 1", party_index);

    // ===== PHASE 2 =====
    // Start listening for Phase 2 messages
    client.start_listening(2, party_index).await?;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    let phase2_msgs = party.start_phase2()
        .map_err(|e| format!("Party {} Phase 2 failed: {}", e.index, e.description))?;
    
    // Send proof commitment to all parties (broadcast)
    let proof_data = serde_json::to_value(&phase2_msgs.proof_commitment)
        .map_err(|e| format!("Serialization error: {}", e))?;
    for receiver in 1..=parameters.share_count {
        client.post_message(2, party_index, receiver, &proof_data).await?;
    }
    
    // Send BIP commitment to all parties (broadcast)
    let bip_data = serde_json::to_value(&phase2_msgs.bip_commitment)
        .map_err(|e| format!("Serialization error: {}", e))?;
    for receiver in 1..=parameters.share_count {
        client.post_message(2, party_index, receiver, &bip_data).await?;
    }
    
    // Send zero commitments to specific receivers
    for zero_commit in &phase2_msgs.zero_commitments {
        let data = serde_json::to_value(zero_commit)
            .map_err(|e| format!("Serialization error: {}", e))?;
        client.post_message(2, party_index, zero_commit.parties.receiver, &data).await?;
    }
    
    // Wait for Phase 2 messages from all parties
    let expected_count = 3 * parameters.share_count as usize - 1;
    let phase2_received_json = client
        .wait_for_messages(2, party_index, expected_count, 60)
        .await?;
    
    // Reconstruct Phase2Messages for complete_phase4
    let phase2_received = reconstruct_phase2_messages(&phase2_received_json, parameters)?;
    party.process_phase2_messages(&phase2_received)
        .map_err(|e| format!("Party {} process Phase 2 failed: {}", e.index, e.description))?;
    println!("   ✅ Party {} completed Phase 2", party_index);

    // ===== PHASE 3 =====
    // Start listening for Phase 3 messages
    client.start_listening(3, party_index).await?;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    let phase3_msgs = party.start_phase3()
        .map_err(|e| format!("Party {} Phase 3 failed: {}", e.index, e.description))?;
    
    // Send BIP reveal to all parties (broadcast)
    let bip_data = serde_json::to_value(&phase3_msgs.bip_reveal)
        .map_err(|e| format!("Serialization error: {}", e))?;
    for receiver in 1..=parameters.share_count {
        client.post_message(3, party_index, receiver, &bip_data).await?;
    }
    
    // Send zero seeds to specific receivers
    for zero_seed in &phase3_msgs.zero_seeds {
        let data = serde_json::to_value(zero_seed)
            .map_err(|e| format!("Serialization error: {}", e))?;
        client.post_message(3, party_index, zero_seed.parties.receiver, &data).await?;
    }
    
    // Send mul init to specific receivers
    for mul_init in &phase3_msgs.mul_init {
        let data = serde_json::to_value(mul_init)
            .map_err(|e| format!("Serialization error: {}", e))?;
        client.post_message(3, party_index, mul_init.parties.receiver, &data).await?;
    }
    
    // Wait for Phase 3 messages
    let expected_count = 3 * parameters.share_count as usize - 2;
    let phase3_received_json = client
        .wait_for_messages(3, party_index, expected_count, 60)
        .await?;
    
    // Reconstruct Phase3Messages for complete_phase4
    let phase3_received = reconstruct_phase3_messages(&phase3_received_json, parameters)?;
    party.process_phase3_messages(&phase3_received)
        .map_err(|e| format!("Party {} process Phase 3 failed: {}", e.index, e.description))?;
    println!("   ✅ Party {} completed Phase 3", party_index);

    // ===== PHASE 4 =====
    let completed = party.complete_phase4(&phase2_received, &phase3_received)
        .map_err(|e| format!("Party {} Phase 4 failed: {}", e.index, e.description))?;
    
    // Save keyshare to local file
    let keyshare_filename = format!("party_{}_keyshare_nostr.json", party_index);
    let keyshare_json = serde_json::to_string_pretty(&completed)
        .map_err(|e| format!("Failed to serialize keyshare: {}", e))?;
    
    fs::write(&keyshare_filename, keyshare_json)
        .map_err(|e| format!("Failed to write keyshare to {}: {}", keyshare_filename, e))?;
    
    println!("   ✅ Party {} completed DKG!", party_index);
    println!("   💾 Party {} keyshare saved to {}", party_index, keyshare_filename);
    Ok(completed)
}

/// Reconstruct Phase2Messages from JSON messages
fn reconstruct_phase2_messages(
    json_messages: &[Value],
    parameters: &Parameters,
) -> Result<Vec<Phase2Messages>, String> {
    use dkls23::protocols::dkg;
    use std::collections::HashMap;
    
    let mut phase2_by_party: HashMap<u8, (Option<dkg::ProofCommitment>, Option<dkg::BroadcastDerivationPhase2to4>, Vec<dkg::TransmitInitZeroSharePhase2to4>)> = HashMap::new();
    
    for json in json_messages {
        // Try to deserialize as ProofCommitment
        if let Ok(proof) = serde_json::from_value::<dkg::ProofCommitment>(json.clone()) {
            let entry = phase2_by_party.entry(proof.index).or_insert_with(|| (None, None, Vec::new()));
            entry.0 = Some(proof);
            continue;
        }
        
        // Try to deserialize as BroadcastDerivationPhase2to4
        if let Ok(bip) = serde_json::from_value::<dkg::BroadcastDerivationPhase2to4>(json.clone()) {
            let entry = phase2_by_party.entry(bip.sender_index).or_insert_with(|| (None, None, Vec::new()));
            entry.1 = Some(bip);
            continue;
        }
        
        // Try to deserialize as TransmitInitZeroSharePhase2to4
        if let Ok(zero) = serde_json::from_value::<dkg::TransmitInitZeroSharePhase2to4>(json.clone()) {
            let sender = zero.parties.sender;
            let entry = phase2_by_party.entry(sender).or_insert_with(|| (None, None, Vec::new()));
            entry.2.push(zero);
        }
    }
    
    // Convert to Phase2Messages
    let mut phase2_by_party_final: HashMap<u8, Phase2Messages> = HashMap::new();
    for (party_idx, (proof_opt, bip_opt, zero_commits)) in phase2_by_party {
        let proof = proof_opt.ok_or_else(|| format!("Missing proof commitment from party {}", party_idx))?;
        let bip = bip_opt.ok_or_else(|| format!("Missing BIP commitment from party {}", party_idx))?;
        phase2_by_party_final.insert(party_idx, Phase2Messages {
            proof_commitment: proof,
            zero_commitments: zero_commits,
            bip_commitment: bip,
        });
    }
    
    // Convert to vector ordered by party index
    let mut result = Vec::new();
    for i in 1..=parameters.share_count {
        if let Some(messages) = phase2_by_party_final.remove(&i) {
            result.push(messages);
        } else {
            return Err(format!("Missing Phase 2 messages from party {}", i));
        }
    }
    
    Ok(result)
}

/// Reconstruct Phase3Messages from JSON messages
fn reconstruct_phase3_messages(
    json_messages: &[Value],
    parameters: &Parameters,
) -> Result<Vec<Phase3Messages>, String> {
    use dkls23::protocols::dkg;
    use std::collections::HashMap;
    
    let mut phase3_by_party: HashMap<u8, Phase3Messages> = HashMap::new();
    
    for json in json_messages {
        // Try to deserialize as BroadcastDerivationPhase3to4
        if let Ok(bip) = serde_json::from_value::<dkg::BroadcastDerivationPhase3to4>(json.clone()) {
            let entry = phase3_by_party.entry(bip.sender_index).or_insert_with(|| Phase3Messages {
                zero_seeds: Vec::new(),
                mul_init: Vec::new(),
                bip_reveal: bip.clone(),
            });
            entry.bip_reveal = bip;
            continue;
        }
        
        // Try to deserialize as TransmitInitZeroSharePhase3to4
        if let Ok(zero) = serde_json::from_value::<dkg::TransmitInitZeroSharePhase3to4>(json.clone()) {
            let sender = zero.parties.sender;
            let entry = phase3_by_party.entry(sender).or_insert_with(|| Phase3Messages {
                zero_seeds: Vec::new(),
                mul_init: Vec::new(),
                bip_reveal: dkg::BroadcastDerivationPhase3to4 {
                    sender_index: sender,
                    aux_chain_code: [0; 32],
                    cc_salt: Vec::new(),
                },
            });
            entry.zero_seeds.push(zero);
            continue;
        }
        
        // Try to deserialize as TransmitInitMulPhase3to4
        if let Ok(mul) = serde_json::from_value::<dkg::TransmitInitMulPhase3to4>(json.clone()) {
            let sender = mul.parties.sender;
            let entry = phase3_by_party.entry(sender).or_insert_with(|| Phase3Messages {
                zero_seeds: Vec::new(),
                mul_init: Vec::new(),
                bip_reveal: dkg::BroadcastDerivationPhase3to4 {
                    sender_index: sender,
                    aux_chain_code: [0; 32],
                    cc_salt: Vec::new(),
                },
            });
            entry.mul_init.push(mul);
        }
    }
    
    // Convert to vector ordered by party index
    let mut result = Vec::new();
    for i in 1..=parameters.share_count {
        if let Some(messages) = phase3_by_party.remove(&i) {
            result.push(messages);
        } else {
            return Err(format!("Missing Phase 3 messages from party {}", i));
        }
    }
    
    Ok(result)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("🔐 Nostr-based Distributed DKG Test");
    println!("====================================\n");
    println!("🌐 Using Nostr relay: {}\n", NOSTR_RELAY);

    let parameters = Parameters {
        threshold: 2,
        share_count: 3,
    };
    let session_id = b"nostr_test_session";

    println!("📋 Parameters: {}-of-{} threshold scheme", parameters.threshold, parameters.share_count);
    println!("🆔 Session ID: {}\n", hex::encode(session_id));

    // Run all parties concurrently
    println!("🚀 Starting all parties...\n");
    
    let mut handles = Vec::new();
    for i in 1..=parameters.share_count {
        let params = parameters.clone();
        let sid = session_id.to_vec();
        let handle = tokio::spawn(async move {
            run_party_nostr(&params, i, &sid).await
        });
        handles.push(handle);
    }

    // Wait for all parties to complete
    let mut parties = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(Ok(party)) => {
                parties.push(party);
                println!("✅ Party {} finished successfully", i + 1);
            }
            Ok(Err(e)) => {
                eprintln!("❌ Party {} failed: {}", i + 1, e);
                return Err(e);
            }
            Err(e) => {
                eprintln!("❌ Party {} task failed: {}", i + 1, e);
                return Err(format!("Task failed: {}", e));
            }
        }
    }

    // List all saved keyshare files
    println!("\n💾 Saved keyshare files:");
    for i in 1..=parameters.share_count {
        let filename = format!("party_{}_keyshare_nostr.json", i);
        if Path::new(&filename).exists() {
            println!("   ✅ {}", filename);
        } else {
            println!("   ⚠️  {} (not found)", filename);
        }
    }

    // Verify all parties have the same public key
    println!("\n🔍 Verification:");
    let first_pk = &parties[0].pk;
    let first_address = &parties[0].btc_address;
    
    for (i, party) in parties.iter().enumerate() {
        assert_eq!(&party.pk, first_pk, "Party {} has different public key!", i + 1);
        assert_eq!(&party.btc_address, first_address, "Party {} has different address!", i + 1);
        println!("   ✅ Party {}: Public key and address match", i + 1);
    }

    println!("\n✅ Nostr-based Distributed DKG completed successfully!");
    println!("🌐 Network: {:?}", parties[0].network);
    println!("₿ Bitcoin address: {}", first_address);
    println!("\n💡 All parties successfully generated keyshares via Nostr communication!");

    // ===== THRESHOLD SIGNING TEST =====
    println!("\n✍️  Testing Threshold Signing");
    println!("==============================\n");

    // Create a test message to sign
    let message = b"Hello from Nostr-based Distributed DKG!";
    let message_hash = hash(message, &[]);
    let sign_id = b"nostr_test_sign_session";
    
    println!("📝 Message to sign: {}", String::from_utf8_lossy(message));
    println!("📋 Hash: {}\n", hex::encode(message_hash));

    // Select parties to participate in signing (need threshold number of parties)
    let executing_parties: Vec<u8> = (1..=parameters.threshold).collect();
    println!("👥 Participating parties for signing: {:?}", executing_parties);
    println!("   (Need {} parties for {}-of-{} threshold)\n", 
             parameters.threshold, parameters.threshold, parameters.share_count);

    // Perform threshold signing
    println!("🔐 Starting threshold signature generation...");
    let (r, s, rec_id) = threshold_sign(
        &parties,
        &executing_parties,
        sign_id,
        message_hash,
        true, // normalize_low_s
    ).map_err(|e| format!("Threshold signing failed: Party {} - {}", e.index, e.description))?;

    println!("\n✅ Threshold signature generated successfully!");
    println!("📝 Signature r: {}", r);
    println!("📝 Signature s: {}", s);
    println!("🆔 Recovery ID: {}", rec_id);

    // Verify the signature using the public key
    println!("\n🔍 Verifying signature...");
    let public_key = &parties[0].pk;
    
    // Use the library's verification function
    use dkls23::protocols::signing::verify_ecdsa_signature;
    let is_valid = verify_ecdsa_signature(&message_hash, public_key, &r, &s);
    
    if is_valid {
        println!("   ✅ Signature is valid!");
        println!("   ✅ Public key matches the signature!");
    } else {
        return Err("Signature verification failed!".to_string());
    }

    println!("\n🎉 All tests passed!");
    println!("   ✅ DKG completed via Nostr");
    println!("   ✅ All parties have matching public keys");
    println!("   ✅ Threshold signature generated successfully");
    println!("   ✅ Signature verified against public key");

    Ok(())
}
