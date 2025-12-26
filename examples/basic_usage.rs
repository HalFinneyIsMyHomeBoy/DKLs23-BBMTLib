//! Basic usage example of the DKLs23 Threshold ECDSA library.
//! 
//! This example demonstrates:
//! 1. Setting up a threshold signature scheme (3-of-5)
//! 2. Performing distributed key generation (DKG)
//! 3. Creating a threshold signature
//! 4. Verifying the signature

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::Parameters;  
use dkls23::utilities::hashes::hash;
use time::Instant;

fn main() -> Result<(), String> {
    let start = Instant::now();
    let params = Parameters { threshold: 2, share_count: 3 }; // 40-of-50 threshold scheme
    let parties = run_dkg_offline(&params, b"example_session").map_err(|e| format!("dkg failed: {}", e.description))?;
    let msg = hash(b"Hello, Threshold ECDSA!", &[]); // test message to sign

    // sign with 7 parties
    let (r, s, recid) = threshold_sign(&parties, &[1, 2], b"sign_session", msg, true) 
        .map_err(|e| format!("sign failed: {}", e.description))?;

    // print the signature and address to verify success
    println!("r={} s={} recid={} addr={}", r, s, recid, parties[0].btc_address);
    let duration = start.elapsed();
    let total_seconds = duration.as_seconds_f64();
    let minutes = (total_seconds / 60.0) as u64;
    let seconds = (total_seconds % 60.0) as u64;
    println!("Time taken: {} minutes and {} seconds", minutes, seconds);
    
    Ok(())
}
