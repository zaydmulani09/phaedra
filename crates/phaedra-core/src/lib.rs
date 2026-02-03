use anyhow::Result;
use rand::Rng;
use rand::SeedableRng;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

mod crash;
pub mod stats;
pub(crate) mod dedup;
pub mod triage;

pub use stats::CampaignStats;

#[derive(Clone)]
pub struct CampaignConfig {
    pub target: PathBuf,
    pub harness: HarnessMode,
    pub corpus_dir: PathBuf,
    pub crash_dir: PathBuf,
    pub corpus_db: PathBuf,
    pub timeout_secs: u64,
    pub jobs: usize,
    pub max_execs: u64,
    pub description: Option<String>,
    pub ollama_url: String,
    pub ollama_model: String,
    pub seed_count: usize,
    pub schema: Option<PathBuf>,
    pub socket_addr: String,
    pub socket_proto: String,
    pub socket_read_bytes: usize,
    pub file_temp_dir: Option<PathBuf>,
    pub file_extra_args: Vec<String>,
    pub llm_provider: String,
    pub llm_api_key: Option<String>,
}

#[derive(Clone)]
pub enum HarnessMode {
    Stdin,
    Socket,
    File,
}

fn extract_tokens_from_description(description: &str) -> Vec<Vec<u8>> {
    description
        .split(|c: char| c.is_whitespace() || ".,;:()[]{}\"'".contains(c))
        .filter(|s| s.len() >= 2 && s.len() <= 16)
        .map(|s| s.as_bytes().to_vec())
        .collect()
}

fn build_harness(
    config: &CampaignConfig,
    shm: Option<phaedra_coverage::SharedMemory>,
) -> Box<dyn phaedra_harness::Harness> {
    let timeout = std::time::Duration::from_secs(config.timeout_secs);
    match config.harness {
        HarnessMode::Stdin => Box::new(phaedra_harness::StdinHarness {
            target: config.target.clone(),
            timeout,
            shm,
        }),
        HarnessMode::Socket => {
            let addr = config.socket_addr.parse()
                .unwrap_or_else(|_| "127.0.0.1:7777".parse().unwrap());
            let protocol = if config.socket_proto == "udp" {
                phaedra_harness::SocketProtocol::Udp
            } else {
                phaedra_harness::SocketProtocol::Tcp
            };
            Box::new(phaedra_harness::SocketHarness {
                addr,
                timeout,
                protocol,
                read_bytes: config.socket_read_bytes,
                shm: None,
            })
        }
        HarnessMode::File => Box::new(phaedra_harness::FileHarness {
            target: config.target.clone(),
            timeout,
            temp_dir: config.file_temp_dir.clone().unwrap_or_else(std::env::temp_dir),
            extra_args: config.file_extra_args.clone(),
            shm,
        }),
    }
}

pub async fn run_campaign(config: CampaignConfig, shared: Option<stats::SharedStats>) -> Result<()> {
    tokio::fs::create_dir_all(&config.corpus_dir).await?;
    tokio::fs::create_dir_all(&config.crash_dir).await?;

    tracing::info!("Starting campaign against {:?}", config.target);

    let mut corpus_mgr = phaedra_corpus::CorpusManager::open(&config.corpus_db)?;

    let cost_db_path = config.corpus_dir.join("llm_costs.db");
    let cost_tracker = phaedra_llm::CostTracker::open(&cost_db_path).ok();

    let env_api_key;
    let api_key: Option<&str> = if let Some(ref k) = config.llm_api_key {
        Some(k.as_str())
    } else {
        env_api_key = match config.llm_provider.as_str() {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            _ => None,
        };
        env_api_key.as_deref()
    };
    let llm_client = phaedra_llm::LlmClient::from_config(
        &config.llm_provider,
        api_key,
        &config.ollama_model,
        &config.ollama_url,
    );

    if corpus_mgr.is_empty()? {
        if let Some(ref description) = config.description {
            let (generated, seed_response) = phaedra_llm::generate_seeds_with_client(
                description,
                config.seed_count,
                &llm_client,
            )
            .await;
            if let (Some(ref tracker), Some(ref resp)) = (&cost_tracker, seed_response) {
                if let Err(e) = tracker.record(resp, "seed_gen") {
                    tracing::warn!("Cost tracking failed: {e}");
                }
            }

            if generated.is_empty() {
                corpus_mgr.add_seed(b"phaedra".to_vec(), 0, "bootstrap")?;
                tracing::info!("LLM unavailable — bootstrapped with 1 fallback seed");
            } else {
                let mut added = 0usize;
                for seed in generated {
                    let label = seed.label.clone();
                    if corpus_mgr.add_seed(seed.data, 0, &format!("llm:{label}"))? {
                        added += 1;
                    }
                }
                tracing::info!("LLM bootstrap: added {added} seeds to corpus");
            }
        } else {
            corpus_mgr.add_seed(b"phaedra".to_vec(), 0, "bootstrap")?;
            tracing::info!("No description provided — bootstrapped with 1 fallback seed");
        }
    }

    let schema: Option<phaedra_schema::Schema> = if let Some(ref schema_path) = config.schema {
        match phaedra_schema::load_schema(schema_path) {
            Ok(s) => {
                tracing::info!("Loaded schema '{}' with {} fields", s.name, s.fields.len());
                Some(s)
            }
            Err(e) => {
                tracing::warn!("Failed to load schema: {e} — falling back to unstructured mutation");
                None
            }
        }
    } else {
        None
    };

    let crash_db_path = config.crash_dir.join("crashes.db");
    let mut triage_db = triage::CrashTriageDb::open(&crash_db_path)?;

    let mut rng = rand::rngs::SmallRng::from_entropy();
    let mut cov_tracker = phaedra_coverage::CoverageTracker::new();
    let shm = phaedra_coverage::SharedMemory::new().ok();
    if shm.is_some() {
        tracing::info!("Shared memory coverage enabled");
    }
    let mut mutator = phaedra_mutator::MutationEngine::new();
    if let Some(ref description) = config.description {
        let tokens = extract_tokens_from_description(description);
        mutator.add_tokens(tokens);
        tracing::debug!("Added {} tokens to mutation dictionary", mutator.tokens().len());
    }
    let mut harness = build_harness(&config, shm);
    let mut stats = CampaignStats::new();

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Ctrl-C received — shutting down gracefully...");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    let stats_interval = std::time::Duration::from_secs(30);
    let mut last_stats_print = std::time::Instant::now();
    let mut execs_since_last_stats: u64 = 0;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        if config.max_execs > 0 && stats.total_execs >= config.max_execs {
            tracing::info!("Reached max-execs limit ({}), stopping.", config.max_execs);
            break;
        }

        let base_seed = match corpus_mgr.pick()? {
            Some(s) => s,
            None => {
                tracing::warn!("Corpus is empty — waiting...");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        corpus_mgr.record_pick(base_seed.id)?;

        let corpus_data = corpus_mgr.all_data()?;
        let (mutated, strategy) = if let Some(ref s) = schema {
            if rng.gen_bool(0.3) {
                let mutated = phaedra_schema::schema_mutate(&base_seed.data, s, &mut rng);
                (mutated, phaedra_mutator::Strategy::ByteSubstitute)
            } else {
                mutator.mutate(&base_seed.data, &corpus_data)
            }
        } else {
            mutator.mutate(&base_seed.data, &corpus_data)
        };
        tracing::debug!("strategy: {:?}", strategy);

        let result = harness.execute(&mutated).await?;

        stats.total_execs += 1;
        execs_since_last_stats += 1;

        match &result.status {
            phaedra_harness::ExecutionStatus::Timeout => {
                stats.total_timeouts += 1;
                tracing::debug!("timeout on exec {}", stats.total_execs);
            }
            phaedra_harness::ExecutionStatus::Crash { .. }
            | phaedra_harness::ExecutionStatus::Error(_) => {
                stats.total_crashes += 1;
                let sig = dedup::CrashSignature::from_status(&result.status, &mutated);
                let sev = triage::Severity::from_status(&result.status);
                let status_str = format!("{:?}", result.status);
                let is_new = triage_db.record(&sig, &sev, &mutated, &status_str)?;
                if is_new {
                    crash::save_crash(&config.crash_dir, &mutated, &result.status)?;
                    tracing::warn!(
                        "[CRASH] NEW {} | sig={} | unique_crashes={}",
                        sev.as_str(),
                        sig.key,
                        triage_db.unique_count()?,
                    );
                    if let Some(ref shared) = shared {
                        if let Ok(mut live) = shared.lock() {
                            live.recent_crashes.push(sev.as_str().to_string());
                            if live.recent_crashes.len() > 5 {
                                live.recent_crashes.remove(0);
                            }
                        }
                    }
                } else {
                    tracing::debug!("[CRASH] duplicate sig={}", sig.key);
                }
            }
            phaedra_harness::ExecutionStatus::Ok => {
                if let Some(map) = &result.coverage {
                    if cov_tracker.is_interesting(map) {
                        let new_edges = cov_tracker.total_edges();
                        stats.interesting_execs += 1;
                        stats.unique_edges = new_edges;

                        let added = corpus_mgr.add_seed(
                            mutated.clone(),
                            new_edges,
                            &format!("{:?}", strategy),
                        )?;
                        if added {
                            corpus_mgr.record_hit(base_seed.id)?;
                            mutator.record_success(strategy);
                            stats.corpus_size = corpus_mgr.len()?;
                            tracing::info!(
                                "[+] new coverage via {:?} | edges={} corpus={}",
                                strategy,
                                new_edges,
                                stats.corpus_size,
                            );
                            if let Some(ref shared) = shared {
                                if let Ok(mut live) = shared.lock() {
                                    let label = format!("edges={new_edges} via {strategy:?}");
                                    live.recent_finds.push(label);
                                    if live.recent_finds.len() > 5 {
                                        live.recent_finds.remove(0);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if stats.total_execs % 10 == 0 {
            if let Some(ref shared) = shared {
                if let Ok(mut live) = shared.lock() {
                    live.total_execs = stats.total_execs;
                    live.interesting_execs = stats.interesting_execs;
                    live.total_crashes = stats.total_crashes;
                    live.unique_crashes = triage_db.unique_count().unwrap_or(0) as u64;
                    live.total_timeouts = stats.total_timeouts;
                    live.corpus_size = stats.corpus_size;
                    live.unique_edges = stats.unique_edges;
                    live.execs_per_sec = stats.execs_per_sec;
                    live.elapsed_secs = stats.elapsed().as_secs();
                    live.strategy_weights = phaedra_mutator::Strategy::all()
                        .iter()
                        .zip(mutator.weights().iter())
                        .map(|(s, w)| (format!("{s:?}"), *w))
                        .collect();
                }
            }
        }

        if last_stats_print.elapsed() >= stats_interval {
            let elapsed_secs = last_stats_print.elapsed().as_secs_f64();
            stats.execs_per_sec = execs_since_last_stats as f64 / elapsed_secs.max(0.001);
            tracing::info!(
                "stats | time={} execs={} exec/s={:.0} corpus={} edges={} crashes={} unique_crashes={} timeouts={}",
                stats.elapsed_hms(),
                stats.total_execs,
                stats.execs_per_sec,
                stats.corpus_size,
                stats.unique_edges,
                stats.total_crashes,
                triage_db.unique_count()?,
                stats.total_timeouts,
            );
            last_stats_print = std::time::Instant::now();
            execs_since_last_stats = 0;
        }
    }

    tracing::info!(
        "Campaign complete | execs={} corpus={} edges={} unique_crashes={} time={}",
        stats.total_execs,
        corpus_mgr.len()?,
        stats.unique_edges,
        triage_db.unique_count()?,
        stats.elapsed_hms(),
    );

    if let Some(ref tracker) = cost_tracker {
        if let (Ok(calls), Ok(cost)) = (tracker.total_calls(), tracker.total_cost_usd()) {
            tracing::info!(
                "LLM usage: {} calls, estimated ${:.4} total",
                calls,
                cost,
            );
        }
    }

    Ok(())
}
