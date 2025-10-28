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

fn main() -> Result<(), String> {
    let params = Parameters { threshold: 2, share_count: 3 };
    let parties = run_dkg_offline(&params, b"example_session").map_err(|e| format!("dkg failed: {}", e.description))?;
    let msg = hash(b"Hello, Threshold ECDSA!", &[]);
    let (r, s, recid) = threshold_sign(&parties, &[1, 2], b"sign_session", msg, true)
        .map_err(|e| format!("sign failed: {}", e.description))?;
    println!("r={} s={} recid={} addr={}", r, s, recid, parties[0].btc_address);
    Ok(())
}
