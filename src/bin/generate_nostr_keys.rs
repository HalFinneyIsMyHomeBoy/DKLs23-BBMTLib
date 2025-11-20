//! Generate Nostr nsec and npub keys
//! 
//! This program generates a random Nostr key pair and outputs them as nsec and npub.
//! 
//! Usage: cargo run --bin generate_nostr_keys

use bech32::{self, Variant};
use k256::elliptic_curve::{ops::Reduce, point::AffineCoordinates};
use k256::{AffinePoint, Scalar, U256};
use rand::RngCore;

fn main() {
    // Generate random 32-byte private key
    let mut private_key_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut private_key_bytes);
    
    // Convert to Scalar (secp256k1 private key)
    // Convert bytes to U256 and reduce modulo curve order
    let private_key_int = U256::from_be_slice(&private_key_bytes);
    let private_key = Scalar::reduce(private_key_int);
    
    // Derive public key: P = private_key * G
    let public_key_point = (AffinePoint::GENERATOR * private_key).to_affine();
    
    // Get the x-coordinate (32 bytes) for npub
    // Nostr npub uses the 32-byte x-coordinate of the public key
    let x_coord_bytes = public_key_point.x().as_slice().to_vec();
    
    // Encode private key as nsec
    let nsec = encode_nostr_key("nsec", &private_key_bytes);
    
    // Encode public key x-coordinate as npub
    let npub = encode_nostr_key("npub", &x_coord_bytes);
    
    // Output as JSON for easy parsing
    println!("{}", serde_json::json!({
        "nsec": nsec,
        "npub": npub
    }));
}

/// Encode bytes as a Nostr bech32 key (nsec or npub)
fn encode_nostr_key(prefix: &str, data: &[u8]) -> String {
    // Convert 8-bit bytes to 5-bit base32
    let base32_data = bech32::convert_bits(data, 8, 5, true)
        .expect("Failed to convert bits");
    
    // Convert Vec<u8> to Vec<u5>
    let base32_u5: Vec<bech32::u5> = base32_data
        .iter()
        .map(|&b| bech32::u5::try_from_u8(b).expect("Invalid 5-bit value"))
        .collect();
    
    // Encode as bech32
    bech32::encode(prefix, &base32_u5, Variant::Bech32)
        .expect("Failed to encode bech32")
}

