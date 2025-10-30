//! CLI tool for DKLs23 Threshold ECDSA library.
//! 
//! This CLI provides a command-line interface that can be used from any language.
//! All input and output uses JSON format for easy integration.

use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json;

use dkls23::facade::{run_dkg_offline, threshold_sign};
use dkls23::protocols::{Network, Parameters, Party};
use dkls23::utilities::hashes::{hash, HashOutput};

/// CLI for DKLs23 Threshold ECDSA operations
#[derive(Parser)]
#[command(name = "dkls23-cli")]
#[command(about = "DKLs23 Threshold ECDSA CLI - Use from any language via JSON", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Suppress progress output (only output JSON)
    #[arg(short, long)]
    quiet: bool,
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
        
        /// Session ID as hex-encoded string
        #[arg(long, default_value = "")]
        session_id: String,
        
        /// Network type: "mainnet" or "testnet3"
        #[arg(long, default_value = "mainnet")]
        network: String,
        
        /// Include full parties data in output (default: summary only)
        #[arg(long)]
        include_parties: bool,
    },
    
    /// Create a threshold signature
    Sign {
        /// Path to JSON file containing parties (or read from stdin if not provided)
        #[arg(short, long)]
        parties: Option<PathBuf>,
        
        /// Comma-separated list of party indices to participate (e.g., "1,2,3")
        #[arg(long)]
        executing_parties: String,
        
        /// Message to sign (as hex-encoded string or plain text)
        #[arg(long)]
        message: String,
        
        /// Message is hex-encoded (if not set, message is treated as plain text)
        #[arg(long)]
        message_hex: bool,
        
        /// Sign ID as hex-encoded string
        #[arg(long, default_value = "")]
        sign_id: String,
        
        /// Normalize s value (low-s) to comply with Bitcoin standards
        #[arg(long, default_value = "true")]
        normalize_low_s: bool,
    },
}

/// JSON structure for DKG output (summary)
#[derive(Serialize, Deserialize)]
struct DkgOutput {
    success: bool,
    parties: Option<Vec<Party>>,  // Only included if --include-parties is used
    party_count: Option<u8>,
    error: Option<String>,
    bitcoin_address: Option<String>,
    network: Option<String>,
    threshold: Option<u8>,
    share_count: Option<u8>,
}

/// JSON structure for signing output
#[derive(Serialize, Deserialize)]
struct SignOutput {
    success: bool,
    r: Option<String>,
    s: Option<String>,
    recid: Option<u8>,
    error: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    
    if !cli.quiet {
        eprintln!("🔐 DKLs23 Threshold ECDSA CLI");
    }
    
    let result = match &cli.command {
        Commands::Dkg { threshold, share_count, session_id, network, include_parties } => {
            handle_dkg(*threshold, *share_count, session_id, network, *include_parties, cli.quiet)
        }
        Commands::Sign { 
            parties, 
            executing_parties, 
            message, 
            message_hex,
            sign_id,
            normalize_low_s,
        } => {
            handle_sign(
                parties,
                executing_parties,
                message,
                *message_hex,
                sign_id,
                *normalize_low_s,
                cli.quiet,
            )
        }
    };
    
    match result {
        Ok(output_json) => {
            println!("{}", output_json);
        }
        Err(e) => {
            let error_output = ErrorOutput {
                success: false,
                error: Some(e),
            };
            eprintln!("{}", serde_json::to_string_pretty(&error_output).unwrap());
            std::process::exit(1);
        }
    }
}

/// Generic error output structure
#[derive(Serialize, Deserialize)]
struct ErrorOutput {
    success: bool,
    error: Option<String>,
}

fn handle_dkg(
    threshold: u8,
    share_count: u8,
    session_id: &str,
    network: &str,
    include_parties: bool,
    quiet: bool,
) -> Result<String, String> {
    // Parse network (currently only Mainnet is supported by the facade, but we'll document testnet3 for future)
    let _network_type = match network.to_lowercase().as_str() {
        "mainnet" => Network::Mainnet,
        "testnet3" | "testnet" => Network::Testnet3,
        _ => {
            return Ok(serde_json::to_string_pretty(&DkgOutput {
                success: false,
                parties: None,
                party_count: None,
                error: Some(format!("Invalid network: '{}'. Must be 'mainnet' or 'testnet3'", network)),
                bitcoin_address: None,
                network: None,
                threshold: None,
                share_count: None,
            }).unwrap());
        }
    };
    
    // Note: Currently the facade always generates Mainnet addresses
    // This is a limitation of run_dkg_offline which doesn't accept a network parameter
    if !quiet && network.to_lowercase() != "mainnet" {
        eprintln!("⚠️  Warning: Network parameter '{}' is ignored. DKG always generates Mainnet addresses.", network);
    }
    
    // Validate parameters
    if threshold == 0 || threshold > share_count {
        return Ok(serde_json::to_string_pretty(&DkgOutput {
            success: false,
            parties: None,
            party_count: None,
            error: Some(format!(
                "Invalid parameters: threshold ({}) must be > 0 and <= share_count ({})",
                threshold, share_count
            )),
            bitcoin_address: None,
            network: None,
            threshold: None,
            share_count: None,
        }).unwrap());
    }
    
    if share_count == 0 {
        return Ok(serde_json::to_string_pretty(&DkgOutput {
            success: false,
            parties: None,
            party_count: None,
            error: Some(format!(
                "Invalid share_count: must be > 0, got {}",
                share_count
            )),
            bitcoin_address: None,
            network: None,
            threshold: None,
            share_count: None,
        }).unwrap());
    }
    
    // Parse session ID
    let session_id_bytes = if session_id.is_empty() {
        if !quiet {
            eprintln!("⚠️  Warning: Using empty session_id");
        }
        vec![]
    } else {
        hex::decode(session_id).map_err(|e| format!("Invalid hex session_id: {}", e))?
    };
    
    let parameters = Parameters {
        threshold,
        share_count,
    };
    
    // Redirect stdout to stderr during DKG to avoid interfering with JSON output
    if quiet {
        // Temporarily disable stdout prints in facade
        // Since we can't easily control facade's prints, we'll just let them go to stderr
        // Users can redirect stderr if they want to suppress them
    }
    
    match run_dkg_offline(&parameters, &session_id_bytes) {
        Ok(parties) => {
            if parties.is_empty() {
                return Ok(serde_json::to_string_pretty(&DkgOutput {
                    success: false,
                    parties: None,
                    party_count: None,
                    error: Some("DKG completed but no parties were generated".to_string()),
                    bitcoin_address: None,
                    network: None,
                    threshold: None,
                    share_count: None,
                }).unwrap());
            }
            
            let bitcoin_address = parties[0].btc_address.clone();
            let network_str = format!("{:?}", parties[0].network);
            let party_count = parties.len() as u8;
            
            Ok(serde_json::to_string_pretty(&DkgOutput {
                success: true,
                parties: if include_parties { Some(parties) } else { None },
                party_count: Some(party_count),
                error: None,
                bitcoin_address: Some(bitcoin_address),
                network: Some(network_str),
                threshold: Some(threshold),
                share_count: Some(share_count),
            }).unwrap())
        }
        Err(abort) => Ok(serde_json::to_string_pretty(&DkgOutput {
            success: false,
            parties: None,
            party_count: None,
            error: Some(format!("DKG aborted by party {}: {}", abort.index, abort.description)),
            bitcoin_address: None,
            network: None,
            threshold: None,
            share_count: None,
        }).unwrap()),
    }
}

fn handle_sign(
    parties_path: &Option<PathBuf>,
    executing_parties: &str,
    message: &str,
    message_hex: bool,
    sign_id: &str,
    normalize_low_s: bool,
    quiet: bool,
) -> Result<String, String> {
    // Read parties
    let parties_json = if let Some(path) = parties_path {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read parties file: {}", e))?
    } else {
        // Read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        input
    };
    
    let parties: Vec<Party> = serde_json::from_str(&parties_json)
        .map_err(|e| format!("Failed to parse parties JSON: {}", e))?;
    
    if parties.is_empty() {
        return Ok(serde_json::to_string_pretty(&SignOutput {
            success: false,
            r: None,
            s: None,
            recid: None,
            error: Some("No parties provided".to_string()),
        }).unwrap());
    }
    
    // Parse executing parties
    let executing_parties: Vec<u8> = executing_parties
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Invalid executing_parties format: {}", e))?;
    
    if executing_parties.is_empty() {
        return Ok(serde_json::to_string_pretty(&SignOutput {
            success: false,
            r: None,
            s: None,
            recid: None,
            error: Some("No executing parties specified".to_string()),
        }).unwrap());
    }
    
    // Validate party indices
    for &idx in &executing_parties {
        if idx == 0 || idx as usize > parties.len() {
            return Ok(serde_json::to_string_pretty(&SignOutput {
                success: false,
                r: None,
                s: None,
                recid: None,
                error: Some(format!(
                    "Invalid party index {}: must be between 1 and {}",
                    idx, parties.len()
                )),
            }).unwrap());
        }
    }
    
    // Check we have enough parties
    let threshold = parties[0].parameters.threshold;
    if executing_parties.len() < threshold as usize {
        return Ok(serde_json::to_string_pretty(&SignOutput {
            success: false,
            r: None,
            s: None,
            recid: None,
            error: Some(format!(
                "Not enough parties: need at least {} (threshold), got {}",
                threshold,
                executing_parties.len()
            )),
        }).unwrap());
    }
    
    // Parse message
    let message_hash: HashOutput = if message_hex {
        let bytes = hex::decode(message)
            .map_err(|e| format!("Invalid hex message: {}", e))?;
        if bytes.len() != 32 {
            return Ok(serde_json::to_string_pretty(&SignOutput {
                success: false,
                r: None,
                s: None,
                recid: None,
                error: Some(format!(
                    "Message hash must be 32 bytes (got {} bytes)",
                    bytes.len()
                )),
            }).unwrap());
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        hash
    } else {
        hash(message.as_bytes(), &[])
    };
    
    // Parse sign_id
    let sign_id_bytes = if sign_id.is_empty() {
        vec![]
    } else {
        hex::decode(sign_id).map_err(|e| format!("Invalid hex sign_id: {}", e))?
    };
    
    if !quiet {
        eprintln!("✍️  Signing with parties: {:?}", executing_parties);
    }
    
    match threshold_sign(
        &parties,
        &executing_parties,
        &sign_id_bytes,
        message_hash,
        normalize_low_s,
    ) {
        Ok((r, s, recid)) => Ok(serde_json::to_string_pretty(&SignOutput {
            success: true,
            r: Some(r),
            s: Some(s),
            recid: Some(recid),
            error: None,
        }).unwrap()),
        Err(abort) => Ok(serde_json::to_string_pretty(&SignOutput {
            success: false,
            r: None,
            s: None,
            recid: None,
            error: Some(format!("Signing aborted by party {}: {}", abort.index, abort.description)),
        }).unwrap()),
    }
}

