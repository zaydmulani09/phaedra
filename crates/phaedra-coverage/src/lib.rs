pub mod bitmap;
pub mod shm;

pub use bitmap::{CoverageMap, CoverageTracker, MAP_SIZE};
pub use shm::SharedMemory;

pub const PHAEDRA_SHM_ENV: &str = "__PHAEDRA_SHM_ID";
