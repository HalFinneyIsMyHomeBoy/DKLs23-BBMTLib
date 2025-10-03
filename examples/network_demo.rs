//! Network demonstration example for DKLs23 Threshold ECDSA library.
//! 
//! This example demonstrates how to generate Bitcoin addresses for both
//! mainnet and testnet3 networks using the DKLs23 library.

use dkls23::protocols::dkg::*;
use dkls23::protocols::signing::*;
use dkls23::protocols::*;
use dkls23::utilities::hashes::hash;
use k256::elliptic_curve::group::GroupEncoding;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DKLs23 Network Demo - Bitcoin Mainnet vs Testnet3");
    println!("====================================================");

    // Step 1: Set up parameters for a 2-of-3 threshold scheme
    let parameters = Parameters {
        threshold: 2,      // Need 2 parties to sign
        share_count: 3,    // Total of 3 parties
    };
    
    println!("📋 Parameters: {}-of-{} threshold scheme", parameters.threshold, parameters.share_count);

    // Step 2: Generate a session ID for this key generation
    let session_id = b"network_demo_session_2024".to_vec();
    println!("🆔 Session ID: {}", hex::encode(&session_id));

    // Step 3: Demonstrate both networks
    println!("\n🌐 Network Comparison:");
    println!("   Mainnet:   Addresses start with '1' (production Bitcoin)");
    println!("   Testnet3:  Addresses start with 'm' or 'n' (testing Bitcoin)");

    // Step 4: Simulate Distributed Key Generation for Mainnet
    println!("\n🔑 Generating Mainnet Keys...");
    let mainnet_parties = generate_parties_for_network(&parameters, &session_id, Network::Mainnet)?;
    
    println!("✅ Mainnet DKG completed!");
    for (i, party) in mainnet_parties.iter().enumerate() {
        println!("   🔑 Party {} public key: {}", i + 1, hex::encode(party.pk.to_bytes().as_slice()));
        println!("   ₿  Party {} Bitcoin address: {}", i + 1, party.btc_address);
    }

    // Step 5: Simulate Distributed Key Generation for Testnet3
    println!("\n🔑 Generating Testnet3 Keys...");
    let testnet_parties = generate_parties_for_network(&parameters, &session_id, Network::Testnet3)?;
    
    println!("✅ Testnet3 DKG completed!");
    for (i, party) in testnet_parties.iter().enumerate() {
        println!("   🔑 Party {} public key: {}", i + 1, hex::encode(party.pk.to_bytes().as_slice()));
        println!("   🧪 Party {} Testnet3 address: {}", i + 1, party.btc_address);
    }

    // Step 6: Demonstrate signing with both networks
    println!("\n✍️  Signing with Both Networks:");
    
    let message = "Hello, Multi-Network Threshold ECDSA!";
    let message_hash = hash(message.as_bytes(), &[]);
    println!("📝 Message: {}", message);
    println!("🔢 Message hash: {}", hex::encode(message_hash));

    // Sign with mainnet
    println!("\n🔐 Mainnet Signature:");
    let mainnet_signature = sign_with_network(&mainnet_parties, &message_hash, "mainnet_signing")?;
    println!("   📝 Signature r: {}", mainnet_signature.0);
    println!("   📝 Signature s: {}", mainnet_signature.1);
    println!("   🆔 Recovery ID: {}", mainnet_signature.2);

    // Sign with testnet3
    println!("\n🧪 Testnet3 Signature:");
    let testnet_signature = sign_with_network(&testnet_parties, &message_hash, "testnet_signing")?;
    println!("   📝 Signature r: {}", testnet_signature.0);
    println!("   📝 Signature s: {}", testnet_signature.1);
    println!("   🆔 Recovery ID: {}", testnet_signature.2);

    // Step 7: Show network differences
    println!("\n📊 Network Comparison Summary:");
    println!("   🌐 Mainnet addresses:");
    for (i, party) in mainnet_parties.iter().enumerate() {
        println!("      Party {}: {}", i + 1, party.btc_address);
    }
    
    println!("   🧪 Testnet3 addresses:");
    for (i, party) in testnet_parties.iter().enumerate() {
        println!("      Party {}: {}", i + 1, party.btc_address);
    }

    println!("\n💡 Key Differences:");
    println!("   • Mainnet addresses start with '1' (production use)");
    println!("   • Testnet3 addresses start with 'm' or 'n' (testing use)");
    println!("   • Same cryptographic security for both networks");
    println!("   • Same threshold signing capabilities");
    println!("   • Testnet3 is free to use for development and testing");

    println!("\n🎉 Network demo completed successfully!");
    println!("   Both mainnet and testnet3 addresses are ready for use!");

    Ok(())
}

/// Generates parties for a specific network
fn generate_parties_for_network(
    parameters: &Parameters,
    session_id: &[u8],
    network: Network,
) -> Result<Vec<Party>, Box<dyn std::error::Error>> {
    // Create session data for each party
    let mut all_data: Vec<SessionData> = Vec::new();
    for i in 0..parameters.share_count {
        all_data.push(SessionData {
            parameters: parameters.clone(),
            party_index: i + 1,
            session_id: session_id.to_vec(),
        });
    }

    // Phase 1: Each party generates polynomial fragments
    let mut dkg_1: Vec<Vec<k256::Scalar>> = Vec::new();
    for i in 0..parameters.share_count {
        let out1 = phase1(&all_data[i as usize]);
        dkg_1.push(out1);
    }

    // Communication round 1: Exchange polynomial fragments
    let mut poly_fragments = vec![Vec::<k256::Scalar>::new(); parameters.share_count as usize];
    for row_i in dkg_1 {
        for j in 0..parameters.share_count {
            poly_fragments[j as usize].push(row_i[j as usize]);
        }
    }

    // Phase 2: Generate proofs and commitments
    let mut poly_points: Vec<k256::Scalar> = Vec::new();
    let mut proofs_commitments: Vec<ProofCommitment> = Vec::new();
    let mut zero_kept_2to3: Vec<BTreeMap<u8, KeepInitZeroSharePhase2to3>> = Vec::new();
    let mut zero_transmit_2to4: Vec<Vec<TransmitInitZeroSharePhase2to4>> = Vec::new();
    let mut bip_kept_2to3: Vec<UniqueKeepDerivationPhase2to3> = Vec::new();
    let mut bip_broadcast_2to4: BTreeMap<u8, BroadcastDerivationPhase2to4> = BTreeMap::new();
    
    for i in 0..parameters.share_count {
        let (out1, out2, out3, out4, out5, out6) = phase2(&all_data[i as usize], &poly_fragments[i as usize]);
        poly_points.push(out1);
        proofs_commitments.push(out2);
        zero_kept_2to3.push(out3);
        zero_transmit_2to4.push(out4);
        bip_kept_2to3.push(out5);
        bip_broadcast_2to4.insert(i + 1, out6);
    }

    // Communication round 2: Exchange zero share commitments
    let mut zero_received_2to4: Vec<Vec<TransmitInitZeroSharePhase2to4>> = Vec::new();
    for i in 1..=parameters.share_count {
        let mut new_row: Vec<TransmitInitZeroSharePhase2to4> = Vec::new();
        for party in &zero_transmit_2to4 {
            for message in party {
                if message.parties.receiver == i {
                    new_row.push(message.clone());
                }
            }
        }
        zero_received_2to4.push(new_row);
    }

    // Phase 3: Continue initialization
    let mut zero_kept_3to4: Vec<BTreeMap<u8, KeepInitZeroSharePhase3to4>> = Vec::new();
    let mut zero_transmit_3to4: Vec<Vec<TransmitInitZeroSharePhase3to4>> = Vec::new();
    let mut mul_kept_3to4: Vec<BTreeMap<u8, KeepInitMulPhase3to4>> = Vec::new();
    let mut mul_transmit_3to4: Vec<Vec<TransmitInitMulPhase3to4>> = Vec::new();
    let mut bip_broadcast_3to4: BTreeMap<u8, BroadcastDerivationPhase3to4> = BTreeMap::new();
    
    for i in 0..parameters.share_count {
        let (out1, out2, out3, out4, out5) = phase3(
            &all_data[i as usize],
            &zero_kept_2to3[i as usize],
            &bip_kept_2to3[i as usize],
        );
        zero_kept_3to4.push(out1);
        zero_transmit_3to4.push(out2);
        mul_kept_3to4.push(out3);
        mul_transmit_3to4.push(out4);
        bip_broadcast_3to4.insert(i + 1, out5);
    }

    // Communication round 3: Exchange final initialization data
    let mut zero_received_3to4: Vec<Vec<TransmitInitZeroSharePhase3to4>> = Vec::new();
    let mut mul_received_3to4: Vec<Vec<TransmitInitMulPhase3to4>> = Vec::new();
    
    for i in 1..=parameters.share_count {
        let mut zero_row: Vec<TransmitInitZeroSharePhase3to4> = Vec::new();
        let mut mul_row: Vec<TransmitInitMulPhase3to4> = Vec::new();
        
        for party in &zero_transmit_3to4 {
            for message in party {
                if message.parties.receiver == i {
                    zero_row.push(message.clone());
                }
            }
        }
        
        for party in &mul_transmit_3to4 {
            for message in party {
                if message.parties.receiver == i {
                    mul_row.push(message.clone());
                }
            }
        }
        
        zero_received_3to4.push(zero_row);
        mul_received_3to4.push(mul_row);
    }

    // Phase 4: Complete DKG and create parties
    let mut parties: Vec<Party> = Vec::new();
    for i in 0..parameters.share_count {
        let result = phase4(
            &all_data[i as usize],
            &poly_points[i as usize],
            &proofs_commitments,
            &zero_kept_3to4[i as usize],
            &zero_received_2to4[i as usize],
            &zero_received_3to4[i as usize],
            &mul_kept_3to4[i as usize],
            &mul_received_3to4[i as usize],
            &bip_broadcast_2to4,
            &bip_broadcast_3to4,
        );
        
        match result {
            Err(abort) => {
                return Err(format!("DKG failed: {}", abort.description).into());
            }
            Ok(mut party) => {
                // Update the network and address for the specific network
                party.network = network.clone();
                party.btc_address = compute_bitcoin_address(&party.pk, &network);
                parties.push(party);
            }
        }
    }

    Ok(parties)
}

/// Signs a message with parties from a specific network
fn sign_with_network(
    parties: &[Party],
    message_hash: &[u8; 32],
    sign_id: &str,
) -> Result<(String, String, u8), Box<dyn std::error::Error>> {
    let sign_id = sign_id.as_bytes().to_vec();
    
    // Select 2 parties to participate in signing (threshold = 2)
    let executing_parties: Vec<u8> = vec![1, 2]; // Parties 1 and 2 will sign
    
    // Prepare signing data for each participating party
    let mut all_sign_data: BTreeMap<u8, SignData> = BTreeMap::new();
    for party_index in &executing_parties {
        let mut counterparties = executing_parties.clone();
        counterparties.retain(|index| *index != *party_index);
        
        all_sign_data.insert(
            *party_index,
            SignData {
                sign_id: sign_id.clone(),
                counterparties,
                message_hash: *message_hash,
            },
        );
    }

    // Phase 1: Each party prepares for signing
    let mut unique_kept_1to2: BTreeMap<u8, UniqueKeep1to2> = BTreeMap::new();
    let mut kept_1to2: BTreeMap<u8, BTreeMap<u8, KeepPhase1to2>> = BTreeMap::new();
    let mut transmit_1to2: BTreeMap<u8, Vec<TransmitPhase1to2>> = BTreeMap::new();
    
    for party_index in &executing_parties {
        let (unique_keep, keep, transmit) = parties[(*party_index - 1) as usize]
            .sign_phase1(all_sign_data.get(party_index).unwrap());
        
        unique_kept_1to2.insert(*party_index, unique_keep);
        kept_1to2.insert(*party_index, keep);
        transmit_1to2.insert(*party_index, transmit);
    }

    // Communication round 1: Exchange signing messages
    let mut received_1to2: BTreeMap<u8, Vec<TransmitPhase1to2>> = BTreeMap::new();
    for &party_index in &executing_parties {
        let messages_for_party: Vec<TransmitPhase1to2> = transmit_1to2
            .values()
            .flatten()
            .filter(|message| message.parties.receiver == party_index)
            .cloned()
            .collect();
        received_1to2.insert(party_index, messages_for_party);
    }

    // Phase 2: Continue signing protocol
    let mut unique_kept_2to3: BTreeMap<u8, UniqueKeep2to3> = BTreeMap::new();
    let mut kept_2to3: BTreeMap<u8, BTreeMap<u8, KeepPhase2to3>> = BTreeMap::new();
    let mut transmit_2to3: BTreeMap<u8, Vec<TransmitPhase2to3>> = BTreeMap::new();
    
    for party_index in &executing_parties {
        let result = parties[(*party_index - 1) as usize].sign_phase2(
            all_sign_data.get(party_index).unwrap(),
            unique_kept_1to2.get(party_index).unwrap(),
            kept_1to2.get(party_index).unwrap(),
            received_1to2.get(party_index).unwrap(),
        );
        
        match result {
            Err(abort) => {
                return Err(format!("Signing failed: {}", abort.description).into());
            }
            Ok((unique_keep, keep, transmit)) => {
                unique_kept_2to3.insert(*party_index, unique_keep);
                kept_2to3.insert(*party_index, keep);
                transmit_2to3.insert(*party_index, transmit);
            }
        }
    }

    // Communication round 2: Exchange more signing messages
    let mut received_2to3: BTreeMap<u8, Vec<TransmitPhase2to3>> = BTreeMap::new();
    for &party_index in &executing_parties {
        let messages_for_party: Vec<TransmitPhase2to3> = transmit_2to3
            .values()
            .flatten()
            .filter(|message| message.parties.receiver == party_index)
            .cloned()
            .collect();
        received_2to3.insert(party_index, messages_for_party);
    }

    // Phase 3: Generate signature components
    let mut x_coords: Vec<String> = Vec::new();
    let mut broadcast_3to4: Vec<Broadcast3to4> = Vec::new();
    
    for party_index in &executing_parties {
        let result = parties[(*party_index - 1) as usize].sign_phase3(
            all_sign_data.get(party_index).unwrap(),
            unique_kept_2to3.get(party_index).unwrap(),
            kept_2to3.get(party_index).unwrap(),
            received_2to3.get(party_index).unwrap(),
        );
        
        match result {
            Err(abort) => {
                return Err(format!("Signing phase 3 failed: {}", abort.description).into());
            }
            Ok((x_coord, broadcast)) => {
                x_coords.push(x_coord);
                broadcast_3to4.push(broadcast);
            }
        }
    }

    // Verify all parties got the same x coordinate
    let x_coord = x_coords[0].clone();
    for i in 1..x_coords.len() {
        assert_eq!(x_coord, x_coords[i]);
    }

    // Phase 4: Complete the signature
    let some_index = executing_parties[0];
    let result = parties[(some_index - 1) as usize].sign_phase4(
        all_sign_data.get(&some_index).unwrap(),
        &x_coord,
        &broadcast_3to4,
        true, // normalize signature
    );
    
    let (signature, recovery_id) = match result {
        Err(abort) => {
            return Err(format!("Final signing failed: {}", abort.description).into());
        }
        Ok(sig) => sig,
    };

    Ok((x_coord, signature, recovery_id))
}
