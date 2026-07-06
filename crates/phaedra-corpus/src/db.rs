use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct SeedRow {
    pub id: i64,
    pub data: Vec<u8>,
    pub hash: String,
    pub pick_count: i64,
    pub hit_count: i64,
    pub edge_count: i64,
    pub added_at: i64,
    pub origin: String,
}

pub fn fingerprint(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

pub(crate) fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS corpus (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            data        BLOB    NOT NULL,
            hash        TEXT    NOT NULL UNIQUE,
            pick_count  INTEGER NOT NULL DEFAULT 0,
            hit_count   INTEGER NOT NULL DEFAULT 0,
            edge_count  INTEGER NOT NULL DEFAULT 0,
            added_at    INTEGER NOT NULL,
            origin      TEXT    NOT NULL DEFAULT 'bootstrap'
        );",
    )?;
    Ok(conn)
}

pub(crate) fn insert_seed(
    conn: &Connection,
    data: &[u8],
    hash: &str,
    edge_count: usize,
    origin: &str,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT INTO corpus (data, hash, edge_count, added_at, origin) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![data, hash, edge_count as i64, now, origin],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn record_pick(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE corpus SET pick_count = pick_count + 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub(crate) fn record_hit(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE corpus SET hit_count = hit_count + 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub(crate) fn count(conn: &Connection) -> Result<usize> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM corpus", [], |row| row.get(0))?;
    Ok(n as usize)
}

pub(crate) fn all_by_priority(conn: &Connection) -> Result<Vec<SeedRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, data, hash, pick_count, hit_count, edge_count, added_at, origin
         FROM corpus
         ORDER BY CAST(hit_count * 10 + edge_count AS REAL) / (pick_count + 1) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SeedRow {
            id: row.get(0)?,
            data: row.get(1)?,
            hash: row.get(2)?,
            pick_count: row.get(3)?,
            hit_count: row.get(4)?,
            edge_count: row.get(5)?,
            added_at: row.get(6)?,
            origin: row.get(7)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub(crate) fn all_data_direct(conn: &Connection) -> Result<Vec<Vec<u8>>> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM corpus", [], |row| row.get(0))?;
    let mut result = Vec::with_capacity(count as usize);
    let mut stmt = conn.prepare(
        "SELECT data FROM corpus ORDER BY CAST(hit_count * 10 + edge_count AS REAL) / (pick_count + 1) DESC",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub(crate) fn sample_seed(
    conn: &Connection,
    rng: &mut impl rand::Rng,
) -> Result<Option<SeedRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, data, hash, pick_count, hit_count, edge_count, added_at, origin
         FROM corpus
         ORDER BY CAST(hit_count * 10 + edge_count AS REAL) / (pick_count + 1) DESC
         LIMIT 50",
    )?;
    let rows: Vec<SeedRow> = stmt
        .query_map([], |row| {
            Ok(SeedRow {
                id: row.get(0)?,
                data: row.get(1)?,
                hash: row.get(2)?,
                pick_count: row.get(3)?,
                hit_count: row.get(4)?,
                edge_count: row.get(5)?,
                added_at: row.get(6)?,
                origin: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        return Ok(None);
    }
    let idx = rng.gen_range(0..rows.len());
    Ok(Some(rows.into_iter().nth(idx).unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_empty() {
        let h = fingerprint(b"");
        assert_eq!(h.len(), 16);
    }
}
