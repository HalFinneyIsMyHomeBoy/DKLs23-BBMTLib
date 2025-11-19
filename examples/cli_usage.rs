//! CLI usage example of the DKLs23 Threshold ECDSA library.
//! 
//! This example demonstrates:
//! 1. Command-line interface for key generation (DKG)
//! 2. Command-line interface for threshold signing
//!
//! Run with:
//!   cargo run --example cli_usage -- dkg --threshold 2 --share-count 3
//!   cargo run --example cli_usage -- sign --parties-dir parties --executing-parties 1,2 --message "Hello, World!"

use clap::{Parser, Subcommand};
use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::Parameters;
use dkls23::utilities::hashes::hash;
use std::fs;
use std::path::PathBuf;
use time::Instant;

/// CLI for DKLs23 Threshold ECDSA operations
#[derive(Parser)]
#[command(name = "cli_usage")]
#[command(about = "DKLs23 Threshold ECDSA CLI Example")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run Distributed Key Generation (DKG)
    Dkg {
        /// Threshold value (t) - minimum number of parties needed to sign
        #[arg(long)]
        threshold: u8,
        
        /// Total number of parties (n)
        #[arg(long)]
        share_count: u8,
        
        /// Session ID for DKG
        #[arg(long, default_value = "dkg_session")]
        session_id: String,
        
        /// Output directory to save party files (each party saved as party_N.json)
        #[arg(long, default_value = "parties")]
        output_dir: PathBuf,
    },
    
    /// Create a threshold signature
    Sign {
        /// Directory containing party files (party_N.json)
        #[arg(long, default_value = "parties")]
        parties_dir: PathBuf,
        
        /// Comma-separated list of party indices to participate (e.g., "1,2")
        #[arg(long)]
        executing_parties: String,
        
        /// Message to sign
        #[arg(long)]
        message: String,
        
        /// Session ID for signing
        #[arg(long, default_value = "sign_session")]
        sign_id: String,
    },
}

/// Function to handle key generation (DKG)
fn handle_key_generation(
    threshold: u8,
    share_count: u8,
    session_id: &str,
    output_dir: &PathBuf,
) -> Result<(), String> {
    println!("🔐 Starting Distributed Key Generation...");
    println!("   Threshold: {}/{}", threshold, share_count);
    println!("   Session ID: {}", session_id);
    
    let start = Instant::now();
    let params = Parameters { threshold, share_count };
    let parties = run_dkg_offline(&params, session_id.as_bytes())
        .map_err(|e| format!("dkg failed: {}", e.description))?;
    
    let duration = start.elapsed();
    let total_seconds = duration.as_seconds_f64();
    let minutes = (total_seconds / 60.0) as u64;
    let seconds = (total_seconds % 60.0) as u64;
    println!("Time taken for KeyGen: {} minutes and {} seconds", minutes, seconds);
    
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create directory {}: {}", output_dir.display(), e))?;
    
    // Save each party to its own file
    for (index, party) in parties.iter().enumerate() {
        let party_index = index + 1; // 1-based indexing
        let party_file = output_dir.join(format!("party_{}.json", party_index));
        
        let json = serde_json::to_string_pretty(party)
            .map_err(|e| format!("Failed to serialize party {}: {}", party_index, e))?;
        
        fs::write(&party_file, json)
            .map_err(|e| format!("Failed to write to {}: {}", party_file.display(), e))?;
        
        println!("   💾 Saved party {} to: {}", party_index, party_file.display());
    }
    
    println!("✅ Key generation completed successfully!");
    println!("   Parties saved to directory: {}", output_dir.display());
    println!("   Bitcoin address: {}", parties[0].btc_address);
    
    Ok(())
}

/// Function to handle threshold signing
fn handle_signing(
    parties_dir: &PathBuf,
    executing_parties: &str,
    message: &str,
    sign_id: &str,
) -> Result<(), String> {
    println!("✍️  Starting threshold signing...");
    println!("   Parties directory: {}", parties_dir.display());
    println!("   Executing parties: {}", executing_parties);
    println!("   Message: {}", message);
    println!("   Sign ID: {}", sign_id);
    
    // Parse executing party indices
    let party_indices: Vec<u8> = executing_parties
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Invalid party indices: {}", e))?;
    
    // Read all parties (we need all parties for threshold_sign, not just executing ones)
    // First, determine how many parties exist by checking for party files
    let mut all_parties: Vec<dkls23::protocols::Party> = Vec::new();
    let mut max_party_index = 0;
    
    // Try to find the maximum party index by checking files
    for i in 1..=255 {
        let party_file = parties_dir.join(format!("party_{}.json", i));
        if party_file.exists() {
            max_party_index = i;
        } else {
            break;
        }
    }
    
    if max_party_index == 0 {
        return Err(format!("No party files found in {}", parties_dir.display()));
    }
    
    // Read all party files
    for i in 1..=max_party_index {
        let party_file = parties_dir.join(format!("party_{}.json", i));
        let json = fs::read_to_string(&party_file)
            .map_err(|e| format!("Failed to read {}: {}", party_file.display(), e))?;
        
        let party: dkls23::protocols::Party = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse party {} JSON: {}", i, e))?;
        
        all_parties.push(party);
    }
    
    println!("   Loaded {} party files", all_parties.len());
    
    // Hash the message
    let msg = hash(message.as_bytes(), &[]);
    
    let start = Instant::now();
    // Perform threshold signing
    let (r, s, recid) = threshold_sign(&all_parties, &party_indices, sign_id.as_bytes(), msg, true)
        .map_err(|e| format!("sign failed: {}", e.description))?;
    
    let duration = start.elapsed();
    let total_seconds = duration.as_seconds_f64();
    let milliseconds = (total_seconds % 1.0) as u64;
    println!("Time taken for signing: {} seconds and {} milliseconds", total_seconds, milliseconds);
    
    // Print the signature and address to verify success (matching basic_usage.rs format)
    println!("r={} s={} recid={} addr={}", r, s, recid, all_parties[0].btc_address);
    
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    
    let result = match &cli.command {
        Commands::Dkg {
            threshold,
            share_count,
            session_id,
            output_dir,
        } => handle_key_generation(*threshold, *share_count, session_id, output_dir),
        
        Commands::Sign {
            parties_dir,
            executing_parties,
            message,
            sign_id,
        } => handle_signing(parties_dir, executing_parties, message, sign_id),
    };
    
    if let Err(e) = result {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
}

