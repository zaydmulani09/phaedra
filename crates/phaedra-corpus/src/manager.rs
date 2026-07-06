use crate::db;
use anyhow::Result;
use rand::rngs::SmallRng;
use rand::SeedableRng;

#[derive(Debug, Clone)]
pub struct CorpusSeed {
    pub id: i64,
    pub data: Vec<u8>,
    pub hash: String,
    pub pick_count: i64,
    pub hit_count: i64,
    pub edge_count: i64,
    pub added_at: i64,
    pub origin: String,
}

impl From<db::SeedRow> for CorpusSeed {
    fn from(r: db::SeedRow) -> Self {
        Self {
            id: r.id,
            data: r.data,
            hash: r.hash,
            pick_count: r.pick_count,
            hit_count: r.hit_count,
            edge_count: r.edge_count,
            added_at: r.added_at,
            origin: r.origin,
        }
    }
}

pub struct CorpusManager {
    conn: rusqlite::Connection,
    rng: SmallRng,
}

impl CorpusManager {
    pub fn open(db_path: &std::path::Path) -> Result<Self> {
        let conn = db::open(db_path)?;
        Ok(Self {
            conn,
            rng: SmallRng::from_entropy(),
        })
    }

    pub fn open_with_seed(db_path: &std::path::Path, seed: u64) -> Result<Self> {
        let conn = db::open(db_path)?;
        Ok(Self {
            conn,
            rng: SmallRng::seed_from_u64(seed),
        })
    }

    pub fn add_seed(&mut self, data: Vec<u8>, edge_count: usize, origin: &str) -> Result<bool> {
        let hash = db::fingerprint(&data);
        match db::insert_seed(&self.conn, &data, &hash, edge_count, origin) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint") {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn pick(&mut self) -> Result<Option<CorpusSeed>> {
        Ok(db::sample_seed(&self.conn, &mut self.rng)?.map(Into::into))
    }

    pub fn record_pick(&mut self, seed_id: i64) -> Result<()> {
        db::record_pick(&self.conn, seed_id)
    }

    pub fn record_hit(&mut self, seed_id: i64) -> Result<()> {
        db::record_hit(&self.conn, seed_id)
    }

    pub fn len(&self) -> Result<usize> {
        db::count(&self.conn)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    pub fn all_by_priority(&self) -> Result<Vec<CorpusSeed>> {
        Ok(db::all_by_priority(&self.conn)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn all_data(&self) -> Result<Vec<Vec<u8>>> {
        db::all_data_direct(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.db");
        (dir, path)
    }

    #[test]
    fn test_open_creates_db() {
        let (_dir, path) = temp_db();
        let mgr = CorpusManager::open(&path).unwrap();
        assert_eq!(mgr.len().unwrap(), 0);
    }

    #[test]
    fn test_add_and_count() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open(&path).unwrap();
        mgr.add_seed(b"aaa".to_vec(), 0, "test").unwrap();
        mgr.add_seed(b"bbb".to_vec(), 0, "test").unwrap();
        mgr.add_seed(b"ccc".to_vec(), 0, "test").unwrap();
        assert_eq!(mgr.len().unwrap(), 3);
    }

    #[test]
    fn test_duplicate_ignored() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open(&path).unwrap();
        let first = mgr.add_seed(b"hello".to_vec(), 0, "test").unwrap();
        let second = mgr.add_seed(b"hello".to_vec(), 0, "test").unwrap();
        assert!(first);
        assert!(!second);
        assert_eq!(mgr.len().unwrap(), 1);
    }

    #[test]
    fn test_pick_returns_seed() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open_with_seed(&path, 1).unwrap();
        mgr.add_seed(b"seed_data".to_vec(), 5, "test").unwrap();
        let picked = mgr.pick().unwrap();
        assert!(picked.is_some());
        assert_eq!(picked.unwrap().data, b"seed_data");
    }

    #[test]
    fn test_pick_empty_corpus() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open(&path).unwrap();
        assert!(mgr.pick().unwrap().is_none());
    }

    #[test]
    fn test_record_pick_increments() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open_with_seed(&path, 2).unwrap();
        mgr.add_seed(b"track_me".to_vec(), 0, "test").unwrap();
        let seed = mgr.pick().unwrap().unwrap();
        mgr.record_pick(seed.id).unwrap();
        let all = mgr.all_by_priority().unwrap();
        assert_eq!(all[0].pick_count, 1);
    }

    #[test]
    fn test_record_hit_increments() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open_with_seed(&path, 3).unwrap();
        mgr.add_seed(b"hit_me".to_vec(), 0, "test").unwrap();
        let seed = mgr.pick().unwrap().unwrap();
        mgr.record_hit(seed.id).unwrap();
        let all = mgr.all_by_priority().unwrap();
        assert_eq!(all[0].hit_count, 1);
    }

    #[test]
    fn test_priority_ordering() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open_with_seed(&path, 4).unwrap();
        mgr.add_seed(b"seed_a".to_vec(), 0, "test").unwrap();
        mgr.add_seed(b"seed_b".to_vec(), 0, "test").unwrap();
        mgr.add_seed(b"seed_c".to_vec(), 0, "test").unwrap();

        let all = mgr.all_by_priority().unwrap();
        let id_a = all.iter().find(|s| s.data == b"seed_a").unwrap().id;
        let id_b = all.iter().find(|s| s.data == b"seed_b").unwrap().id;
        let id_c = all.iter().find(|s| s.data == b"seed_c").unwrap().id;

        // B gets 5 hits, C gets 2, A gets 0
        for _ in 0..5 {
            mgr.record_hit(id_b).unwrap();
        }
        for _ in 0..2 {
            mgr.record_hit(id_c).unwrap();
        }

        let _ = id_a;
        let ordered = mgr.all_by_priority().unwrap();
        assert_eq!(ordered[0].id, id_b);
        assert_eq!(ordered[1].id, id_c);
    }

    #[test]
    fn test_all_data_returns_bytes() {
        let (_dir, path) = temp_db();
        let mut mgr = CorpusManager::open(&path).unwrap();
        mgr.add_seed(b"alpha".to_vec(), 0, "test").unwrap();
        mgr.add_seed(b"beta".to_vec(), 0, "test").unwrap();
        let data = mgr.all_data().unwrap();
        assert_eq!(data.len(), 2);
        assert!(data.contains(&b"alpha".to_vec()));
        assert!(data.contains(&b"beta".to_vec()));
    }

    #[test]
    fn test_fingerprint_stable() {
        let h1 = db::fingerprint(b"hello");
        let h2 = db::fingerprint(b"hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_fingerprint_differs() {
        let h1 = db::fingerprint(b"hello");
        let h2 = db::fingerprint(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_open_with_seed_deterministic() {
        let (_dir, path) = temp_db();
        {
            let mut mgr = CorpusManager::open_with_seed(&path, 42).unwrap();
            mgr.add_seed(b"only_seed".to_vec(), 0, "test").unwrap();
            let s = mgr.pick().unwrap().unwrap();
            assert_eq!(s.data, b"only_seed");
        }
        {
            let mut mgr = CorpusManager::open_with_seed(&path, 42).unwrap();
            let s = mgr.pick().unwrap().unwrap();
            assert_eq!(s.data, b"only_seed");
        }
    }
}
