use std::sync::{Arc, Mutex};

/// Thread-safe handle shared between fuzzing loop and TUI task.
pub type SharedStats = Arc<Mutex<LiveStats>>;

/// Extended live stats including per-strategy data for the TUI.
#[derive(Debug, Clone, Default)]
pub struct LiveStats {
    pub total_execs: u64,
    pub interesting_execs: u64,
    pub total_crashes: u64,
    pub unique_crashes: u64,
    pub total_timeouts: u64,
    pub corpus_size: usize,
    pub unique_edges: usize,
    pub execs_per_sec: f64,
    pub elapsed_secs: u64,
    /// Strategy names and their current weights, in Strategy::all() order.
    pub strategy_weights: Vec<(String, f64)>,
    /// Last 5 interesting inputs found (label for display).
    pub recent_finds: Vec<String>,
    /// Last 5 crashes found (severity string).
    pub recent_crashes: Vec<String>,
}

pub fn new_shared_stats() -> SharedStats {
    Arc::new(Mutex::new(LiveStats::default()))
}

/// Live campaign statistics, updated after every execution.
#[derive(Debug, Clone)]
pub struct CampaignStats {
    pub total_execs: u64,
    pub interesting_execs: u64,
    pub total_crashes: u64,
    pub total_timeouts: u64,
    pub corpus_size: usize,
    pub unique_edges: usize,
    pub execs_per_sec: f64,
    pub started_at: std::time::Instant,
}

impl Default for CampaignStats {
    fn default() -> Self {
        Self {
            total_execs: 0,
            interesting_execs: 0,
            total_crashes: 0,
            total_timeouts: 0,
            corpus_size: 0,
            unique_edges: 0,
            execs_per_sec: 0.0,
            started_at: std::time::Instant::now(),
        }
    }
}

impl CampaignStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub fn elapsed_hms(&self) -> String {
        let secs = self.elapsed().as_secs();
        format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_elapsed_hms_format() {
        let stats = CampaignStats::new();
        let hms = stats.elapsed_hms();
        let parts: Vec<&str> = hms.split(':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 2);
        assert_eq!(parts[1].len(), 2);
        assert_eq!(parts[2].len(), 2);
    }

    #[test]
    fn test_stats_default_zeroed() {
        let stats = CampaignStats::new();
        assert_eq!(stats.total_execs, 0);
        assert_eq!(stats.total_crashes, 0);
        assert_eq!(stats.corpus_size, 0);
        assert_eq!(stats.unique_edges, 0);
    }
}
