//! Coverage tracking via SanCov edge bitmaps and POSIX shared memory.
//!
//! `SharedMemory` allocates a named shared memory region (POSIX `shm_open` on Unix, `CreateFileMappingA`
//! on Windows) and sets the `__PHAEDRA_SHM_ID` environment variable so child processes compiled with the
//! SanCov shim (`sancov_rt.c`) write their edge counters directly into the mapping. `CoverageMap` wraps
//! the 65536-byte bitmap; `CoverageTracker` compares successive maps against a running union and returns
//! `true` when a new edge bit is set, driving corpus admission decisions.

pub mod bitmap;
pub mod shm;

pub use bitmap::{CoverageMap, CoverageTracker, MAP_SIZE};
pub use shm::SharedMemory;

pub const PHAEDRA_SHM_ENV: &str = "__PHAEDRA_SHM_ID";
