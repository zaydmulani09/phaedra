use anyhow::Result;
use rusqlite::Connection;

pub struct CostTracker {
    conn: Connection,
}

impl CostTracker {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS llm_calls (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                provider        TEXT    NOT NULL,
                model           TEXT    NOT NULL,
                input_tokens    INTEGER NOT NULL,
                output_tokens   INTEGER NOT NULL,
                cost_usd        REAL,
                purpose         TEXT    NOT NULL,
                ts_ms           INTEGER NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn record(&self, response: &crate::provider::LlmResponse, purpose: &str) -> Result<()> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let cost = response.estimated_cost_usd();
        let provider_str = format!("{:?}", response.provider).to_lowercase();
        self.conn.execute(
            "INSERT INTO llm_calls (provider, model, input_tokens, output_tokens, cost_usd, purpose, ts_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                provider_str,
                response.model,
                response.input_tokens,
                response.output_tokens,
                cost,
                purpose,
                ts,
            ],
        )?;
        Ok(())
    }

    pub fn total_cost_usd(&self) -> Result<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM llm_calls",
            [],
            |r| r.get(0),
        )?;
        Ok(total)
    }

    pub fn total_calls(&self) -> Result<u64> {
        let n: i64 =
            self.conn.query_row("SELECT COUNT(*) FROM llm_calls", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{LlmResponse, Provider};

    fn make_response(provider: Provider, model: &str, input: u32, output: u32) -> LlmResponse {
        LlmResponse {
            content: "test".into(),
            input_tokens: input,
            output_tokens: output,
            provider,
            model: model.into(),
        }
    }

    #[test]
    fn test_open_creates_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let tracker = CostTracker::open(&path).unwrap();
        assert_eq!(tracker.total_calls().unwrap(), 0);
    }

    #[test]
    fn test_record_and_total_cost() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let tracker = CostTracker::open(&path).unwrap();
        let r = make_response(Provider::OpenAI, "gpt-4o", 1000, 500);
        tracker.record(&r, "seed_gen").unwrap();
        let cost = tracker.total_cost_usd().unwrap();
        assert!(cost > 0.0);
    }

    #[test]
    fn test_total_calls_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let tracker = CostTracker::open(&path).unwrap();
        let r = make_response(Provider::Ollama, "llama3.2", 0, 0);
        tracker.record(&r, "seed_gen").unwrap();
        tracker.record(&r, "crash_analysis").unwrap();
        assert_eq!(tracker.total_calls().unwrap(), 2);
    }

    #[test]
    fn test_ollama_records_zero_cost() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let tracker = CostTracker::open(&path).unwrap();
        let r = make_response(Provider::Ollama, "llama3.2", 500, 200);
        tracker.record(&r, "seed_gen").unwrap();
        assert_eq!(tracker.total_cost_usd().unwrap(), 0.0);
    }

    #[test]
    fn test_empty_db_total_cost_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let tracker = CostTracker::open(&path).unwrap();
        assert_eq!(tracker.total_cost_usd().unwrap(), 0.0);
    }
}
