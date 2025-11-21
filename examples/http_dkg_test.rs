//! HTTP-based Distributed DKG Test
//!
//! This example demonstrates distributed DKG where parties communicate via HTTP.
//! It starts an HTTP relay server and then runs multiple parties that send/receive
//! messages through the relay.

use dkls23::protocols::dkg_distributed::*;
use dkls23::protocols::Parameters;
use dkls23::facade::threshold_sign;
use dkls23::utilities::hashes::hash;
use serde_json::Value;
use std::time::Duration;
use std::fs;
use std::path::Path;

const RELAY_URL: &str = "http://127.0.0.1:8080";

/// HTTP client for communicating with the relay
struct RelayClient {
    client: reqwest::Client,
}

impl RelayClient {
    fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Post a message to the relay
    async fn post_message(
        &self,
        phase: u8,
        sender: u8,
        receiver: u8,
        data: &Value,
    ) -> Result<(), String> {
        let url = format!("{}/message/{}/{}/{}", RELAY_URL, phase, sender, receiver);
        let payload = serde_json::json!({ "data": data });
        
        self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;
        
        Ok(())
    }

    /// Get messages for a receiver in a specific phase
    async fn get_messages(
        &self,
        phase: u8,
        receiver: u8,
    ) -> Result<Vec<Value>, String> {
        let url = format!("{}/messages/{}/{}", RELAY_URL, phase, receiver);
        
        let response = self.client.get(&url).send().await
            .map_err(|e| format!("HTTP error: {}", e))?;
        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("JSON parse error: {}", e))?;
        
        let messages = json["messages"]
            .as_array()
            .ok_or_else(|| "Invalid response format".to_string())?
            .iter()
            .map(|msg| msg["data"].clone())
            .collect();
        
        Ok(messages)
    }

    /// Wait for messages to arrive (polling)
    async fn wait_for_messages(
        &self,
        phase: u8,
        receiver: u8,
        expected_count: usize,
        max_wait_secs: u64,
    ) -> Result<Vec<Value>, String> {
        let start = std::time::Instant::now();
        
        loop {
            let messages = self.get_messages(phase, receiver).await?;
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
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Run a single party through the DKG protocol
async fn run_party(
    parameters: &Parameters,
    party_index: u8,
    session_id: &[u8],
) -> Result<dkls23::protocols::Party, String> {
    let client = RelayClient::new();
    let mut party = DkgPartyState::new(parameters, party_index, session_id);

    println!("   🎯 Party {} starting DKG...", party_index);

    // ===== PHASE 1 =====
    let phase1_msgs = party.start_phase1()
        .map_err(|e| format!("Party {} Phase 1 failed: {}", e.index, e.description))?;
    
    // Send messages to relay
    for msg in &phase1_msgs {
        let data = serde_json::to_value(msg)
            .map_err(|e| format!("Serialization error: {}", e))?;
        client.post_message(1, msg.sender, msg.receiver, &data).await?;
    }
    
    // Wait for and receive messages from other parties
    let received_json = client
        .wait_for_messages(1, party_index, parameters.share_count as usize, 30)
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
    // Each party receives:
    //   - n proof commitments (one from each party, including self)
    //   - n BIP commitments (one from each party, including self)
    //   - (n-1) zero commitments (one from each other party)
    // Total: 2n + (n-1) = 3n - 1
    let expected_count = 3 * parameters.share_count as usize - 1;
    let phase2_received_json = client
        .wait_for_messages(2, party_index, expected_count, 30)
        .await?;
    
    // Reconstruct Phase2Messages for complete_phase4
    let phase2_received = reconstruct_phase2_messages(&phase2_received_json, parameters)?;
    party.process_phase2_messages(&phase2_received)
        .map_err(|e| format!("Party {} process Phase 2 failed: {}", e.index, e.description))?;
    println!("   ✅ Party {} completed Phase 2", party_index);

    // ===== PHASE 3 =====
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
    // Each party receives:
    //   - n BIP reveals (one from each party, including self)
    //   - (n-1) zero seeds (one from each other party)
    //   - (n-1) mul init (one from each other party)
    // Total: n + 2*(n-1) = 3n - 2
    let expected_count = 3 * parameters.share_count as usize - 2;
    let phase3_received_json = client
        .wait_for_messages(3, party_index, expected_count, 30)
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
    let keyshare_filename = format!("party_{}_keyshare.json", party_index);
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
    println!("🔐 HTTP-based Distributed DKG Test");
    println!("====================================\n");

    // Check if relay is running
    let client = reqwest::Client::new();
    match client.get(&format!("{}/health", RELAY_URL)).send().await {
        Ok(_) => println!("✅ Relay server is running\n"),
        Err(_) => {
            println!("❌ Relay server is not running!");
            println!("   Please start it with: cargo run --bin http_relay");
            return Err("Relay server not available".into());
        }
    }

    let parameters = Parameters {
        threshold: 2,
        share_count: 3,
    };
    let session_id = b"http_test_session";

    println!("📋 Parameters: {}-of-{} threshold scheme", parameters.threshold, parameters.share_count);
    println!("🆔 Session ID: {}\n", hex::encode(session_id));

    // Clear any existing messages
    client.post(&format!("{}/clear", RELAY_URL)).send().await
        .map_err(|e| format!("Failed to clear messages: {}", e))?;

    // Run all parties concurrently
    println!("🚀 Starting all parties...\n");
    
    let mut handles = Vec::new();
    for i in 1..=parameters.share_count {
        let params = parameters.clone();
        let sid = session_id.to_vec();
        let handle = tokio::spawn(async move {
            run_party(&params, i, &sid).await
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
        let filename = format!("party_{}_keyshare.json", i);
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

    println!("\n✅ HTTP-based Distributed DKG completed successfully!");
    println!("🌐 Network: {:?}", parties[0].network);
    println!("₿ Bitcoin address: {}", first_address);
    println!("\n💡 All parties successfully generated keyshares via HTTP communication!");

    // ===== THRESHOLD SIGNING TEST =====
    println!("\n✍️  Testing Threshold Signing");
    println!("==============================\n");

    // Create a test message to sign
    let message = b"Hello from HTTP-based Distributed DKG!";
    let message_hash = hash(message, &[]);
    let sign_id = b"http_test_sign_session";
    
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
    println!("   ✅ DKG completed via HTTP");
    println!("   ✅ All parties have matching public keys");
    println!("   ✅ Threshold signature generated successfully");
    println!("   ✅ Signature verified against public key");

    Ok(())
}
