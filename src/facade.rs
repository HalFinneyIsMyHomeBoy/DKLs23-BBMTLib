use std::collections::BTreeMap;

use k256::Scalar;
use rayon::prelude::*;

use crate::protocols::dkg::*;
use crate::protocols::signing::*;
use crate::protocols::*;
use crate::utilities::hashes::HashOutput;

/// Run dealerless DKG entirely offline and return initialized parties.
pub fn run_dkg_offline(
    parameters: &Parameters,
    session_id: &[u8],
) -> Result<Vec<Party>, Abort> {
    println!("🔐 Starting Distributed Key Generation...");
    println!("📋 Parameters: {}-of-{} threshold scheme", parameters.threshold, parameters.share_count);
    // Prepare per-party session data
    let mut all_data: Vec<SessionData> = Vec::with_capacity(parameters.share_count as usize);
    for i in 0..parameters.share_count {
        all_data.push(SessionData {
            parameters: parameters.clone(),
            party_index: i + 1,
            session_id: session_id.to_vec(),
        });
    }

    // Phase 1
    println!("📊 Phase 1: Generating polynomial fragments...");
    let phase1_outputs: Vec<Vec<Scalar>> = all_data
        .par_iter()
        .enumerate()
        .map(|(i, data)| {
            print!("   🔄 Processing party {}/{}...", i + 1, parameters.share_count);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = phase1(data);
            println!("\r   ✅ Party {}/{} completed", i + 1, parameters.share_count);
            result
        })
        .collect();

    // Communication round 1 → transpose rows to per-receiver columns
    print!("📡 Communication round 1: Exchanging polynomial fragments...");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut poly_fragments = vec![
        Vec::<Scalar>::with_capacity(parameters.share_count as usize);
        parameters.share_count as usize
    ];
    for row in phase1_outputs {
        for j in 0..parameters.share_count {
            poly_fragments[j as usize].push(row[j as usize]);
        }
    }
    println!(" ✅ Complete");

    // Phase 2
    println!("📊 Phase 2: Generating proofs and commitments...");
    let phase2_results: Vec<_> = all_data
        .par_iter()
        .zip(poly_fragments.par_iter())
        .enumerate()
        .map(|(i, (data, fragments))| {
            print!("   🔄 Processing party {}/{}...", i + 1, parameters.share_count);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = phase2(data, fragments);
            println!("\r   ✅ Party {}/{} completed", i + 1, parameters.share_count);
            result
        })
        .collect();
    
    // Extract results from parallel computation
    let mut poly_points: Vec<Scalar> = Vec::with_capacity(parameters.share_count as usize);
    let mut proofs_commitments: Vec<ProofCommitment> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut zero_kept_2to3: Vec<BTreeMap<u8, KeepInitZeroSharePhase2to3>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut zero_transmit_2to4: Vec<Vec<TransmitInitZeroSharePhase2to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut bip_kept_2to3: Vec<UniqueKeepDerivationPhase2to3> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut bip_broadcast_2to4: BTreeMap<u8, BroadcastDerivationPhase2to4> = BTreeMap::new();
    
    for (i, (out1, out2, out3, out4, out5, out6)) in phase2_results.into_iter().enumerate() {
        poly_points.push(out1);
        proofs_commitments.push(out2);
        zero_kept_2to3.push(out3);
        zero_transmit_2to4.push(out4);
        bip_kept_2to3.push(out5);
        bip_broadcast_2to4.insert(i as u8 + 1, out6);
    }

    // Communication round 2 → route zero commitments to receivers
    print!("📡 Communication round 2: Exchanging zero share commitments...");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut zero_received_2to4: Vec<Vec<TransmitInitZeroSharePhase2to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    for i in 1..=parameters.share_count {
        let mut row: Vec<TransmitInitZeroSharePhase2to4> =
            Vec::with_capacity((parameters.share_count - 1) as usize);
        for party_msgs in &zero_transmit_2to4 {
            for msg in party_msgs {
                if msg.parties.receiver == i {
                    row.push(msg.clone());
                }
            }
        }
        zero_received_2to4.push(row);
    }
    println!(" ✅ Complete");

    // Phase 3
    println!("📊 Phase 3: Continuing initialization...");
    let phase3_results: Vec<_> = all_data
        .par_iter()
        .zip(zero_kept_2to3.par_iter())
        .zip(bip_kept_2to3.par_iter())
        .enumerate()
        .map(|(i, ((data, zero_kept), bip_kept))| {
            print!("   🔄 Processing party {}/{}...", i + 1, parameters.share_count);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = phase3(data, zero_kept, bip_kept);
            println!("\r   ✅ Party {}/{} completed", i + 1, parameters.share_count);
            result
        })
        .collect();
    
    // Extract results from parallel computation
    let mut zero_kept_3to4: Vec<BTreeMap<u8, KeepInitZeroSharePhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut zero_transmit_3to4: Vec<Vec<TransmitInitZeroSharePhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut mul_kept_3to4: Vec<BTreeMap<u8, KeepInitMulPhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut mul_transmit_3to4: Vec<Vec<TransmitInitMulPhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut bip_broadcast_3to4: BTreeMap<u8, BroadcastDerivationPhase3to4> = BTreeMap::new();
    
    for (i, (out1, out2, out3, out4, out5)) in phase3_results.into_iter().enumerate() {
        zero_kept_3to4.push(out1);
        zero_transmit_3to4.push(out2);
        mul_kept_3to4.push(out3);
        mul_transmit_3to4.push(out4);
        bip_broadcast_3to4.insert(i as u8 + 1, out5);
    }

    // Communication round 3 → route zero/mul transmissions to receivers
    print!("📡 Communication round 3: Exchanging final initialization data...");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut zero_received_3to4: Vec<Vec<TransmitInitZeroSharePhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    let mut mul_received_3to4: Vec<Vec<TransmitInitMulPhase3to4>> =
        Vec::with_capacity(parameters.share_count as usize);
    for i in 1..=parameters.share_count {
        let mut zr: Vec<TransmitInitZeroSharePhase3to4> =
            Vec::with_capacity((parameters.share_count - 1) as usize);
        for party_msgs in &zero_transmit_3to4 {
            for msg in party_msgs {
                if msg.parties.receiver == i {
                    zr.push(msg.clone());
                }
            }
        }
        zero_received_3to4.push(zr);

        let mut mr: Vec<TransmitInitMulPhase3to4> =
            Vec::with_capacity((parameters.share_count - 1) as usize);
        for party_msgs in &mul_transmit_3to4 {
            for msg in party_msgs {
                if msg.parties.receiver == i {
                    mr.push(msg.clone());
                }
            }
        }
        mul_received_3to4.push(mr);
    }
    println!(" ✅ Complete");

    // Phase 4 → build parties
    println!("📊 Phase 4: Completing DKG and creating parties...");
    let parties: Result<Vec<Party>, Abort> = all_data
        .par_iter()
        .zip(poly_points.par_iter())
        .zip(zero_kept_3to4.par_iter())
        .zip(zero_received_2to4.par_iter())
        .zip(zero_received_3to4.par_iter())
        .zip(mul_kept_3to4.par_iter())
        .zip(mul_received_3to4.par_iter())
        .enumerate()
        .map(|(i, ((((((data, poly_point), zero_kept), zero_rec_2), zero_rec_3), mul_kept), mul_rec))| {
            print!("   🔄 Processing party {}/{}...", i + 1, parameters.share_count);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = phase4(
                data,
                poly_point,
                &proofs_commitments,
                zero_kept,
                zero_rec_2,
                zero_rec_3,
                mul_kept,
                mul_rec,
                &bip_broadcast_2to4,
                &bip_broadcast_3to4,
            );
            println!("\r   ✅ Party {}/{} completed", i + 1, parameters.share_count);
            result
        })
        .collect();
    
    let parties = parties?;
    
    println!("✅ DKG completed successfully!");
    println!("🌐 Network: {:?}", parties[0].network);
    println!("₿ Bitcoin address: {}", parties[0].btc_address);

    Ok(parties)
}

/// Run the full threshold signing flow for selected parties.
pub fn threshold_sign(
    parties: &[Party],
    executing_parties: &[u8],
    sign_id: &[u8],
    message_hash: HashOutput,
    normalize_low_s: bool,
) -> Result<(String, String, u8), Abort> {
    println!("✍️  Starting Threshold Signature...");
    println!("📝 Participating parties: {:?}", executing_parties);
    // Build SignData for each executing party
    let mut all_sign_data: BTreeMap<u8, SignData> = BTreeMap::new();
    for &party_index in executing_parties {
        let mut counterparties = executing_parties.to_vec();
        counterparties.retain(|idx| *idx != party_index);
        all_sign_data.insert(
            party_index,
            SignData {
                sign_id: sign_id.to_vec(),
                counterparties,
                message_hash,
            },
        );
    }

    // Phase 1
    println!("📊 Phase 1: Preparing for signing...");
    let phase1_results: Vec<_> = executing_parties
        .par_iter()
        .map(|&party_index| {
            print!("   🔄 Processing party {}...", party_index);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = parties[(party_index - 1) as usize]
                .sign_phase1(all_sign_data.get(&party_index).unwrap());
            println!("\r   ✅ Party {} completed", party_index);
            (party_index, result)
        })
        .collect();
    
    // Extract results
    let mut unique_kept_1to2: BTreeMap<u8, UniqueKeep1to2> = BTreeMap::new();
    let mut kept_1to2: BTreeMap<u8, BTreeMap<u8, KeepPhase1to2>> = BTreeMap::new();
    let mut transmit_1to2: BTreeMap<u8, Vec<TransmitPhase1to2>> = BTreeMap::new();
    for (party_index, (u, k, t)) in phase1_results {
        unique_kept_1to2.insert(party_index, u);
        kept_1to2.insert(party_index, k);
        transmit_1to2.insert(party_index, t);
    }

    // Communication round 1 → route to receivers
    print!("📡 Communication round 1: Exchanging signing messages...");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut received_1to2: BTreeMap<u8, Vec<TransmitPhase1to2>> = BTreeMap::new();
    for &party_index in executing_parties {
        let msgs: Vec<TransmitPhase1to2> = transmit_1to2
            .values()
            .flatten()
            .filter(|m| m.parties.receiver == party_index)
            .cloned()
            .collect();
        received_1to2.insert(party_index, msgs);
    }
    println!(" ✅ Complete");

    // Phase 2
    println!("📊 Phase 2: Continuing signing protocol...");
    let phase2_results: Result<Vec<_>, Abort> = executing_parties
        .par_iter()
        .map(|&party_index| {
            print!("   🔄 Processing party {}...", party_index);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = parties[(party_index - 1) as usize]
                .sign_phase2(
                    all_sign_data.get(&party_index).unwrap(),
                    unique_kept_1to2.get(&party_index).unwrap(),
                    kept_1to2.get(&party_index).unwrap(),
                    received_1to2.get(&party_index).unwrap(),
                );
            println!("\r   ✅ Party {} completed", party_index);
            result.map(|(u, k, t)| (party_index, u, k, t))
        })
        .collect();
    
    let phase2_results = phase2_results?;
    
    // Extract results
    let mut unique_kept_2to3: BTreeMap<u8, UniqueKeep2to3> = BTreeMap::new();
    let mut kept_2to3: BTreeMap<u8, BTreeMap<u8, KeepPhase2to3>> = BTreeMap::new();
    let mut transmit_2to3: BTreeMap<u8, Vec<TransmitPhase2to3>> = BTreeMap::new();
    for (party_index, u, k, t) in phase2_results {
        unique_kept_2to3.insert(party_index, u);
        kept_2to3.insert(party_index, k);
        transmit_2to3.insert(party_index, t);
    }

    // Communication round 2 → route to receivers
    print!("📡 Communication round 2: Exchanging more signing messages...");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut received_2to3: BTreeMap<u8, Vec<TransmitPhase2to3>> = BTreeMap::new();
    for &party_index in executing_parties {
        let msgs: Vec<TransmitPhase2to3> = transmit_2to3
            .values()
            .flatten()
            .filter(|m| m.parties.receiver == party_index)
            .cloned()
            .collect();
        received_2to3.insert(party_index, msgs);
    }
    println!(" ✅ Complete");

    // Phase 3
    println!("📊 Phase 3: Generating signature components...");
    let phase3_results: Result<Vec<_>, Abort> = executing_parties
        .par_iter()
        .map(|&party_index| {
            print!("   🔄 Processing party {}...", party_index);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            let result = parties[(party_index - 1) as usize].sign_phase3(
                all_sign_data.get(&party_index).unwrap(),
                unique_kept_2to3.get(&party_index).unwrap(),
                kept_2to3.get(&party_index).unwrap(),
                received_2to3.get(&party_index).unwrap(),
            );
            println!("\r   ✅ Party {} completed", party_index);
            result.map(|(x, b)| (party_index, x, b))
        })
        .collect();
    
    let phase3_results = phase3_results?;
    
    // Extract results
    let mut x_coords: Vec<String> = Vec::with_capacity(executing_parties.len());
    let mut broadcast_3to4: Vec<Broadcast3to4> =
        Vec::with_capacity(executing_parties.len());
    for (_, x, b) in phase3_results {
        x_coords.push(x);
        broadcast_3to4.push(b);
    }
    let r_hex = x_coords[0].clone();

    // Phase 4 (one party computes final s and recid)
    println!("📊 Phase 4: Completing signature...");
    let some_index = executing_parties[0];
    let (s_hex, rec_id) = parties[(some_index - 1) as usize].sign_phase4(
        all_sign_data.get(&some_index).unwrap(),
        &r_hex,
        &broadcast_3to4,
        normalize_low_s,
    )?;
    
    println!("✅ Threshold signature completed!");
    println!("📝 Signature r: {}", r_hex);
    println!("📝 Signature s: {}", s_hex);
    println!("🆔 Recovery ID: {}", rec_id);

    Ok((r_hex, s_hex, rec_id))
}


