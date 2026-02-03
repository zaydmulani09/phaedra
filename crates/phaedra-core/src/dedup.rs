/// A normalized crash signature used for deduplication.
/// Two crashes with the same signature are considered the same bug.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrashSignature {
    /// Normalized signature string — hex digest of the normalized key.
    pub key: String,
    /// Human-readable summary of what the signature was derived from.
    pub summary: String,
}

impl CrashSignature {
    /// Derive a crash signature from an ExecutionStatus and the crashing input.
    ///
    /// Normalization strategy:
    /// - Crash { signal: Some(n) } → key based on signal number
    /// - Crash { signal: None }    → key based on FNV hash of first 32 bytes of input
    ///   (different inputs with no signal info get different signatures)
    /// - Timeout                   → fixed key "timeout"
    /// - Error(msg)                → key based on FNV of first 64 chars of msg
    /// - Ok                        → key "ok" (should never be triaged, but handle it)
    pub fn from_status(
        status: &phaedra_harness::ExecutionStatus,
        input: &[u8],
    ) -> Self {
        match status {
            phaedra_harness::ExecutionStatus::Crash { signal: Some(sig) } => {
                let key = format!("signal_{sig:02}");
                Self {
                    summary: format!("signal {sig}"),
                    key,
                }
            }
            phaedra_harness::ExecutionStatus::Crash { signal: None } => {
                let hash = fnv64(&input[..input.len().min(32)]);
                let key = format!("crash_{hash:016x}");
                Self {
                    summary: "crash (no signal)".into(),
                    key,
                }
            }
            phaedra_harness::ExecutionStatus::Timeout => Self {
                key: "timeout".into(),
                summary: "timeout".into(),
            },
            phaedra_harness::ExecutionStatus::Error(msg) => {
                let hash = fnv64(msg.as_bytes().get(..64.min(msg.len())).unwrap_or(msg.as_bytes()));
                let key = format!("error_{hash:016x}");
                Self {
                    summary: format!("error: {}", &msg[..msg.len().min(40)]),
                    key,
                }
            }
            phaedra_harness::ExecutionStatus::Ok => Self {
                key: "ok".into(),
                summary: "ok".into(),
            },
        }
    }
}

fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use phaedra_harness::ExecutionStatus;

    #[test]
    fn test_signal_signature_stable() {
        let a = CrashSignature::from_status(&ExecutionStatus::Crash { signal: Some(11) }, b"input");
        let b = CrashSignature::from_status(&ExecutionStatus::Crash { signal: Some(11) }, b"input");
        assert_eq!(a.key, b.key);
    }

    #[test]
    fn test_different_signals_different_keys() {
        let a = CrashSignature::from_status(&ExecutionStatus::Crash { signal: Some(11) }, b"x");
        let b = CrashSignature::from_status(&ExecutionStatus::Crash { signal: Some(6) }, b"x");
        assert_ne!(a.key, b.key);
    }

    #[test]
    fn test_timeout_fixed_key() {
        let sig = CrashSignature::from_status(&ExecutionStatus::Timeout, b"anything");
        assert_eq!(sig.key, "timeout");
    }

    #[test]
    fn test_no_signal_keyed_by_input() {
        let a = CrashSignature::from_status(&ExecutionStatus::Crash { signal: None }, &[1, 2, 3]);
        let b = CrashSignature::from_status(&ExecutionStatus::Crash { signal: None }, &[4, 5, 6]);
        assert_ne!(a.key, b.key);
    }

    #[test]
    fn test_same_input_same_key() {
        let input = &[1u8, 2, 3, 4, 5];
        let a = CrashSignature::from_status(&ExecutionStatus::Crash { signal: None }, input);
        let b = CrashSignature::from_status(&ExecutionStatus::Crash { signal: None }, input);
        assert_eq!(a.key, b.key);
    }
}
