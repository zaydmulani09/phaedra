//! SQLite-backed corpus manager with priority-weighted seed selection.
//!
//! `CorpusManager` opens (or creates) a WAL-mode SQLite database and exposes `add_seed`, `pick`,
//! `record_pick`, and `record_hit`. Seeds are deduplicated by FNV-1a 64-bit fingerprint stored as a
//! 16-char hex string -- no sha2 dependency. `pick` performs reservoir sampling over the top-50 seeds
//! ranked by `(hit_count * 10 + edge_count) / (pick_count + 1)`, a formula that favors seeds that
//! consistently open new coverage over seeds that are picked frequently but contribute nothing.

pub(crate) mod db;
mod manager;

pub use db::fingerprint;
pub use manager::{CorpusManager, CorpusSeed};
