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
    let params = Parameters { threshold: 2, share_count: 3 }; // 2-of-3 threshold scheme
    let parties = run_dkg_offline(&params, b"example_session").map_err(|e| format!("dkg failed: {}", e.description))?;
    let msg = hash(b"Hello, Threshold ECDSA!", &[]); // test message to sign

    let duration_keygen = start.elapsed();
    let total_seconds_keygen = duration_keygen.as_seconds_f64();
    let milliseconds_keygen = (total_seconds_keygen * 1000.0) as u64;


    let start_sign = Instant::now();
    // sign with 2 parties
    let (r, s, recid) = threshold_sign(&parties, &[1, 2], b"sign_session", msg, true) 
        .map_err(|e| format!("sign failed: {}", e.description))?;
    let duration_sign = start_sign.elapsed();
    let total_seconds_sign = duration_sign.as_seconds_f64();
    let milliseconds_sign = (total_seconds_sign * 1000.0) as u64;

    
    // print the signature and address to verify success
    println!("r={} s={} recid={} addr={}", r, s, recid, parties[0].btc_address);
    
    // Display detailed timing information
    println!("\nShare count: {}", params.share_count);
    println!("Threshold: {}", params.threshold);
    println!("=== Timing Summary ===");
    println!("Keygen:");
    println!("  Total seconds: {:.6}", total_seconds_keygen);
    println!("  Milliseconds: {}", milliseconds_keygen);
    println!("Sign:");
    println!("  Total seconds: {:.6}", total_seconds_sign);
    println!("  Milliseconds: {}", milliseconds_sign);

    
    let total_time = total_seconds_keygen + total_seconds_sign;
    println!("Total: {:.6} seconds", total_time);
    Ok(())
}
