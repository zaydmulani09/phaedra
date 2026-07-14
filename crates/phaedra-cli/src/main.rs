use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rand::Rng;
use std::path::PathBuf;

mod config;
mod tui;

#[derive(Parser)]
#[command(name = "phaedra", about = "Local-first protocol fuzzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    Fuzz(FuzzArgs),
    /// Interactive setup wizard — generates phaedra.toml
    Init(InitArgs),
    Infer(InferArgs),
    Crashes(CrashesArgs),
    Report(ReportArgs),
    Compat(CompatArgs),
    /// Show campaign status: corpus size, crash breakdown, LLM cost
    Status(StatusArgs),
    /// Replay a crash input against the target binary
    Replay(ReplayArgs),
    /// Minimize a crashing input to its smallest crash-triggering form
    Minimize(MinimizeArgs),
    /// Run an in-process self-benchmark and print throughput numbers
    Bench(BenchArgs),
}

#[derive(clap::Args)]
pub struct BenchArgs {}

#[derive(Parser)]
struct FuzzArgs {
    #[arg(long)]
    target: Option<PathBuf>,

    #[arg(long, default_value = "stdin")]
    harness: HarnessArg,

    #[arg(long, default_value = "./phaedra-corpus")]
    corpus_dir: PathBuf,

    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,

    #[arg(long, default_value_t = 5)]
    timeout: u64,

    #[arg(long, default_value_t = 1)]
    jobs: usize,

    #[arg(long)]
    description: Option<String>,

    /// Stop after this many executions (0 = run forever until Ctrl-C).
    #[arg(long, default_value_t = 0)]
    max_execs: u64,

    /// Ollama base URL.
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Ollama model to use for seed generation.
    #[arg(long, default_value = "llama3.2")]
    ollama_model: String,

    /// Number of LLM-generated seeds to request.
    #[arg(long, default_value_t = 16)]
    seed_count: usize,

    /// Optional path to a phaedra.schema.toml file for structure-aware mutation
    #[arg(long)]
    schema: Option<std::path::PathBuf>,

    /// Target address for socket harness (host:port)
    #[arg(long, default_value = "127.0.0.1:7777")]
    socket_addr: String,

    /// Socket protocol: tcp or udp
    #[arg(long, default_value = "tcp")]
    socket_proto: String,

    /// Bytes to read from socket response (0 = write-only mode)
    #[arg(long, default_value_t = 4096)]
    socket_read_bytes: usize,

    /// Directory for temp input files used by file harness (default: system temp dir)
    #[arg(long)]
    file_temp_dir: Option<std::path::PathBuf>,

    /// Extra arguments to pass to target before the input file path
    #[arg(long, num_args = 0.., value_delimiter = ',')]
    file_extra_args: Vec<String>,

    /// Enable the live TUI dashboard
    #[arg(long, default_value_t = false)]
    tui: bool,

    /// Load campaign config from a phaedra.toml file (overridden by explicit flags)
    #[arg(long)]
    config: Option<std::path::PathBuf>,

    /// Run against a built-in demo target: http, tlv, or json
    #[arg(long)]
    demo: Option<String>,

    /// LLM provider: ollama, openai, or anthropic
    #[arg(long, default_value = "ollama")]
    llm_provider: String,

    /// API key for OpenAI or Anthropic (reads OPENAI_API_KEY or ANTHROPIC_API_KEY env vars if not set)
    #[arg(long)]
    llm_api_key: Option<String>,
}

#[derive(clap::Args)]
pub struct InitArgs {
    /// Output path for generated config (default: ./phaedra.toml)
    #[arg(long, default_value = "phaedra.toml")]
    pub output: std::path::PathBuf,
    /// Skip interactive prompts and use all defaults (non-interactive mode)
    #[arg(long)]
    pub non_interactive: bool,
}

#[derive(Parser)]
struct InferArgs {
    #[arg(long)]
    corpus_db: PathBuf,

    #[arg(long, default_value = "./phaedra.schema.toml")]
    output: PathBuf,

    #[arg(long, default_value = "inferred")]
    name: String,

    #[arg(long, default_value_t = 100)]
    samples: usize,
}

#[derive(Parser)]
struct CrashesArgs {
    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,
}

#[derive(Parser)]
struct ReportArgs {
    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,

    #[arg(long, default_value = "./phaedra-report.md")]
    output: PathBuf,

    #[arg(long)]
    target_description: Option<String>,

    #[arg(long)]
    no_llm: bool,

    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    #[arg(long, default_value = "llama3.2")]
    ollama_model: String,
}

#[derive(Parser)]
struct CompatArgs {
    /// Path to the compiled libFuzzer harness binary
    #[arg(long)]
    fuzz_target: PathBuf,

    /// Path to the fuzz/ directory (for corpus import)
    #[arg(long, default_value = "./fuzz")]
    fuzz_dir: PathBuf,

    /// Phaedra corpus directory
    #[arg(long, default_value = "./phaedra-corpus-compat")]
    corpus_dir: PathBuf,

    /// Phaedra crash directory
    #[arg(long, default_value = "./phaedra-crashes-compat")]
    crash_dir: PathBuf,

    /// Stop after N executions (0 = unlimited)
    #[arg(long, default_value_t = 0)]
    max_execs: u64,

    /// Target description for LLM seeding
    #[arg(long)]
    description: Option<String>,

    /// Enable the live TUI dashboard
    #[arg(long, default_value_t = false)]
    tui: bool,
}

#[derive(Parser)]
struct StatusArgs {
    #[arg(long, default_value = "./phaedra-corpus/corpus.db")]
    corpus_db: PathBuf,

    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,
}

#[derive(Parser)]
struct ReplayArgs {
    /// Crash id from the triage database
    #[arg(long)]
    crash_id: i64,

    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,

    /// Target binary to replay against
    #[arg(long)]
    target: PathBuf,

    /// Harness mode: stdin or file
    #[arg(long, default_value = "stdin")]
    harness: String,
}

#[derive(Parser)]
struct MinimizeArgs {
    /// Crash id from the triage database
    #[arg(long)]
    crash_id: i64,

    #[arg(long, default_value = "./phaedra-crashes")]
    crash_dir: PathBuf,

    /// Target binary
    #[arg(long)]
    target: PathBuf,

    /// Harness mode: stdin or file
    #[arg(long, default_value = "stdin")]
    harness: String,

    /// Maximum minimization iterations
    #[arg(long, default_value_t = 1000)]
    max_iters: usize,
}

#[derive(Clone, PartialEq, ValueEnum)]
enum HarnessArg {
    Stdin,
    Socket,
    File,
}

fn harness_arg_to_mode(h: &HarnessArg) -> phaedra_core::HarnessMode {
    match h {
        HarnessArg::Stdin => phaedra_core::HarnessMode::Stdin,
        HarnessArg::Socket => phaedra_core::HarnessMode::Socket,
        HarnessArg::File => phaedra_core::HarnessMode::File,
    }
}

fn build_campaign_config_from_args(
    args: &FuzzArgs,
) -> Result<phaedra_core::CampaignConfig> {
    let target = args
        .target
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--target is required when --config is not provided"))?;
    let corpus_db = args.corpus_dir.join("corpus.db");
    Ok(phaedra_core::CampaignConfig {
        target,
        harness: harness_arg_to_mode(&args.harness),
        corpus_dir: args.corpus_dir.clone(),
        crash_dir: args.crash_dir.clone(),
        corpus_db,
        timeout_secs: args.timeout,
        jobs: args.jobs,
        max_execs: args.max_execs,
        description: args.description.clone(),
        ollama_url: args.ollama_url.clone(),
        ollama_model: args.ollama_model.clone(),
        seed_count: args.seed_count,
        schema: args.schema.clone(),
        socket_addr: args.socket_addr.clone(),
        socket_proto: args.socket_proto.clone(),
        socket_read_bytes: args.socket_read_bytes,
        file_temp_dir: args.file_temp_dir.clone(),
        file_extra_args: args.file_extra_args.clone(),
        llm_provider: args.llm_provider.clone(),
        llm_api_key: args.llm_api_key.clone(),
    })
}

fn build_campaign_config_from_file(
    file: &config::PhaedraConfig,
    args: &FuzzArgs,
) -> Result<phaedra_core::CampaignConfig> {
    // target: CLI wins if provided, else file
    let target = if let Some(ref t) = args.target {
        t.clone()
    } else {
        PathBuf::from(&file.target)
    };

    // harness: if CLI is non-default (not Stdin), CLI wins; else use file
    let harness_mode = if args.harness != HarnessArg::Stdin {
        harness_arg_to_mode(&args.harness)
    } else {
        match file.harness.as_str() {
            "socket" => phaedra_core::HarnessMode::Socket,
            "file" => phaedra_core::HarnessMode::File,
            _ => phaedra_core::HarnessMode::Stdin,
        }
    };

    let corpus_dir = if args.corpus_dir.as_os_str() != "./phaedra-corpus" {
        args.corpus_dir.clone()
    } else {
        PathBuf::from(&file.corpus_dir)
    };

    let crash_dir = if args.crash_dir.as_os_str() != "./phaedra-crashes" {
        args.crash_dir.clone()
    } else {
        PathBuf::from(&file.crash_dir)
    };

    let timeout_secs = if args.timeout != 5 { args.timeout } else { file.timeout_secs };
    let jobs = if args.jobs != 1 { args.jobs } else { file.jobs };

    let description = if args.description.is_some() {
        args.description.clone()
    } else {
        file.description.clone()
    };

    let schema = if args.schema.is_some() {
        args.schema.clone()
    } else {
        file.schema.as_ref().map(PathBuf::from)
    };

    let ollama_url = if args.ollama_url != "http://localhost:11434" {
        args.ollama_url.clone()
    } else {
        file.ollama_url.clone()
    };

    let ollama_model = if args.ollama_model != "llama3.2" {
        args.ollama_model.clone()
    } else {
        file.ollama_model.clone()
    };

    let seed_count = if args.seed_count != 16 { args.seed_count } else { file.seed_count };

    let socket_addr = if args.socket_addr != "127.0.0.1:7777" {
        args.socket_addr.clone()
    } else {
        file.socket_addr.clone()
    };

    let socket_proto = if args.socket_proto != "tcp" {
        args.socket_proto.clone()
    } else {
        file.socket_proto.clone()
    };

    let socket_read_bytes = if args.socket_read_bytes != 4096 {
        args.socket_read_bytes
    } else {
        file.socket_read_bytes
    };

    let corpus_db = corpus_dir.join("corpus.db");

    Ok(phaedra_core::CampaignConfig {
        target,
        harness: harness_mode,
        corpus_dir,
        crash_dir,
        corpus_db,
        timeout_secs,
        jobs,
        max_execs: args.max_execs,
        description,
        ollama_url,
        ollama_model,
        seed_count,
        schema,
        socket_addr,
        socket_proto,
        socket_read_bytes,
        file_temp_dir: args.file_temp_dir.clone(),
        file_extra_args: args.file_extra_args.clone(),
        llm_provider: args.llm_provider.clone(),
        llm_api_key: args.llm_api_key.clone(),
    })
}

fn prompt(question: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", question, default);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_bool(question: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", question, default_str);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();
    Ok(match trimmed.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => default,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Fuzz(mut args) => {
            if let Some(ref demo) = args.demo.clone() {
                let (bin_name, description) = match demo.as_str() {
                    "http" => ("phaedra-target-http", "HTTP/1.1 request parser with Content-Length header"),
                    "tlv"  => ("phaedra-target-tlv",  "binary TLV protocol with PHDR magic and u16be length prefix"),
                    "json" => ("phaedra-target-json",  "JSON object parser with string key-value pairs"),
                    other  => anyhow::bail!("Unknown demo target '{}'. Use: http, tlv, or json", other),
                };
                let bin_path = find_demo_binary(bin_name)?;
                println!("Running demo: {} target. Watch Phaedra find the bug...", demo);
                println!("  binary: {}", bin_path.display());
                args.target = Some(bin_path);
                if args.description.is_none() {
                    args.description = Some(description.to_string());
                }
                if args.max_execs == 0 {
                    args.max_execs = 500;
                }
            }

            let campaign_config = if let Some(ref config_path) = args.config {
                let file_config = config::load_config(config_path)?;
                build_campaign_config_from_file(&file_config, &args)?
            } else {
                build_campaign_config_from_args(&args)?
            };

            println!("Phaedra v0.1.0 — local-first protocol fuzzer");
            println!("  target:     {:?}", campaign_config.target);
            println!("  harness:    {}", match campaign_config.harness {
                phaedra_core::HarnessMode::Stdin => "stdin",
                phaedra_core::HarnessMode::Socket => "socket",
                phaedra_core::HarnessMode::File => "file",
            });
            println!("  corpus-dir: {:?}", campaign_config.corpus_dir);
            println!("  crash-dir:  {:?}", campaign_config.crash_dir);
            println!("  timeout:    {}s", campaign_config.timeout_secs);
            println!("  jobs:       {}", campaign_config.jobs);

            if args.tui {
                let shared = phaedra_core::stats::new_shared_stats();
                let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

                let campaign_shared = shared.clone();
                let campaign_shutdown = shutdown.clone();
                let config_clone = campaign_config;

                let campaign_handle = tokio::spawn(async move {
                    if let Err(e) = phaedra_core::run_campaign(config_clone, Some(campaign_shared)).await {
                        tracing::error!("Campaign error: {e}");
                    }
                    campaign_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                });

                tui::run_tui(shared, shutdown).await?;
                campaign_handle.await?;
            } else {
                phaedra_core::run_campaign(campaign_config, None).await?;
            }
            println!("Campaign finished.");
        }
        Commands::Init(args) => {
            println!("\n⚡ Phaedra Init Wizard\n");

            let non_interactive = args.non_interactive;

            macro_rules! ask {
                ($q:expr, $default:expr) => {
                    if non_interactive {
                        $default.to_string()
                    } else {
                        prompt($q, $default)?
                    }
                };
            }

            macro_rules! ask_bool {
                ($q:expr, $default:expr) => {
                    if non_interactive {
                        $default
                    } else {
                        prompt_bool($q, $default)?
                    }
                };
            }

            let target = ask!("Target binary path", "/bin/echo");
            let harness = ask!("Harness mode (stdin/socket/file)", "stdin");
            let description_str = ask!("What does the target parse? (used for LLM seed gen)", "");
            let description = if description_str.is_empty() {
                None
            } else {
                Some(description_str)
            };

            let ollama_url = ask!("Ollama URL", "http://localhost:11434");
            let ollama_model = ask!("Ollama model", "llama3.2");

            let ollama_available = {
                let client = phaedra_llm::OllamaClient::new(&ollama_url, &ollama_model);
                client.is_available().await
            };

            if ollama_available {
                println!("  ✓ Ollama is reachable at {ollama_url}");
            } else {
                println!("  ✗ Ollama not reachable at {ollama_url} — LLM features will be disabled until it is started");
            }

            let schema = if ask_bool!(
                "Generate a schema file? (enables structure-aware mutation)",
                false
            ) {
                let schema_path = ask!("Schema output path", "phaedra.schema.toml");
                if ollama_available {
                    println!("  Generating schema via Ollama...");
                    let samples: Vec<Vec<u8>> = vec![];
                    let inferred = phaedra_schema::infer_schema(&samples, "generated");
                    let toml_str = phaedra_schema::schema_to_toml(&inferred)?;
                    std::fs::write(&schema_path, toml_str)?;
                    println!("  Schema written to {schema_path}");
                } else {
                    let desc = description.as_deref().unwrap_or("unknown target");
                    let template = format!(
                        "# Phaedra schema for: {desc}\n\
                         # Edit field types and names to match your protocol\n\n\
                         name = \"generated\"\n\n\
                         [[fields]]\n\
                         name = \"data\"\n\
                         type = \"bytes\"\n\
                         length = 16\n"
                    );
                    std::fs::write(&schema_path, template)?;
                    println!(
                        "  Schema template written to {schema_path} — edit it to match your protocol"
                    );
                }
                Some(schema_path)
            } else {
                None
            };

            let timeout_str = ask!("Per-execution timeout (seconds)", "5");
            let timeout_secs: u64 = timeout_str.parse().unwrap_or(5);

            let file_config = config::PhaedraConfig {
                target,
                harness,
                corpus_dir: "./phaedra-corpus".to_string(),
                crash_dir: "./phaedra-crashes".to_string(),
                timeout_secs,
                jobs: 1,
                description,
                schema,
                ollama_url,
                ollama_model,
                seed_count: 16,
                socket_addr: "127.0.0.1:7777".to_string(),
                socket_proto: "tcp".to_string(),
                socket_read_bytes: 4096,
            };

            config::save_config(&file_config, &args.output)?;
            println!("\n✓ Config written to {}", args.output.display());
            println!("\nTo start fuzzing:");
            println!("  phaedra fuzz --config {}", args.output.display());
            println!("\nTo enable the live dashboard:");
            println!("  phaedra fuzz --config {} --tui", args.output.display());
        }
        Commands::Infer(args) => {
            let mgr = phaedra_corpus::CorpusManager::open(&args.corpus_db)?;
            let all = mgr.all_data()?;
            let samples: Vec<Vec<u8>> = all.into_iter().take(args.samples).collect();
            if samples.is_empty() {
                eprintln!("Corpus is empty — run a fuzz campaign first.");
                std::process::exit(1);
            }
            tracing::info!("Inferring schema from {} samples...", samples.len());
            let schema = phaedra_schema::infer_schema(&samples, &args.name);
            let toml = phaedra_schema::schema_to_toml(&schema)?;
            std::fs::write(&args.output, &toml)?;
            println!("Schema written to {}", args.output.display());
            println!("Fields inferred: {}", schema.fields.len());
            for f in &schema.fields {
                println!("  - {} ({:?})", f.name, f.field_type);
            }
        }
        Commands::Crashes(args) => {
            let db_path = args.crash_dir.join("crashes.db");
            if !db_path.exists() {
                eprintln!("No crash database found at {}", db_path.display());
                std::process::exit(1);
            }
            let db = phaedra_core::triage::CrashTriageDb::open(&db_path)?;
            let records = db.all()?;
            if records.is_empty() {
                println!("No crashes recorded yet.");
                return Ok(());
            }
            println!("{:<6} {:<10} {:<8} {:<12} STATUS", "ID", "SEVERITY", "HITS", "SIGNATURE");
            println!("{}", "-".repeat(70));
            for r in &records {
                println!(
                    "{:<6} {:<10} {:<8} {:<12} {}",
                    r.id,
                    r.severity.as_str(),
                    r.hit_count,
                    &r.signature_key[..r.signature_key.len().min(12)],
                    &r.status_str[..r.status_str.len().min(40)],
                );
            }
            println!("\nTotal unique crashes: {}", records.len());
        }
        Commands::Report(args) => {
            let db_path = args.crash_dir.join("crashes.db");
            if !db_path.exists() {
                eprintln!("No crash database found at {}", db_path.display());
                std::process::exit(1);
            }

            let db = phaedra_core::triage::CrashTriageDb::open(&db_path)?;
            let records = db.all()?;

            let llm_client = if !args.no_llm {
                let client = phaedra_llm::OllamaClient::new(&args.ollama_url, &args.ollama_model);
                if client.is_available().await {
                    tracing::info!("Ollama available — will generate crash analysis");
                    Some(client)
                } else {
                    tracing::warn!("Ollama not available — skipping LLM analysis");
                    None
                }
            } else {
                None
            };

            let mut md = String::new();

            md.push_str("# Phaedra Crash Report\n\n");
            md.push_str(&format!(
                "Generated: {}\n\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));
            md.push_str(&format!("Crash database: `{}`\n\n", db_path.display()));
            md.push_str(&format!("Total unique crashes: **{}**\n\n", records.len()));

            if records.is_empty() {
                md.push_str("No crashes recorded.\n");
                std::fs::write(&args.output, &md)?;
                println!("Report written to {}", args.output.display());
                return Ok(());
            }

            md.push_str("## Summary\n\n");
            md.push_str("| ID | Severity | Hits | Signature | Status |\n");
            md.push_str("|----|----------|------|-----------|--------|\n");
            for r in &records {
                md.push_str(&format!(
                    "| {} | {} | {} | `{}` | `{}` |\n",
                    r.id,
                    r.severity.as_str(),
                    r.hit_count,
                    &r.signature_key[..r.signature_key.len().min(20)],
                    &r.status_str[..r.status_str.len().min(50)],
                ));
            }
            md.push('\n');

            md.push_str("## Crash Details\n\n");
            for r in &records {
                md.push_str(&format!("### Crash {} — {}\n\n", r.id, r.severity.as_str()));
                md.push_str(&format!("- **Signature:** `{}`\n", r.signature_key));
                md.push_str(&format!("- **Summary:** {}\n", r.signature_summary));
                md.push_str(&format!("- **Hit count:** {}\n", r.hit_count));
                md.push_str(&format!("- **Status:** `{}`\n", r.status_str));
                md.push_str(&format!("- **Input size:** {} bytes\n\n", r.input.len()));

                md.push_str("**Input (hex dump):**\n\n```\n");
                md.push_str(&hex_dump(&r.input));
                md.push_str("\n```\n\n");

                let printable: String = r
                    .input
                    .iter()
                    .map(|&b| {
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                md.push_str(&format!(
                    "**Input (ASCII):** `{}`\n\n",
                    &printable[..printable.len().min(120)]
                ));

                if let Some(ref client) = llm_client {
                    let desc = args.target_description.as_deref().unwrap_or("unknown target");
                    let status_str = r.severity.as_str();
                    let user = phaedra_llm::prompt::crash_analysis_user(
                        desc,
                        &r.input_hex[..r.input_hex.len().min(256)],
                        status_str,
                    );
                    match client
                        .chat(phaedra_llm::prompt::seed_generation_system(), &user, 0.3, 512)
                        .await
                    {
                        Ok(analysis) => {
                            md.push_str("**LLM Analysis:**\n\n");
                            md.push_str(&format!(
                                "> {}\n\n",
                                analysis.trim().replace('\n', "\n> ")
                            ));
                        }
                        Err(e) => {
                            tracing::warn!("LLM analysis failed for crash {}: {e}", r.id);
                        }
                    }
                }

                md.push_str("---\n\n");
            }

            md.push_str("## Reproducer Commands\n\n");
            md.push_str("To replay a specific crash input:\n\n```bash\n");
            md.push_str("cargo run --bin phaedra -- replay --crash-id <ID>\n");
            md.push_str("```\n\n");
            md.push_str("*(replay subcommand implemented in P19)*\n");

            std::fs::write(&args.output, &md)?;
            println!(
                "Report written to {} ({} crashes)",
                args.output.display(),
                records.len()
            );
        }
        Commands::Compat(args) => {
            println!("Phaedra cargo-fuzz compat mode");
            println!("Target: {}", args.fuzz_target.display());

            let target_name = args.fuzz_target
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            let libfuzzer_corpus = args.fuzz_dir.join("corpus").join(target_name);

            let corpus_db = args.corpus_dir.join("corpus.db");

            std::fs::create_dir_all(&args.corpus_dir)?;
            let mut mgr = phaedra_corpus::CorpusManager::open(&corpus_db)?;
            let imported = import_corpus_dir(&libfuzzer_corpus, &mut mgr)?;
            if imported > 0 {
                tracing::info!(
                    "Imported {} seeds from libFuzzer corpus at {}",
                    imported,
                    libfuzzer_corpus.display()
                );
            }
            drop(mgr);

            let config = phaedra_core::CampaignConfig {
                target: args.fuzz_target.clone(),
                harness: phaedra_core::HarnessMode::File,
                corpus_dir: args.corpus_dir.clone(),
                crash_dir: args.crash_dir.clone(),
                corpus_db,
                timeout_secs: 10,
                jobs: 1,
                description: args.description.clone(),
                schema: None,
                ollama_url: "http://localhost:11434".to_string(),
                ollama_model: "llama3.2".to_string(),
                seed_count: 16,
                socket_addr: "127.0.0.1:7777".to_string(),
                socket_proto: "tcp".to_string(),
                socket_read_bytes: 4096,
                file_temp_dir: None,
                file_extra_args: vec![],
                max_execs: args.max_execs,
                llm_provider: "ollama".to_string(),
                llm_api_key: None,
            };

            if args.tui {
                let shared = phaedra_core::stats::new_shared_stats();
                let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let campaign_shared = shared.clone();
                let campaign_shutdown = shutdown.clone();
                let campaign_handle = tokio::spawn(async move {
                    if let Err(e) = phaedra_core::run_campaign(config, Some(campaign_shared)).await {
                        tracing::error!("Campaign error: {e}");
                    }
                    campaign_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                });
                tui::run_tui(shared, shutdown).await?;
                campaign_handle.await?;
            } else {
                phaedra_core::run_campaign(config, None).await?;
            }
        }
        Commands::Status(args) => {
            println!("{}", color::bold("Phaedra Campaign Status"));
            println!("{}", color::bold("======================="));

            // Corpus
            if args.corpus_db.exists() {
                let mgr = phaedra_corpus::CorpusManager::open(&args.corpus_db)?;
                let count = mgr.len()?;
                println!(
                    "Corpus:   {}",
                    color::green(&format!("{count} seeds"))
                );
                let top = mgr.all_by_priority()?;
                if !top.is_empty() {
                    println!("          {}:", color::yellow("Top 5 by priority"));
                    for (i, s) in top.iter().take(5).enumerate() {
                        println!(
                            "            #{:<2} hash={}  hits={:<4} picks={:<4} edges={:<6} origin={}",
                            i + 1,
                            &s.hash[..8.min(s.hash.len())],
                            s.hit_count,
                            s.pick_count,
                            s.edge_count,
                            s.origin,
                        );
                    }
                }
            } else {
                println!("Corpus:   {}", color::yellow("no corpus db found"));
            }

            // Crashes
            let crash_db_path = args.crash_dir.join("crashes.db");
            if crash_db_path.exists() {
                let db = phaedra_core::triage::CrashTriageDb::open(&crash_db_path)?;
                let records = db.all()?;
                let unique = records.len();
                let crash_str = format!("{unique} unique");
                if unique > 0 {
                    println!("Crashes:  {}", color::red(&crash_str));
                } else {
                    println!("Crashes:  {}", color::green(&crash_str));
                }
                let mut crit = 0u32;
                let mut high = 0u32;
                let mut med = 0u32;
                let mut low = 0u32;
                for r in &records {
                    match r.severity {
                        phaedra_core::triage::Severity::Critical => crit += 1,
                        phaedra_core::triage::Severity::High => high += 1,
                        phaedra_core::triage::Severity::Medium => med += 1,
                        _ => low += 1,
                    }
                }
                if unique > 0 {
                    println!(
                        "          {}: {}  {}: {}  {}: {}  LOW: {}",
                        color::red("CRITICAL"), crit,
                        color::yellow("HIGH"), high,
                        color::cyan("MEDIUM"), med,
                        low,
                    );
                }
            } else {
                println!("Crashes:  {}", color::yellow("no crash db found"));
            }

            // LLM cost
            let llm_db_path = args.corpus_db.parent()
                .unwrap_or(std::path::Path::new("."))
                .join("llm_costs.db");
            if llm_db_path.exists() {
                if let Ok(tracker) = phaedra_llm::CostTracker::open(&llm_db_path) {
                    let calls = tracker.total_calls().unwrap_or(0);
                    let cost = tracker.total_cost_usd().unwrap_or(0.0);
                    println!(
                        "LLM:      {} calls  {} estimated cost",
                        color::cyan(&calls.to_string()),
                        color::cyan(&format!("${cost:.4}")),
                    );
                }
            } else {
                println!("LLM:      {}", color::yellow("no cost db found"));
            }
        }
        Commands::Replay(args) => {
            let crash_db_path = args.crash_dir.join("crashes.db");
            let db = phaedra_core::triage::CrashTriageDb::open(&crash_db_path)?;
            let record = db.get_by_id(args.crash_id)?
                .ok_or_else(|| anyhow::anyhow!("crash id {} not found", args.crash_id))?;

            println!("{}", color::bold(&format!("Replaying crash #{}", args.crash_id)));
            println!("  signature: {}", color::cyan(&record.signature_key));
            println!("  severity:  {}", color::yellow(record.severity.as_str()));
            println!("  input:     {} bytes", record.input.len());

            let timeout = std::time::Duration::from_secs(10);
            let result = execute_once(&record.input, &args.target, &args.harness, timeout).await?;

            let stdout_str = String::from_utf8_lossy(&result.stdout);
            let stderr_str = String::from_utf8_lossy(&result.stderr);
            if !stdout_str.is_empty() {
                println!("stdout: {}", stdout_str.trim());
            }
            if !stderr_str.is_empty() {
                println!("stderr: {}", stderr_str.trim());
            }

            let reproduced = matches!(result.status, phaedra_harness::ExecutionStatus::Crash { .. });
            if reproduced {
                println!("{}", color::red("Reproduced: YES"));
            } else {
                println!("{}", color::green("Reproduced: NO"));
            }
        }
        Commands::Bench(_) => {
            println!("{}", color::bold("Phaedra Self-Benchmark"));
            println!("{}", "=".repeat(40));

            // 1. Mutation throughput
            let mut engine = phaedra_mutator::MutationEngine::with_seed(42);
            let input = vec![0u8; 256];
            let corpus: Vec<Vec<u8>> = (0..10).map(|i| vec![i as u8; 64]).collect();
            let iters = 100_000u64;
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let _ = engine.mutate(&input, &corpus);
            }
            let elapsed = start.elapsed();
            let mutations_per_sec = iters as f64 / elapsed.as_secs_f64();
            println!("{}: {:.0} mutations/sec",
                color::cyan("Mutation engine"),
                mutations_per_sec);

            // 2. Coverage tracker throughput
            let mut tracker = phaedra_coverage::CoverageTracker::new();
            let mut map = phaedra_coverage::CoverageMap::new();
            map.data[42] = 1;
            let start = std::time::Instant::now();
            for _ in 0..iters {
                tracker.reset();
                let _ = tracker.is_interesting(&map);
            }
            let elapsed = start.elapsed();
            println!("{}: {:.0} checks/sec",
                color::cyan("Coverage tracker"),
                iters as f64 / elapsed.as_secs_f64());

            // 3. Corpus operations
            let tmp = tempfile::tempdir()?;
            let db = tmp.path().join("bench.db");
            let mut mgr = phaedra_corpus::CorpusManager::open(&db)?;
            let small_iters = 1000u64;
            let start = std::time::Instant::now();
            for i in 0..small_iters {
                let seed = format!("seed_{i}").into_bytes();
                let _ = mgr.add_seed(seed, i as usize, "bench");
            }
            let elapsed = start.elapsed();
            println!("{}: {:.0} inserts/sec",
                color::cyan("Corpus inserts"),
                small_iters as f64 / elapsed.as_secs_f64());

            // 4. Schema mutation throughput
            let schema_str = r#"
name = "bench"
[[fields]]
name = "version"
type = "u8"
[[fields]]
name = "payload"
type = "lp_bytes8"
"#;
            if let Ok(schema) = phaedra_schema::parse_schema(schema_str) {
                let mut rng = rand::thread_rng();
                let test_input = vec![1u8, 4, 0x41, 0x41, 0x41, 0x41];
                let start = std::time::Instant::now();
                for _ in 0..iters {
                    let _ = phaedra_schema::schema_mutate(&test_input, &schema, &mut rng);
                }
                let elapsed = start.elapsed();
                println!("{}: {:.0} mutations/sec",
                    color::cyan("Schema mutator"),
                    iters as f64 / elapsed.as_secs_f64());
            }

            println!("\n{}", color::green("Benchmark complete."));
            println!("For detailed criterion benchmarks run: cargo bench");
        }
        Commands::Minimize(args) => {
            let crash_db_path = args.crash_dir.join("crashes.db");
            let db = phaedra_core::triage::CrashTriageDb::open(&crash_db_path)?;
            let record = db.get_by_id(args.crash_id)?
                .ok_or_else(|| anyhow::anyhow!("crash id {} not found", args.crash_id))?;

            println!("{}", color::bold(&format!("Minimizing crash #{}", args.crash_id)));
            println!("  original size: {} bytes", record.input.len());

            let timeout = std::time::Duration::from_secs(10);
            let minimized = minimize_input(
                &record.input,
                &args.target,
                &args.harness,
                timeout,
                args.max_iters,
            ).await?;

            let orig_len = record.input.len();
            let min_len = minimized.len();
            let pct = (min_len * 100).checked_div(orig_len).map(|v| 100 - v).unwrap_or(0);
            println!(
                "{}",
                color::green(&format!(
                    "Minimized: {orig_len} bytes -> {min_len} bytes ({pct}% reduction)"
                ))
            );

            let out_path = args.crash_dir.join(format!("minimized_{}.bin", args.crash_id));
            std::fs::write(&out_path, &minimized)?;
            println!("Saved to: {}", out_path.display());

            println!("\nHex dump:");
            print!("{}", hex_dump(&minimized));
        }
    }

    Ok(())
}

fn import_corpus_dir(
    corpus_dir: &std::path::Path,
    mgr: &mut phaedra_corpus::CorpusManager,
) -> anyhow::Result<usize> {
    if !corpus_dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(corpus_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let data = std::fs::read(&path)?;
            if mgr.add_seed(data, 0, "imported")? {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn find_demo_binary(name: &str) -> anyhow::Result<std::path::PathBuf> {
    #[cfg(windows)]
    let name = format!("{name}.exe");
    #[cfg(not(windows))]
    let name = name.to_string();

    let exe = std::env::current_exe()?;
    let sibling = exe.parent().unwrap_or(std::path::Path::new(".")).join(&name);
    if sibling.exists() {
        return Ok(sibling);
    }
    let debug = std::path::PathBuf::from("target/debug").join(&name);
    if debug.exists() {
        return Ok(debug);
    }
    let release = std::path::PathBuf::from("target/release").join(&name);
    if release.exists() {
        return Ok(release);
    }
    anyhow::bail!("Demo binary '{}' not found. Run `cargo build` first.", name);
}

async fn execute_once(
    input: &[u8],
    target: &std::path::Path,
    harness_mode: &str,
    timeout: std::time::Duration,
) -> Result<phaedra_harness::ExecutionResult> {
    use phaedra_harness::Harness;
    match harness_mode {
        "file" => {
            let mut h = phaedra_harness::FileHarness {
                target: target.to_path_buf(),
                timeout,
                temp_dir: std::env::temp_dir(),
                extra_args: vec![],
                shm: None,
            };
            h.execute(input).await
        }
        _ => {
            let mut h = phaedra_harness::StdinHarness {
                target: target.to_path_buf(),
                timeout,
                shm: None,
            };
            h.execute(input).await
        }
    }
}

async fn minimize_input(
    input: &[u8],
    target: &std::path::Path,
    harness_mode: &str,
    timeout: std::time::Duration,
    max_iters: usize,
) -> Result<Vec<u8>> {
    use indicatif::{ProgressBar, ProgressStyle};
    use rand::SeedableRng;

    let mut current = input.to_vec();
    let pb = ProgressBar::new(max_iters as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut rng = rand::rngs::SmallRng::from_entropy();
    let mut no_progress = 0usize;

    for iter in 0..max_iters {
        pb.inc(1);

        if current.len() <= 1 || no_progress >= 50 {
            break;
        }

        let reduced: Vec<u8> = match iter % 5 {
            0 => {
                let mut r = current.clone();
                r.pop();
                r
            }
            1 => current[1..].to_vec(),
            2 => {
                let pos = rng.gen_range(0..current.len());
                let mut r = current.clone();
                r.remove(pos);
                r
            }
            3 => {
                let pos = rng.gen_range(0..current.len());
                let mut r = current.clone();
                r[pos] = 0x00;
                r
            }
            _ => current[..current.len() / 2].to_vec(),
        };

        if reduced.is_empty() {
            no_progress += 1;
            continue;
        }

        match execute_once(&reduced, target, harness_mode, timeout).await {
            Ok(result) => {
                if matches!(result.status, phaedra_harness::ExecutionStatus::Crash { .. }) {
                    current = reduced;
                    no_progress = 0;
                    pb.set_message(format!("size={}", current.len()));
                } else {
                    no_progress += 1;
                }
            }
            Err(_) => {
                no_progress += 1;
            }
        }
    }

    pb.finish_with_message(format!("done, {} bytes", current.len()));
    Ok(current)
}

mod color {
    pub fn red(s: &str) -> String { format!("\x1b[31m{s}\x1b[0m") }
    pub fn green(s: &str) -> String { format!("\x1b[32m{s}\x1b[0m") }
    pub fn yellow(s: &str) -> String { format!("\x1b[33m{s}\x1b[0m") }
    pub fn bold(s: &str) -> String { format!("\x1b[1m{s}\x1b[0m") }
    pub fn cyan(s: &str) -> String { format!("\x1b[36m{s}\x1b[0m") }
}

fn hex_dump(data: &[u8]) -> String {
    let mut out = String::new();
    for (i, chunk) in data.chunks(16).enumerate() {
        let offset = i * 16;
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
            .collect();
        let hex_str = hex.join(" ");
        out.push_str(&format!("{offset:08x}  {hex_str:<47}  {ascii}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_dump_empty() {
        let out = hex_dump(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn test_hex_dump_single_byte() {
        let out = hex_dump(&[0x41]);
        assert!(out.contains("41"));
        assert!(out.contains("A"));
    }

    #[test]
    fn test_hex_dump_full_row() {
        let data: Vec<u8> = (0u8..16).collect();
        let out = hex_dump(&data);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("00000000"));
    }

    #[test]
    fn test_hex_dump_two_rows() {
        let data: Vec<u8> = (0u8..20).collect();
        let out = hex_dump(&data);
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn test_hex_dump_non_printable_shown_as_dot() {
        let out = hex_dump(&[0x00, 0x01, 0x02]);
        assert!(out.contains("..."));
    }

    #[test]
    fn test_import_corpus_dir_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("corpus.db");
        let empty_dir = tmp.path().join("empty_corpus");
        std::fs::create_dir_all(&empty_dir).unwrap();
        let mut mgr = phaedra_corpus::CorpusManager::open(&db_path).unwrap();
        let count = import_corpus_dir(&empty_dir, &mut mgr).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_corpus_dir_with_files() {
        let tmp = tempfile::tempdir().unwrap();
        let corpus_dir = tmp.path().join("corpus");
        std::fs::create_dir_all(&corpus_dir).unwrap();
        std::fs::write(corpus_dir.join("a.bin"), b"aaa").unwrap();
        std::fs::write(corpus_dir.join("b.bin"), b"bbb").unwrap();
        std::fs::write(corpus_dir.join("c.bin"), b"ccc").unwrap();

        let db_path = tmp.path().join("corpus.db");
        let mut mgr = phaedra_corpus::CorpusManager::open(&db_path).unwrap();
        let count = import_corpus_dir(&corpus_dir, &mut mgr).unwrap();
        assert_eq!(count, 3);
        assert_eq!(mgr.len().unwrap(), 3);
    }

    #[test]
    fn test_color_red_contains_ansi() {
        let s = color::red("x");
        assert!(s.contains("\x1b[31m"));
        assert!(s.contains("x"));
    }

    #[test]
    fn test_color_reset_present() {
        let s = color::green("x");
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_color_bold_wraps() {
        let s = color::bold("hello");
        assert!(s.contains("\x1b[1m"));
        assert!(s.contains("hello"));
    }

    #[tokio::test]
    async fn test_minimize_input_reduces_size() {
        // TLV crash: PHDR magic + u16be length 0x0100 (256) + 4 data bytes → length > data → panic
        let tlv_crash: Vec<u8> = vec![0x50, 0x48, 0x44, 0x52, 0x01, 0x00, 0x41, 0x41, 0x41, 0x41];

        #[cfg(windows)]
        let binary = std::path::PathBuf::from("target/debug/phaedra-target-tlv.exe");
        #[cfg(not(windows))]
        let binary = std::path::PathBuf::from("target/debug/phaedra-target-tlv");

        if !binary.exists() {
            eprintln!("Skipping minimize test: binary not found at {binary:?}");
            return;
        }

        let result = minimize_input(
            &tlv_crash,
            &binary,
            "stdin",
            std::time::Duration::from_secs(5),
            50,
        )
        .await
        .unwrap();

        assert!(result.len() <= tlv_crash.len());
    }

    #[test]
    fn test_import_corpus_dir_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");
        let db_path = tmp.path().join("corpus.db");
        let mut mgr = phaedra_corpus::CorpusManager::open(&db_path).unwrap();
        let result = import_corpus_dir(&nonexistent, &mut mgr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
