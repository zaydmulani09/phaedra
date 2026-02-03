use crate::dedup::CrashSignature;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

/// A triaged crash record.
#[derive(Debug, Clone)]
pub struct CrashRecord {
    pub id: i64,
    pub signature_key: String,
    pub signature_summary: String,
    pub severity: Severity,
    pub input: Vec<u8>,
    pub input_hex: String,
    pub status_str: String,
    pub first_seen_ms: u64,
    pub hit_count: u64,
}

/// Crash severity classification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    /// Signal 11 (SIGSEGV) or signal 6 (SIGABRT) — likely memory corruption
    Critical,
    /// Signal 4 (SIGILL) or signal 8 (SIGFPE) — illegal instruction or arithmetic
    High,
    /// Any other non-zero signal
    Medium,
    /// No signal — process exited non-zero or error
    Low,
    /// Timeout
    Timeout,
}

impl Severity {
    pub fn from_status(status: &phaedra_harness::ExecutionStatus) -> Self {
        match status {
            phaedra_harness::ExecutionStatus::Crash { signal: Some(11) } => Severity::Critical,
            phaedra_harness::ExecutionStatus::Crash { signal: Some(6) }  => Severity::Critical,
            phaedra_harness::ExecutionStatus::Crash { signal: Some(4) }  => Severity::High,
            phaedra_harness::ExecutionStatus::Crash { signal: Some(8) }  => Severity::High,
            phaedra_harness::ExecutionStatus::Crash { signal: Some(_) }  => Severity::Medium,
            phaedra_harness::ExecutionStatus::Crash { signal: None }     => Severity::Low,
            phaedra_harness::ExecutionStatus::Timeout                    => Severity::Timeout,
            phaedra_harness::ExecutionStatus::Error(_)                   => Severity::Low,
            phaedra_harness::ExecutionStatus::Ok                         => Severity::Low,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High     => "HIGH",
            Severity::Medium   => "MEDIUM",
            Severity::Low      => "LOW",
            Severity::Timeout  => "TIMEOUT",
        }
    }
}

/// Manages the crash triage database.
pub struct CrashTriageDb {
    conn: Connection,
}

impl CrashTriageDb {
    /// Open or create the crash triage database at the given path.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS crashes (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                signature_key     TEXT    NOT NULL UNIQUE,
                signature_summary TEXT    NOT NULL,
                severity          TEXT    NOT NULL,
                input             BLOB    NOT NULL,
                input_hex         TEXT    NOT NULL,
                status_str        TEXT    NOT NULL,
                first_seen_ms     INTEGER NOT NULL,
                hit_count         INTEGER NOT NULL DEFAULT 1
            );
        ")?;
        Ok(Self { conn })
    }

    /// Record a crash. If the signature already exists, increment hit_count and return false.
    /// If it is new, insert it and return true.
    pub fn record(
        &mut self,
        sig: &CrashSignature,
        severity: &Severity,
        input: &[u8],
        status_str: &str,
    ) -> Result<bool> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let input_hex = hex_encode(input);

        let existing: Option<i64> = self.conn.query_row(
            "SELECT id FROM crashes WHERE signature_key = ?1",
            rusqlite::params![sig.key],
            |row| row.get(0),
        ).optional()?;

        if existing.is_some() {
            self.conn.execute(
                "UPDATE crashes SET hit_count = hit_count + 1 WHERE signature_key = ?1",
                rusqlite::params![sig.key],
            )?;
            return Ok(false);
        }

        self.conn.execute(
            "INSERT INTO crashes
             (signature_key, signature_summary, severity, input, input_hex, status_str, first_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                sig.key,
                sig.summary,
                severity.as_str(),
                input,
                input_hex,
                status_str,
                now_ms as i64,
            ],
        )?;
        Ok(true)
    }

    /// Return all crash records ordered by severity then first_seen_ms.
    pub fn all(&self) -> Result<Vec<CrashRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, signature_key, signature_summary, severity, input, input_hex,
                    status_str, first_seen_ms, hit_count
             FROM crashes
             ORDER BY CASE severity
                 WHEN 'CRITICAL' THEN 0
                 WHEN 'HIGH'     THEN 1
                 WHEN 'MEDIUM'   THEN 2
                 WHEN 'LOW'      THEN 3
                 ELSE 4 END, first_seen_ms ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CrashRecord {
                id:                row.get(0)?,
                signature_key:     row.get(1)?,
                signature_summary: row.get(2)?,
                severity:          severity_from_str(&row.get::<_, String>(3)?),
                input:             row.get(4)?,
                input_hex:         row.get(5)?,
                status_str:        row.get(6)?,
                first_seen_ms:     row.get::<_, i64>(7)? as u64,
                hit_count:         row.get::<_, i64>(8)? as u64,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// Retrieve a single crash record by its primary-key id.
    pub fn get_by_id(&self, id: i64) -> Result<Option<CrashRecord>> {
        let result = self.conn.query_row(
            "SELECT id, signature_key, signature_summary, severity, input, input_hex,
                    status_str, first_seen_ms, hit_count
             FROM crashes WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok(CrashRecord {
                id:                row.get(0)?,
                signature_key:     row.get(1)?,
                signature_summary: row.get(2)?,
                severity:          severity_from_str(&row.get::<_, String>(3)?),
                input:             row.get(4)?,
                input_hex:         row.get(5)?,
                status_str:        row.get(6)?,
                first_seen_ms:     row.get::<_, i64>(7)? as u64,
                hit_count:         row.get::<_, i64>(8)? as u64,
            }),
        ).optional()?;
        Ok(result)
    }

    /// Total unique crash signatures.
    pub fn unique_count(&self) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM crashes",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }
}

fn severity_from_str(s: &str) -> Severity {
    match s {
        "CRITICAL" => Severity::Critical,
        "HIGH"     => Severity::High,
        "MEDIUM"   => Severity::Medium,
        "TIMEOUT"  => Severity::Timeout,
        _          => Severity::Low,
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use phaedra_harness::ExecutionStatus;

    fn make_sig(key: &str, summary: &str) -> CrashSignature {
        CrashSignature { key: key.to_string(), summary: summary.to_string() }
    }

    #[test]
    fn test_open_creates_table() {
        let dir = tempfile::tempdir().unwrap();
        let db = CrashTriageDb::open(&dir.path().join("crashes.db"));
        assert!(db.is_ok());
    }

    #[test]
    fn test_new_crash_returns_true() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        let sig = make_sig("signal_11", "signal 11");
        let is_new = db.record(&sig, &Severity::Critical, b"input", "Crash { signal: Some(11) }").unwrap();
        assert!(is_new);
    }

    #[test]
    fn test_duplicate_returns_false_and_increments_hit_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        let sig = make_sig("signal_11", "signal 11");
        db.record(&sig, &Severity::Critical, b"input", "Crash").unwrap();
        let is_new = db.record(&sig, &Severity::Critical, b"input2", "Crash").unwrap();
        assert!(!is_new);
        let records = db.all().unwrap();
        assert_eq!(records[0].hit_count, 2);
    }

    #[test]
    fn test_severity_critical_for_sigsegv() {
        let sev = Severity::from_status(&ExecutionStatus::Crash { signal: Some(11) });
        assert_eq!(sev, Severity::Critical);
    }

    #[test]
    fn test_severity_ordering_in_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        db.record(&make_sig("low_key", "low"), &Severity::Low, b"a", "err").unwrap();
        db.record(&make_sig("crit_key", "crit"), &Severity::Critical, b"b", "crash").unwrap();
        let records = db.all().unwrap();
        assert_eq!(records[0].severity, Severity::Critical);
        assert_eq!(records[1].severity, Severity::Low);
    }

    #[test]
    fn test_get_by_id_existing() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        let sig = make_sig("findme_key", "findme summary");
        db.record(&sig, &Severity::High, b"crash_input", "Crash { signal: Some(4) }").unwrap();
        let all = db.all().unwrap();
        let inserted_id = all[0].id;
        let record = db.get_by_id(inserted_id).unwrap();
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.signature_key, "findme_key");
        assert_eq!(r.input, b"crash_input");
        assert_eq!(r.severity, Severity::High);
    }

    #[test]
    fn test_get_by_id_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        let result = db.get_by_id(9999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_unique_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = CrashTriageDb::open(&dir.path().join("crashes.db")).unwrap();
        assert_eq!(db.unique_count().unwrap(), 0);
        db.record(&make_sig("k1", "s1"), &Severity::Low, b"a", "e").unwrap();
        db.record(&make_sig("k2", "s2"), &Severity::High, b"b", "e").unwrap();
        db.record(&make_sig("k1", "s1"), &Severity::Low, b"c", "e").unwrap(); // dup
        assert_eq!(db.unique_count().unwrap(), 2);
    }
}
