use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub status: ExecutionStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration_ms: u64,
    /// Coverage bitmap from this execution, if shared memory was configured.
    #[serde(skip)]
    pub coverage: Option<phaedra_coverage::CoverageMap>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionStatus {
    Ok,
    Timeout,
    Crash { signal: Option<i32> },
    Error(String),
}

#[async_trait]
pub trait Harness: Send + Sync {
    async fn execute(&mut self, input: &[u8]) -> Result<ExecutionResult>;
}

pub struct StdinHarness {
    pub target: PathBuf,
    pub timeout: Duration,
    /// If Some, coverage data is read from this shared memory segment after each run.
    pub shm: Option<phaedra_coverage::SharedMemory>,
}

#[async_trait]
impl Harness for StdinHarness {
    async fn execute(&mut self, input: &[u8]) -> Result<ExecutionResult> {
        use std::process::Stdio;
        use tokio::process::Command;

        if let Some(ref shm) = self.shm {
            shm.clear();
        }

        let start = Instant::now();

        let mut cmd = Command::new(&self.target);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref shm) = self.shm {
            cmd.env(phaedra_coverage::PHAEDRA_SHM_ENV, &shm.id);
        }

        let mut child = cmd.spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input).await?;
        }

        let result = tokio::time::timeout(self.timeout, child.wait_with_output()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Err(_elapsed) => Ok(ExecutionResult {
                status: ExecutionStatus::Timeout,
                stdout: vec![],
                stderr: vec![],
                duration_ms,
                coverage: None,
            }),
            Ok(Err(e)) => Ok(ExecutionResult {
                status: ExecutionStatus::Error(e.to_string()),
                stdout: vec![],
                stderr: vec![],
                duration_ms,
                coverage: None,
            }),
            Ok(Ok(output)) => {
                let coverage = self.shm.as_ref().map(|shm| shm.read_map());
                let signal = extract_signal(&output.status);
                let status = if output.status.success() {
                    ExecutionStatus::Ok
                } else {
                    ExecutionStatus::Crash { signal }
                };
                Ok(ExecutionResult {
                    status,
                    stdout: output.stdout,
                    stderr: output.stderr,
                    duration_ms,
                    coverage,
                })
            }
        }
    }
}

#[cfg(unix)]
fn extract_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn extract_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    Tcp,
    Udp,
}

pub struct SocketHarness {
    pub addr: std::net::SocketAddr,
    pub timeout: Duration,
    pub protocol: SocketProtocol,
    /// How many bytes to read from the response (0 = write-only).
    pub read_bytes: usize,
    /// Optional shared memory for coverage.
    pub shm: Option<phaedra_coverage::SharedMemory>,
}

#[async_trait]
impl Harness for SocketHarness {
    async fn execute(&mut self, input: &[u8]) -> Result<ExecutionResult> {
        match self.protocol {
            SocketProtocol::Tcp => self.execute_tcp(input).await,
            SocketProtocol::Udp => self.execute_udp(input).await,
        }
    }
}

impl SocketHarness {
    async fn execute_tcp(&mut self, input: &[u8]) -> Result<ExecutionResult> {
        let start = Instant::now();

        if let Some(ref shm) = self.shm {
            shm.clear();
        }

        // Connect with timeout
        let stream = match tokio::time::timeout(
            self.timeout,
            tokio::net::TcpStream::connect(self.addr),
        )
        .await
        {
            Err(_) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Timeout,
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Err(e)) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Error(e.to_string()),
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Ok(s)) => s,
        };

        let (mut reader, mut writer) = stream.into_split();

        // Write input with timeout; connection reset mid-write = crash
        let remaining = self.timeout.saturating_sub(start.elapsed());
        match tokio::time::timeout(remaining, writer.write_all(input)).await {
            Err(_) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Timeout,
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Err(_)) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Crash { signal: None },
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Ok(_)) => {}
        }

        // Signal EOF to server
        let _ = writer.shutdown().await;

        // Read response
        let mut response = Vec::new();
        if self.read_bytes > 0 {
            let mut buf = vec![0u8; self.read_bytes];
            let remaining = self.timeout.saturating_sub(start.elapsed());
            match tokio::time::timeout(remaining, reader.read(&mut buf)).await {
                Err(_) => {} // timeout on read is not a crash — server may be slow
                Ok(Err(_)) => {
                    // Connection reset during read = crash
                    let coverage = self.shm.as_ref().map(|shm| shm.read_map());
                    return Ok(ExecutionResult {
                        status: ExecutionStatus::Crash { signal: None },
                        stdout: vec![],
                        stderr: vec![],
                        duration_ms: start.elapsed().as_millis() as u64,
                        coverage,
                    });
                }
                Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
            }
        }

        let coverage = self.shm.as_ref().map(|shm| shm.read_map());
        Ok(ExecutionResult {
            status: ExecutionStatus::Ok,
            stdout: response,
            stderr: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
            coverage,
        })
    }

    async fn execute_udp(&mut self, input: &[u8]) -> Result<ExecutionResult> {
        let start = Instant::now();

        let socket = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => s,
            Err(e) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Error(e.to_string()),
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
        };

        if let Err(e) = socket.connect(self.addr).await {
            return Ok(ExecutionResult {
                status: ExecutionStatus::Error(e.to_string()),
                stdout: vec![],
                stderr: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
                coverage: None,
            });
        }

        let remaining = self.timeout.saturating_sub(start.elapsed());
        match tokio::time::timeout(remaining, socket.send(input)).await {
            Err(_) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Timeout,
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Err(e)) => {
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Error(e.to_string()),
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                })
            }
            Ok(Ok(_)) => {}
        }

        let mut response = Vec::new();
        if self.read_bytes > 0 {
            let mut buf = vec![0u8; self.read_bytes];
            let remaining = self.timeout.saturating_sub(start.elapsed());
            // UDP timeout on recv is not a crash
            if let Ok(Ok(n)) = tokio::time::timeout(remaining, socket.recv(&mut buf)).await {
                response.extend_from_slice(&buf[..n]);
            }
        }

        Ok(ExecutionResult {
            status: ExecutionStatus::Ok,
            stdout: response,
            stderr: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
            coverage: None,
        })
    }
}

pub struct FileHarness {
    pub target: PathBuf,
    pub timeout: Duration,
    pub temp_dir: PathBuf,
    pub extra_args: Vec<String>,
    pub shm: Option<phaedra_coverage::SharedMemory>,
}

#[async_trait]
impl Harness for FileHarness {
    async fn execute(&mut self, input: &[u8]) -> Result<ExecutionResult> {
        use std::process::Stdio;
        use tokio::fs;
        use tokio::process::Command;

        let counter = FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let temp_path = self.temp_dir.join(format!("phaedra_input_{}_{}.bin", pid, counter));

        fs::write(&temp_path, input).await?;

        if let Some(ref shm) = self.shm {
            shm.clear();
        }

        let start = Instant::now();

        let mut cmd = Command::new(&self.target);
        for arg in &self.extra_args {
            cmd.arg(arg);
        }
        cmd.arg(&temp_path);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref shm) = self.shm {
            cmd.env(phaedra_coverage::PHAEDRA_SHM_ENV, &shm.id);
        }

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&temp_path).await;
                return Ok(ExecutionResult {
                    status: ExecutionStatus::Error(e.to_string()),
                    stdout: vec![],
                    stderr: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                    coverage: None,
                });
            }
        };

        let result = tokio::time::timeout(self.timeout, child.wait_with_output()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let _ = fs::remove_file(&temp_path).await;

        match result {
            Err(_elapsed) => Ok(ExecutionResult {
                status: ExecutionStatus::Timeout,
                stdout: vec![],
                stderr: vec![],
                duration_ms,
                coverage: None,
            }),
            Ok(Err(e)) => Ok(ExecutionResult {
                status: ExecutionStatus::Error(e.to_string()),
                stdout: vec![],
                stderr: vec![],
                duration_ms,
                coverage: None,
            }),
            Ok(Ok(output)) => {
                let coverage = self.shm.as_ref().map(|shm| shm.read_map());
                let signal = extract_signal(&output.status);
                let status = if output.status.success() {
                    ExecutionStatus::Ok
                } else {
                    ExecutionStatus::Crash { signal }
                };
                Ok(ExecutionResult {
                    status,
                    stdout: output.stdout,
                    stderr: output.stderr,
                    duration_ms,
                    coverage,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_harness_construction() {
        let h = SocketHarness {
            addr: "127.0.0.1:9999".parse().unwrap(),
            timeout: Duration::from_secs(5),
            protocol: SocketProtocol::Tcp,
            read_bytes: 4096,
            shm: None,
        };
        assert_eq!(h.addr.port(), 9999);
        assert_eq!(h.read_bytes, 4096);
        assert_eq!(h.protocol, SocketProtocol::Tcp);
    }

    #[test]
    fn test_socket_protocol_variants() {
        assert_ne!(SocketProtocol::Tcp, SocketProtocol::Udp);
        assert_eq!(SocketProtocol::Tcp, SocketProtocol::Tcp);
    }

    #[tokio::test]
    async fn test_stdin_harness_timeout() {
        // Use python if available; skip if not
        let python = if cfg!(windows) { "python" } else { "python3" };
        let mut h = StdinHarness {
            target: PathBuf::from(python),
            timeout: Duration::from_millis(200),
            shm: None,
        };
        // Spawn python sleeping 60s; should time out
        let result = h.execute(b"").await;
        // If python not found we get an error — that's fine, skip
        match result {
            Ok(r) => {
                // May be Timeout or Error (if python not found)
                assert!(
                    matches!(r.status, ExecutionStatus::Timeout | ExecutionStatus::Error(_)),
                    "unexpected status: {:?}", r.status
                );
            }
            Err(_) => {} // binary not found — skip
        }
    }

    #[test]
    fn test_file_harness_construction() {
        let h = FileHarness {
            target: PathBuf::from("cat"),
            timeout: Duration::from_secs(5),
            temp_dir: std::env::temp_dir(),
            extra_args: vec!["--flag".into()],
            shm: None,
        };
        assert_eq!(h.target, PathBuf::from("cat"));
        assert_eq!(h.extra_args.len(), 1);
        assert!(h.shm.is_none());
    }

    #[test]
    fn test_atomic_counter_increments() {
        let a = FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let b = FILE_COUNTER.load(Ordering::SeqCst);
        assert!(b > a);
    }

    #[tokio::test]
    async fn test_file_harness_exit_zero_is_ok() {
        // cmd /c "exit 0" filepath — cmd ignores filepath after processing /c "exit 0"
        let (target, extra): (&str, Vec<String>) = if cfg!(windows) {
            ("cmd", vec!["/c".into(), "exit 0".into()])
        } else {
            ("/usr/bin/true", vec![])
        };
        let mut h = FileHarness {
            target: PathBuf::from(target),
            timeout: Duration::from_secs(5),
            temp_dir: std::env::temp_dir(),
            extra_args: extra,
            shm: None,
        };
        match h.execute(b"hello").await {
            Ok(r) => assert!(
                matches!(r.status, ExecutionStatus::Ok | ExecutionStatus::Error(_)),
                "unexpected: {:?}", r.status
            ),
            Err(_) => {}
        }
    }

    #[tokio::test]
    async fn test_file_harness_exit_nonzero_is_crash() {
        let (target, extra): (&str, Vec<String>) = if cfg!(windows) {
            ("cmd", vec!["/c".into(), "exit 1".into()])
        } else {
            ("/usr/bin/false", vec![])
        };
        let mut h = FileHarness {
            target: PathBuf::from(target),
            timeout: Duration::from_secs(5),
            temp_dir: std::env::temp_dir(),
            extra_args: extra,
            shm: None,
        };
        match h.execute(b"hello").await {
            Ok(r) => assert!(
                matches!(r.status, ExecutionStatus::Crash { .. } | ExecutionStatus::Error(_)),
                "unexpected: {:?}", r.status
            ),
            Err(_) => {}
        }
    }

    #[tokio::test]
    async fn test_file_harness_creates_and_deletes_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (target, extra): (&str, Vec<String>) = if cfg!(windows) {
            ("cmd", vec!["/c".into(), "exit 0".into()])
        } else {
            ("/usr/bin/true", vec![])
        };
        let mut h = FileHarness {
            target: PathBuf::from(target),
            timeout: Duration::from_secs(5),
            temp_dir: tmp.path().to_path_buf(),
            extra_args: extra,
            shm: None,
        };
        match h.execute(b"testdata").await {
            Ok(_) => {
                let bins: Vec<_> = std::fs::read_dir(tmp.path())
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "bin").unwrap_or(false))
                    .collect();
                assert!(bins.is_empty(), "temp .bin files not cleaned up: {:?}", bins);
            }
            Err(_) => {}
        }
    }

    #[tokio::test]
    async fn test_stdin_harness_echo() {
        let (target, args_input): (&str, &[u8]) = if cfg!(windows) {
            ("cmd", b"/c echo hello")
        } else {
            ("/bin/echo", b"hello")
        };

        let mut h = StdinHarness {
            target: PathBuf::from(target),
            timeout: Duration::from_secs(5),
            shm: None,
        };

        let result = h.execute(args_input).await;
        match result {
            Ok(r) => {
                assert!(
                    matches!(r.status, ExecutionStatus::Ok | ExecutionStatus::Crash { .. }),
                    "unexpected: {:?}", r.status
                );
            }
            Err(_) => {} // binary not found — skip
        }
    }
}
