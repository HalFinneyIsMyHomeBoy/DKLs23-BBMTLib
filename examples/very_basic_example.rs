//! Very basic example demonstrating DKLs23 Threshold ECDSA library.
//! 
//! This example shows the fundamental concepts and basic usage
//! of the DKLs23 library in a simple, easy-to-understand way.

use dkls23::utilities::hashes::hash;
use dkls23::utilities::rng;
use k256::elliptic_curve::Field;
use k256::elliptic_curve::group::GroupEncoding;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DKLs23 Very Basic Example");
    println!("============================");

    // Step 1: Show what DKLs23 is about
    println!("\n📖 What is DKLs23?");
    println!("   DKLs23 is a threshold ECDSA (Elliptic Curve Digital Signature Algorithm)");
    println!("   library that allows multiple parties to jointly generate and use");
    println!("   cryptographic keys without any single party having access to the");
    println!("   complete private key.");

    // Step 2: Demonstrate basic cryptographic building blocks
    println!("\n🔧 Basic Cryptographic Operations:");

    // Generate a random number (simulating a private key component)
    let mut rng = rng::get_rng();
    let secret_value = k256::Scalar::random(&mut rng);
    println!("   🔐 Generated secret value: {}", hex::encode(secret_value.to_bytes().as_slice()));

    // Create a public key from the secret
    let public_key = k256::AffinePoint::GENERATOR * secret_value;
    println!("   🔑 Corresponding public key: {}", hex::encode(public_key.to_bytes().as_slice()));

    // Step 3: Demonstrate message hashing
    println!("\n📝 Message Hashing:");
    let message = "Hello from DKLs23!";
    let message_hash = hash(message.as_bytes(), &[]);
    println!("   📄 Message: \"{}\"", message);
    println!("   🔢 Hash: {}", hex::encode(message_hash));

    // Step 4: Show library capabilities
    println!("\n🚀 DKLs23 Library Capabilities:");
    println!("   ✅ Distributed Key Generation (DKG)");
    println!("   ✅ Threshold Signing (e.g., 2-of-3 parties needed to sign)");
    println!("   ✅ Key Refresh (update shares without changing the public key)");
    println!("   ✅ BIP-32 Key Derivation (hierarchical deterministic keys)");
    println!("   ✅ Zero-knowledge proofs (prove knowledge without revealing secrets)");
    println!("   ✅ Oblivious Transfer (secure data exchange)");
    println!("   ✅ Multi-party multiplication (compute products of secrets)");

    // Step 5: Show security parameters
    println!("\n🛡️  Security Configuration:");
    println!("   🔒 Raw security parameter: {} bits", dkls23::RAW_SECURITY);
    println!("   🔒 Security parameter: {} bytes", dkls23::SECURITY);
    println!("   🔒 Statistical security: {} bits", dkls23::STAT_SECURITY);

    // Step 6: Demonstrate a simple threshold scheme concept
    println!("\n🎯 Threshold Scheme Example:");
    let threshold = 2;
    let total_parties = 3;
    println!("   📊 {}-of-{} threshold scheme", threshold, total_parties);
    println!("   💡 This means:");
    println!("      - {} parties are needed to create a signature", threshold);
    println!("      - {} parties total can participate", total_parties);
    println!("      - Even if {} party is compromised, the system is still secure", total_parties - threshold);

    // Step 7: Show what happens in a real scenario
    println!("\n🔄 Real-World Scenario:");
    println!("   🏦 Bank: 3 executives, need 2 to approve Bitcoin transactions");
    println!("   🔐 Bitcoin wallet: 3 devices, need 2 to sign transactions");
    println!("   🏢 Corporate: 5 board members, need 3 to approve Bitcoin decisions");
    println!("   🛡️  Security: Even if 1-2 parties are compromised, Bitcoin funds remain safe");

    // Step 8: Show how to run more complex examples
    println!("\n📚 Next Steps:");
    println!("   To see a complete working example with actual DKG and signing:");
    println!("   💻 cargo run --example basic_usage");
    println!("   ");
    println!("   To see a simpler demonstration:");
    println!("   💻 cargo run --example simple_demo");

    println!("\n🎉 Basic example completed successfully!");
    println!("   The DKLs23 library is ready to use for secure multi-party cryptography!");

    Ok(())
}
