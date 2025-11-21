//! Distributed Key Generation protocol for multi-party execution.
//!
//! This module provides an API for running DKG where each party runs as a separate process.
//! Each party can generate its own keyshare locally and communicate with other parties
//! via serializable messages.
//!
//! # Usage
//!
//! Each party should:
//! 1. Initialize with `DkgPartyState::new()`
//! 2. Call `start_phase1()` to generate initial messages
//! 3. Send messages to other parties and receive their messages
//! 4. Call `process_phase1_messages()` and `start_phase2()` to continue
//! 5. Repeat for phases 3 and 4 until DKG is complete
//!
//! # Example
//!
//! ```no_run
//! use dkls23::protocols::dkg_distributed::*;
//! use dkls23::protocols::Parameters;
//!
//! // Party 1 initializes
//! let mut party1 = DkgPartyState::new(
//!     &Parameters { threshold: 2, share_count: 3 },
//!     1,
//!     b"session_id"
//! );
//!
//! // Phase 1: Generate messages to send
//! let phase1_msgs = party1.start_phase1().unwrap();
//! // Send phase1_msgs.to_send to other parties
//!
//! // After receiving messages from all parties, process them
//! let received: Vec<Phase1Message> = vec![]; // Received from other parties
//! party1.process_phase1_messages(&received).unwrap();
//!
//! // Continue with Phase 2...
//! ```

use std::collections::BTreeMap;

use k256::Scalar;
use serde::{Deserialize, Serialize};

use crate::protocols::dkg::*;
use crate::protocols::{Abort, Parameters, Party};

/// State machine for a single party participating in distributed DKG.
#[derive(Clone)]
pub struct DkgPartyState {
    /// Session data for this party
    pub session_data: SessionData,
    /// Current phase (1-4)
    pub phase: u8,
    /// Phase 1 output: polynomial fragments to send
    phase1_output: Option<Vec<Scalar>>,
    /// Phase 2 data to keep
    phase2_keep: Option<Phase2KeepData>,
    /// Phase 2 output: messages to send
    phase2_output: Option<Phase2Output>,
    /// Phase 3 data to keep
    phase3_keep: Option<Phase3KeepData>,
    /// Phase 3 output: messages to send
    phase3_output: Option<Phase3Output>,
}

/// Data kept between Phase 2 and Phase 3
#[derive(Clone, Debug)]
struct Phase2KeepData {
    poly_point: Scalar,
    proof_commitment: ProofCommitment,
    zero_kept: BTreeMap<u8, KeepInitZeroSharePhase2to3>,
    bip_kept: UniqueKeepDerivationPhase2to3,
}

/// Data kept between Phase 3 and Phase 4
#[derive(Clone, Debug)]
struct Phase3KeepData {
    poly_point: Scalar,
    proof_commitment: ProofCommitment,
    zero_kept: BTreeMap<u8, KeepInitZeroSharePhase3to4>,
    mul_kept: BTreeMap<u8, KeepInitMulPhase3to4>,
}

/// Messages sent in Phase 1 (polynomial fragments)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phase1Message {
    pub sender: u8,
    pub receiver: u8,
    pub fragment: Scalar,
}

/// Messages sent in Phase 2
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phase2Messages {
    /// Proof commitment (broadcast to all)
    pub proof_commitment: ProofCommitment,
    /// Zero share commitments (one per other party)
    pub zero_commitments: Vec<TransmitInitZeroSharePhase2to4>,
    /// BIP-32 chain code commitment (broadcast to all)
    pub bip_commitment: BroadcastDerivationPhase2to4,
}

/// Messages sent in Phase 3
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phase3Messages {
    /// Zero share seeds (one per other party)
    pub zero_seeds: Vec<TransmitInitZeroSharePhase3to4>,
    /// Multiplication initialization (one per other party)
    pub mul_init: Vec<TransmitInitMulPhase3to4>,
    /// BIP-32 chain code reveal (broadcast to all)
    pub bip_reveal: BroadcastDerivationPhase3to4,
}

/// Output from Phase 2
#[derive(Clone, Debug)]
struct Phase2Output {
    messages: Phase2Messages,
}

/// Output from Phase 3
#[derive(Clone, Debug)]
struct Phase3Output {
    messages: Phase3Messages,
}

impl DkgPartyState {
    /// Create a new DKG party state for distributed execution.
    ///
    /// # Arguments
    ///
    /// * `parameters` - DKG parameters (threshold and share_count)
    /// * `party_index` - This party's index (1-based)
    /// * `session_id` - Session identifier (must be same for all parties)
    pub fn new(parameters: &Parameters, party_index: u8, session_id: &[u8]) -> Self {
        Self {
            session_data: SessionData {
                parameters: parameters.clone(),
                party_index,
                session_id: session_id.to_vec(),
            },
            phase: 0,
            phase1_output: None,
            phase2_keep: None,
            phase2_output: None,
            phase3_keep: None,
            phase3_output: None,
        }
    }

    /// Start Phase 1: Generate polynomial fragments to send to all parties.
    ///
    /// Returns messages that should be sent to other parties.
    /// Each party should send the fragment corresponding to their index.
    ///
    /// # Errors
    ///
    /// Returns error if called out of order (not in phase 0).
    pub fn start_phase1(&mut self) -> Result<Vec<Phase1Message>, Abort> {
        if self.phase != 0 {
            return Err(Abort::new(
                self.session_data.party_index,
                "start_phase1 called out of order",
            ));
        }

        let fragments = phase1(&self.session_data);
        self.phase1_output = Some(fragments.clone());
        self.phase = 1;

        // Create messages to send to each party
        let mut messages = Vec::with_capacity(self.session_data.parameters.share_count as usize);
        for (idx, fragment) in fragments.iter().enumerate() {
            let receiver = (idx + 1) as u8;
            messages.push(Phase1Message {
                sender: self.session_data.party_index,
                receiver,
                fragment: *fragment,
            });
        }

        Ok(messages)
    }

    /// Process Phase 1 messages received from other parties.
    ///
    /// # Arguments
    ///
    /// * `received` - Messages received from other parties (should include one from each party)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Called out of order (not in phase 1)
    /// - Not enough messages received
    /// - Messages are not for this party
    pub fn process_phase1_messages(&mut self, received: &[Phase1Message]) -> Result<(), Abort> {
        if self.phase != 1 {
            return Err(Abort::new(
                self.session_data.party_index,
                "process_phase1_messages called out of order",
            ));
        }

        if received.len() != self.session_data.parameters.share_count as usize {
            return Err(Abort::new(
                self.session_data.party_index,
                &format!(
                    "Expected {} messages, got {}",
                    self.session_data.parameters.share_count,
                    received.len()
                ),
            ));
        }

        // Verify all messages are for this party
        for msg in received {
            if msg.receiver != self.session_data.party_index {
                return Err(Abort::new(
                    self.session_data.party_index,
                    &format!("Received message not for this party (receiver: {})", msg.receiver),
                ));
            }
        }

        // Extract fragments in order (by sender index)
        let mut poly_fragments = Vec::with_capacity(self.session_data.parameters.share_count as usize);
        for i in 1..=self.session_data.parameters.share_count {
            let msg = received
                .iter()
                .find(|m| m.sender == i)
                .ok_or_else(|| {
                    Abort::new(
                        self.session_data.party_index,
                        &format!("Missing message from party {}", i),
                    )
                })?;
            poly_fragments.push(msg.fragment);
        }

        // Store for Phase 2
        self.phase1_output = Some(poly_fragments);
        self.phase = 2;

        Ok(())
    }

    /// Start Phase 2: Generate proofs, commitments, and initialization data.
    ///
    /// Must be called after `process_phase1_messages()`.
    ///
    /// Returns messages that should be sent to other parties (broadcasts and point-to-point).
    ///
    /// # Errors
    ///
    /// Returns error if called out of order (not in phase 2 after processing Phase 1).
    pub fn start_phase2(&mut self) -> Result<Phase2Messages, Abort> {
        if self.phase != 2 {
            return Err(Abort::new(
                self.session_data.party_index,
                "start_phase2 called out of order",
            ));
        }

        let poly_fragments = self
            .phase1_output
            .as_ref()
            .ok_or_else(|| {
                Abort::new(
                    self.session_data.party_index,
                    "Phase 1 output not available",
                )
            })?
            .clone();

        let (poly_point, proof_commitment, zero_kept, zero_transmit, bip_kept, bip_broadcast) =
            phase2(&self.session_data, &poly_fragments);

        // Store data for Phase 3
        self.phase2_keep = Some(Phase2KeepData {
            poly_point,
            proof_commitment: proof_commitment.clone(),
            zero_kept,
            bip_kept,
        });

        self.phase2_output = Some(Phase2Output {
            messages: Phase2Messages {
                proof_commitment,
                zero_commitments: zero_transmit,
                bip_commitment: bip_broadcast,
            },
        });

        self.phase = 3;

        Ok(self.phase2_output.as_ref().unwrap().messages.clone())
    }

    /// Process Phase 2 messages received from other parties.
    ///
    /// # Arguments
    ///
    /// * `_received` - Messages received from all parties (should include one from each party)
    ///
    /// # Errors
    ///
    /// Returns error if called out of order or if messages are invalid.
    pub fn process_phase2_messages(&mut self, _received: &[Phase2Messages]) -> Result<(), Abort> {
        if self.phase != 3 {
            return Err(Abort::new(
                self.session_data.party_index,
                "process_phase2_messages called out of order",
            ));
        }

        if _received.len() != self.session_data.parameters.share_count as usize {
            return Err(Abort::new(
                self.session_data.party_index,
                &format!(
                    "Expected {} messages, got {}",
                    self.session_data.parameters.share_count,
                    _received.len()
                ),
            ));
        }

        // Messages are processed in Phase 3, so we just mark that we've received them
        // The actual processing happens when we start Phase 3
        Ok(())
    }

    /// Start Phase 3: Continue initialization (zero shares, multiplication, BIP-32).
    ///
    /// Must be called after `process_phase2_messages()`.
    ///
    /// Returns messages that should be sent to other parties.
    ///
    /// # Errors
    ///
    /// Returns error if called out of order.
    pub fn start_phase3(&mut self) -> Result<Phase3Messages, Abort> {
        if self.phase != 3 {
            return Err(Abort::new(
                self.session_data.party_index,
                "start_phase3 called out of order",
            ));
        }

        let phase2_keep = self
            .phase2_keep
            .as_ref()
            .ok_or_else(|| {
                Abort::new(
                    self.session_data.party_index,
                    "Phase 2 keep data not available",
                )
            })?
            .clone();

        // Extract zero_kept and bip_kept from our stored data
        let zero_kept = phase2_keep.zero_kept;
        let bip_kept = phase2_keep.bip_kept;

        let (zero_keep, zero_transmit, mul_keep, mul_transmit, bip_broadcast) =
            phase3(&self.session_data, &zero_kept, &bip_kept);

        // Store data for Phase 4
        self.phase3_keep = Some(Phase3KeepData {
            poly_point: phase2_keep.poly_point,
            proof_commitment: phase2_keep.proof_commitment,
            zero_kept: zero_keep,
            mul_kept: mul_keep,
        });

        self.phase3_output = Some(Phase3Output {
            messages: Phase3Messages {
                zero_seeds: zero_transmit,
                mul_init: mul_transmit,
                bip_reveal: bip_broadcast,
            },
        });

        self.phase = 4;

        Ok(self.phase3_output.as_ref().unwrap().messages.clone())
    }

    /// Process Phase 3 messages received from other parties.
    ///
    /// # Arguments
    ///
    /// * `received` - Messages received from all parties (should include one from each party)
    ///
    /// # Errors
    ///
    /// Returns error if called out of order or if messages are invalid.
    pub fn process_phase3_messages(&mut self, received: &[Phase3Messages]) -> Result<(), Abort> {
        if self.phase != 4 {
            return Err(Abort::new(
                self.session_data.party_index,
                "process_phase3_messages called out of order",
            ));
        }

        if received.len() != self.session_data.parameters.share_count as usize {
            return Err(Abort::new(
                self.session_data.party_index,
                &format!(
                    "Expected {} messages, got {}",
                    self.session_data.parameters.share_count,
                    received.len()
                ),
            ));
        }

        // Messages are processed in Phase 4, so we just mark that we've received them
        Ok(())
    }

    /// Complete Phase 4: Finalize DKG and create Party instance.
    ///
    /// Must be called after `process_phase3_messages()`.
    ///
    /// # Arguments
    ///
    /// * `phase2_messages` - All Phase 2 messages from all parties
    /// * `phase3_messages` - All Phase 3 messages from all parties
    ///
    /// Returns the completed `Party` instance ready for signing.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Called out of order
    /// - Any verification fails
    /// - Public key is trivial
    pub fn complete_phase4(
        &mut self,
        phase2_messages: &[Phase2Messages],
        phase3_messages: &[Phase3Messages],
    ) -> Result<Party, Abort> {
        if self.phase != 4 {
            return Err(Abort::new(
                self.session_data.party_index,
                "complete_phase4 called out of order",
            ));
        }

        let phase3_keep = self
            .phase3_keep
            .as_ref()
            .ok_or_else(|| {
                Abort::new(
                    self.session_data.party_index,
                    "Phase 3 keep data not available",
                )
            })?
            .clone();

        // Collect all proof commitments
        let mut proofs_commitments: Vec<ProofCommitment> = Vec::with_capacity(
            self.session_data.parameters.share_count as usize,
        );
        for msg in phase2_messages {
            proofs_commitments.push(msg.proof_commitment.clone());
        }

        // Collect zero share messages
        let mut zero_received_2to4: Vec<TransmitInitZeroSharePhase2to4> = Vec::new();
        let mut zero_received_3to4: Vec<TransmitInitZeroSharePhase3to4> = Vec::new();
        for msg in phase2_messages {
            for zero_commit in &msg.zero_commitments {
                if zero_commit.parties.receiver == self.session_data.party_index {
                    zero_received_2to4.push(zero_commit.clone());
                }
            }
        }
        for msg in phase3_messages {
            for zero_seed in &msg.zero_seeds {
                if zero_seed.parties.receiver == self.session_data.party_index {
                    zero_received_3to4.push(zero_seed.clone());
                }
            }
        }

        // Collect multiplication messages
        let mut mul_received: Vec<TransmitInitMulPhase3to4> = Vec::new();
        for msg in phase3_messages {
            for mul_msg in &msg.mul_init {
                if mul_msg.parties.receiver == self.session_data.party_index {
                    mul_received.push(mul_msg.clone());
                }
            }
        }

        // Collect BIP-32 messages
        let mut bip_received_2to4: BTreeMap<u8, BroadcastDerivationPhase2to4> = BTreeMap::new();
        let mut bip_received_3to4: BTreeMap<u8, BroadcastDerivationPhase3to4> = BTreeMap::new();
        for msg in phase2_messages {
            bip_received_2to4.insert(msg.bip_commitment.sender_index, msg.bip_commitment.clone());
        }
        for msg in phase3_messages {
            bip_received_3to4.insert(msg.bip_reveal.sender_index, msg.bip_reveal.clone());
        }

        // Call phase4
        let party = phase4(
            &self.session_data,
            &phase3_keep.poly_point,
            &proofs_commitments,
            &phase3_keep.zero_kept,
            &zero_received_2to4,
            &zero_received_3to4,
            &phase3_keep.mul_kept,
            &mul_received,
            &bip_received_2to4,
            &bip_received_3to4,
        )?;

        self.phase = 5; // Completed

        Ok(party)
    }

    /// Get the current phase number (0-5, where 5 means completed).
    pub fn current_phase(&self) -> u8 {
        self.phase
    }

    /// Get this party's index.
    pub fn party_index(&self) -> u8 {
        self.session_data.party_index
    }
}

