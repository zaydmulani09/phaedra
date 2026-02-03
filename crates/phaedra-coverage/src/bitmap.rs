pub const MAP_SIZE: usize = 1 << 16;

#[derive(Clone, Debug)]
pub struct CoverageMap {
    pub data: Box<[u8; MAP_SIZE]>,
}

impl CoverageMap {
    pub fn new() -> Self {
        Self {
            data: Box::new([0u8; MAP_SIZE]),
        }
    }

    pub fn clear(&mut self) {
        self.data.as_mut().fill(0);
    }

    pub fn edge_count(&self) -> usize {
        self.data.iter().filter(|&&b| b != 0).count()
    }

    pub fn has_coverage(&self) -> bool {
        self.data.iter().any(|&b| b != 0)
    }
}

impl Default for CoverageMap {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CoverageTracker {
    seen: Box<[u8; MAP_SIZE]>,
    total_edges: usize,
}

impl CoverageTracker {
    pub fn new() -> Self {
        Self {
            seen: Box::new([0u8; MAP_SIZE]),
            total_edges: 0,
        }
    }

    pub fn is_interesting(&mut self, map: &CoverageMap) -> bool {
        if !map.has_coverage() {
            return false;
        }
        let mut found_new = false;
        for (i, &byte) in map.data.iter().enumerate() {
            if byte != 0 && self.seen[i] == 0 {
                self.seen[i] = byte;
                self.total_edges += 1;
                found_new = true;
            }
        }
        found_new
    }

    pub fn total_edges(&self) -> usize {
        self.total_edges
    }

    pub fn reset(&mut self) {
        self.seen.as_mut().fill(0);
        self.total_edges = 0;
    }
}

impl Default for CoverageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_map_has_no_coverage() {
        let map = CoverageMap::new();
        assert!(!map.has_coverage());
        assert_eq!(map.edge_count(), 0);
    }

    #[test]
    fn test_map_clear() {
        let mut map = CoverageMap::new();
        map.data[42] = 1;
        assert!(map.has_coverage());
        map.clear();
        assert!(!map.has_coverage());
    }

    #[test]
    fn test_tracker_novelty_detection() {
        let mut tracker = CoverageTracker::new();

        let mut map1 = CoverageMap::new();
        map1.data[0] = 1;
        map1.data[100] = 1;
        assert!(tracker.is_interesting(&map1));
        assert_eq!(tracker.total_edges(), 2);

        assert!(!tracker.is_interesting(&map1));
        assert_eq!(tracker.total_edges(), 2);

        let mut map2 = CoverageMap::new();
        map2.data[200] = 1;
        assert!(tracker.is_interesting(&map2));
        assert_eq!(tracker.total_edges(), 3);
    }

    #[test]
    fn test_tracker_reset() {
        let mut tracker = CoverageTracker::new();
        let mut map = CoverageMap::new();
        map.data[0] = 1;
        tracker.is_interesting(&map);
        assert_eq!(tracker.total_edges(), 1);
        tracker.reset();
        assert_eq!(tracker.total_edges(), 0);
        assert!(tracker.is_interesting(&map));
    }
}
