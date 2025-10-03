//! Simple demonstration of DKLs23 Threshold ECDSA library.
//! 
//! This example shows the basic concepts without the full complexity
//! of the multi-party protocol.

use dkls23::protocols::*;
use dkls23::utilities::hashes::hash;
use dkls23::utilities::rng;
use k256::elliptic_curve::Field;
use k256::elliptic_curve::group::GroupEncoding;

fn main() {
    println!("🔐 DKLs23 Threshold ECDSA Simple Demo");
    println!("====================================");

    // Step 1: Create parameters for a 2-of-3 threshold scheme
    let parameters = Parameters {
        threshold: 2,      // Need 2 parties to sign
        share_count: 3,    // Total of 3 parties
    };
    
    println!("📋 Threshold scheme: {}-of-{}", parameters.threshold, parameters.share_count);

    // Step 2: Create a session ID
    let session_id = b"demo_session".to_vec();
    println!("🆔 Session ID: {}", hex::encode(&session_id));

    // Step 3: Demonstrate basic cryptographic operations
    println!("\n🔑 Basic Cryptographic Operations:");
    
    // Generate a random scalar (like a private key)
    let mut rng = rng::get_rng();
    let secret_scalar = k256::Scalar::random(&mut rng);
    println!("🔐 Generated secret scalar: {}", hex::encode(secret_scalar.to_bytes().as_slice()));

    // Create a public key from the scalar
    let public_key = k256::AffinePoint::GENERATOR * secret_scalar;
    println!("🔑 Public key: {}", hex::encode(public_key.to_bytes().as_slice()));

    // Step 4: Demonstrate hashing
    let message = "Hello, DKLs23!";
    let message_hash = hash(message.as_bytes(), &[]);
    println!("📝 Message: {}", message);
    println!("🔢 Message hash: {}", hex::encode(message_hash));

    // Step 5: Show what the library provides
    println!("\n📚 Library Features:");
    println!("   ✅ Distributed Key Generation (DKG)");
    println!("   ✅ Threshold Signing");
    println!("   ✅ Key Refresh");
    println!("   ✅ BIP-32 Key Derivation (hierarchical deterministic Bitcoin keys)");
    println!("   ✅ Zero-knowledge proofs");
    println!("   ✅ Oblivious Transfer");
    println!("   ✅ Multi-party multiplication");

    // Step 6: Show security parameters
    println!("\n🛡️  Security Parameters:");
    println!("   🔒 Raw security: {} bits", dkls23::RAW_SECURITY);
    println!("   🔒 Security: {} bytes", dkls23::SECURITY);
    println!("   🔒 Statistical security: {} bits", dkls23::STAT_SECURITY);

    println!("\n💡 To see a full working example, run:");
    println!("   cargo run --example basic_usage");
    
    println!("\n🎉 Demo completed!");
}
