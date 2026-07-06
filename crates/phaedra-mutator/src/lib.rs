//! Coverage-guided mutation engine with 18 strategies and adaptive weight scheduling.
//!
//! `MutationEngine` selects a `Strategy` variant each call via `WeightedIndex`; the distribution is
//! rebuilt lazily when weights change (cached to avoid per-call allocation). `record_success` increases
//! the weight of the strategy that produced a corpus-worthy input; a decay pass every 500 mutations
//! nudges all weights back toward 1.0 so no strategy is permanently suppressed. The 18 strategies span
//! bit-level ops (BitFlip, ByteSubstitute, Arithmetic), block ops (BlockFlip, BlockInsert, BlockDelete,
//! ByteRepeat, BlockSubstitute), structured ops (InterestingByte, InterestingU16, InterestingU32,
//! Splice), and higher-level ops (TokenInsert, TokenReplace, Recombine, Havoc, RepeatByte, ZeroRange);
//! a token dictionary populated from the target description is used by TokenInsert and TokenReplace.

pub mod engine;
pub mod strategies;

pub use engine::{MutationEngine, Strategy};
