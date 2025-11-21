//! Example demonstrating distributed DKG where each party runs as a separate process.
//!
//! This example shows how to use the distributed DKG API to allow each party
//! to generate its own keyshare locally, rather than all parties running on
//! the same machine.
//!
//! In a real deployment, each party would:
//! 1. Run as a separate process/machine
//! 2. Communicate via network (HTTP, gRPC, etc.)
//! 3. Serialize messages using serde (JSON, bincode, etc.)
//!
//! This example simulates multiple parties by running them sequentially,
//! but in practice they would run concurrently on different machines.

use dkls23::protocols::dkg_distributed::*;
use dkls23::protocols::Parameters;

fn main() -> Result<(), String> {
    println!("🔐 Distributed DKG Example");
    println!("==========================\n");

    // Parameters for a 2-of-3 threshold scheme
    let parameters = Parameters {
        threshold: 2,
        share_count: 3,
    };
    let session_id = b"distributed_session_example";

    println!("📋 Parameters: {}-of-{} threshold scheme", parameters.threshold, parameters.share_count);
    println!("🆔 Session ID: {}\n", hex::encode(session_id));

    // Initialize all parties
    let mut parties: Vec<DkgPartyState> = (1..=parameters.share_count)
        .map(|i| DkgPartyState::new(&parameters, i, session_id))
        .collect();

    println!("✅ Initialized {} parties\n", parties.len());

    // ===== PHASE 1 =====
    println!("📊 Phase 1: Generating polynomial fragments...");
    let mut phase1_messages: Vec<Vec<Phase1Message>> = Vec::new();
    
    for party in &mut parties {
        let msgs = party.start_phase1().map_err(|e| format!("Party {} Phase 1 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} generated {} fragments", party.party_index(), msgs.len());
        phase1_messages.push(msgs);
    }

    // Simulate network communication: each party receives fragments from all parties
    println!("\n📡 Communication round 1: Exchanging polynomial fragments...");
    for (i, party) in parties.iter_mut().enumerate() {
        // Collect all messages intended for this party
        let mut received: Vec<Phase1Message> = Vec::new();
        for msgs in &phase1_messages {
            for msg in msgs {
                if msg.receiver == party.party_index() {
                    received.push(msg.clone());
                }
            }
        }
        party.process_phase1_messages(&received).map_err(|e| format!("Party {} process Phase 1 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} received {} fragments", i + 1, received.len());
    }

    // ===== PHASE 2 =====
    println!("\n📊 Phase 2: Generating proofs and commitments...");
    let mut phase2_messages: Vec<Phase2Messages> = Vec::new();
    
    for party in &mut parties {
        let msgs = party.start_phase2().map_err(|e| format!("Party {} Phase 2 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} generated proofs and commitments", party.party_index());
        phase2_messages.push(msgs);
    }

    // Simulate network communication: each party receives Phase 2 messages
    println!("\n📡 Communication round 2: Exchanging proofs and commitments...");
    for party in &mut parties {
        party.process_phase2_messages(&phase2_messages).map_err(|e| format!("Party {} process Phase 2 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} received Phase 2 messages", party.party_index());
    }

    // ===== PHASE 3 =====
    println!("\n📊 Phase 3: Continuing initialization...");
    let mut phase3_messages: Vec<Phase3Messages> = Vec::new();
    
    for party in &mut parties {
        let msgs = party.start_phase3().map_err(|e| format!("Party {} Phase 3 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} generated Phase 3 messages", party.party_index());
        phase3_messages.push(msgs);
    }

    // Simulate network communication: each party receives Phase 3 messages
    println!("\n📡 Communication round 3: Exchanging final initialization data...");
    for party in &mut parties {
        party.process_phase3_messages(&phase3_messages).map_err(|e| format!("Party {} process Phase 3 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} received Phase 3 messages", party.party_index());
    }

    // ===== PHASE 4 =====
    println!("\n📊 Phase 4: Completing DKG...");
    let mut completed_parties: Vec<dkls23::protocols::Party> = Vec::new();
    
    for party in &mut parties {
        let completed = party.complete_phase4(&phase2_messages, &phase3_messages).map_err(|e| format!("Party {} Phase 4 failed: {}", e.index, e.description))?;
        println!("   ✅ Party {} completed DKG", party.party_index());
        completed_parties.push(completed);
    }

    // Verify all parties have the same public key
    println!("\n🔍 Verification:");
    let first_pk = &completed_parties[0].pk;
    let first_address = &completed_parties[0].btc_address;
    
    for (i, party) in completed_parties.iter().enumerate() {
        assert_eq!(&party.pk, first_pk, "Party {} has different public key!", i + 1);
        assert_eq!(&party.btc_address, first_address, "Party {} has different address!", i + 1);
        println!("   ✅ Party {}: Public key matches", i + 1);
    }

    println!("\n✅ Distributed DKG completed successfully!");
    println!("🌐 Network: {:?}", completed_parties[0].network);
    println!("₿ Bitcoin address: {}", first_address);
    println!("\n💡 Each party now has its own keyshare and can participate in threshold signing!");

    Ok(())
}

