# Phaedra

![CI](https://github.com/zaydmulani09/phaedra/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue)
![Version](https://img.shields.io/badge/version-0.1.0-orange)

Local-first, LLM-guided, coverage-driven fuzzer for network protocols and binary formats. Single Rust binary. Zero signup. Zero cloud.

## The problem

AFL++ and libFuzzer are excellent tools but they require you to write C harnesses, understand LLVM instrumentation, and manually seed your corpus with real examples. cargo-fuzz wraps libFuzzer but inherits all of the same constraints. If you are fuzzing a Rust service that speaks a custom binary protocol and you do not have a corpus of real traffic, you are starting from random bytes -- which means you spend the first hours of a campaign getting past the first length check.

## What Phaedra does differently

- **LLM seed bootstrap** -- describe what your target parses in plain English and Phaedra generates a structured initial corpus using a local Ollama model. No API key required.
- **Schema-aware mutation** -- define your protocol structure in a TOML schema file and Phaedra mutates at the field level: flipping length prefixes, corrupting magic bytes, overflowing integer fields. Random byte mutation still runs in parallel.
- **Coverage-guided** -- SanCov edge bitmaps via shared memory. Inputs that hit new code paths go into the corpus. Inputs that do not are discarded.
- **Crash triage built in** -- crashes are deduplicated by signal and input fingerprint, scored by severity (CRITICAL/HIGH/MEDIUM/LOW), stored in SQLite, and can be minimized with `phaedra minimize`.
- **cargo-fuzz compatible** -- point `phaedra compat` at an existing libFuzzer harness binary and it drives it using Phaedra's corpus and mutation engine.

## Quickstart

```bash
cargo install phaedra
```

Or build from source:

```bash
git clone https://github.com/zaydmulani09/phaedra
cd phaedra
cargo build --release
```

Run against a built-in demo target to see it find a real bug:

```bash
phaedra fuzz --demo http
```

```
phaedra v0.1.0 -- local-first protocol fuzzer

target       : phaedra-target-http
harness      : stdin
corpus-dir   : ./phaedra-corpus
crash-dir    : ./phaedra-crashes
timeout      : 5s
jobs         : 1

[INFO] No description provided -- bootstrapped with 1 fallback seed
[INFO] Starting campaign against "phaedra-target-http"
[DEBUG] strategy: ByteSubstitute
[DEBUG] strategy: Arithmetic
[DEBUG] strategy: TokenInsert
[WARN]  [CRASH] NEW LOW | sig=crash_3f2a1b0c | unique_crashes=1
[DEBUG] strategy: BlockFlip
[DEBUG] strategy: Havoc
[WARN]  [CRASH] NEW LOW | sig=crash_7e4d2c1a | unique_crashes=2
[INFO]  [+] new coverage via Arithmetic | edges=3 corpus=2
...
[INFO] Reached max-execs limit (500), stopping.
[INFO] Campaign complete | execs=500 corpus=8 edges=3 unique_crashes=47 time=00:00:12
```

With Ollama running:

```bash
phaedra fuzz --demo tlv --description "TLV binary protocol with PHDR magic header"
```

```
[INFO] Generating 16 seeds via Ollama (llama3.2)...
[INFO] LLM bootstrap: added 14 seeds to corpus
[WARN]  [CRASH] NEW LOW | sig=crash_1a2b3c4d | unique_crashes=1
```

## How it works

**Coverage engine** (`phaedra-coverage`) maintains a 65536-slot edge bitmap in POSIX shared memory. Before each execution the bitmap is cleared. After the process exits, Phaedra reads it and checks for edges not seen in previous runs. Inputs that open new edges go into the corpus.

**Mutation engine** (`phaedra-mutator`) implements 18 strategies including bit flip, arithmetic, interesting integer values, block operations, token insertion, cross-seed recombination, and havoc (chained mutations). Strategies are selected by weighted random choice; weights increase when a strategy produces corpus-worthy inputs and decay slowly over time so no strategy gets permanently starved.

**LLM seeding** (`phaedra-llm`) calls a local Ollama instance (or OpenAI/Anthropic with `--llm-provider`) with a structured prompt asking for hex-encoded seed inputs based on your target description. The response is parsed with a three-strategy fallback that handles malformed JSON gracefully.

**Schema DSL** (`phaedra-schema`) lets you describe your protocol in TOML with field types like `u32_be`, `lp_bytes16_be`, `cstring`, and `magic`. When a schema is loaded, 30% of mutations operate at the field level instead of on raw bytes.

**Crash triage** (`phaedra-core`) deduplicates crashes by normalizing the signal number and input fingerprint into a signature key. Each unique signature is stored once in SQLite with a hit counter. Only new unique crashes are written to disk.

## Subcommands

| Command | Description |
|---------|-------------|
| `phaedra fuzz` | Run a fuzzing campaign |
| `phaedra init` | Interactive setup wizard, generates `phaedra.toml` |
| `phaedra status` | Show corpus size, crash breakdown, LLM cost |
| `phaedra crashes` | Print crash triage table |
| `phaedra replay` | Replay a crash by ID and confirm reproduction |
| `phaedra minimize` | Delta-minimize a crashing input |
| `phaedra report` | Generate a markdown crash report |
| `phaedra infer` | Infer a schema from an existing corpus |
| `phaedra compat` | Drive a cargo-fuzz libFuzzer harness |
| `phaedra bench` | Run in-process throughput benchmark |

## Schema DSL

Define your protocol structure in TOML and Phaedra mutates at the field level:

```toml
name = "binary_tlv"
description = "Type-Length-Value binary protocol"

[[fields]]
name = "magic"
type = "magic"
length = 4
value = "50484452"
mutable = false

[[fields]]
name = "version"
type = "u8"

[[fields]]
name = "payload"
type = "lp_bytes16_be"

[[fields]]
name = "checksum"
type = "u32_be"
```

Supported field types: `u8`, `u16_be`, `u16_le`, `u32_be`, `u32_le`, `u64_be`, `u64_le`, `bytes`, `cstring`, `lp_bytes8`, `lp_bytes16_be`, `lp_bytes32_be`, `magic`, `padding`, `repeated`.

If you already have a corpus but no schema, let Phaedra infer one:

```bash
phaedra infer --corpus-db ./phaedra-corpus/corpus.db --name my_proto
```

## LLM backends

Ollama (default, local, free):
```bash
phaedra fuzz --target ./my_parser --description "parses length-prefixed binary frames"
```

OpenAI:
```bash
phaedra fuzz --target ./my_parser --llm-provider openai --llm-api-key sk-... \
  --description "parses length-prefixed binary frames"
```

Anthropic:
```bash
phaedra fuzz --target ./my_parser --llm-provider anthropic --llm-api-key sk-ant-... \
  --description "parses length-prefixed binary frames"
```

## Demo targets

Three built-in targets with real bugs you can watch Phaedra find:

| Demo | Bug class | Command |
|------|-----------|---------|
| `http` | Content-Length overflow -- `&body[0..content_length]` panics when header exceeds actual body | `phaedra fuzz --demo http` |
| `tlv` | TLV length prefix bounds -- `&data[offset..offset+length]` panics on truncated records | `phaedra fuzz --demo tlv` |
| `json` | Escape handling -- unescaped `\"` causes off-by-one slice panic in hand-rolled parser | `phaedra fuzz --demo json` |

All three find crashes within seconds on the first run.

## cargo-fuzz compatibility

If you already have a `fuzz/` directory with libFuzzer harnesses, point Phaedra at the compiled binary:

```bash
phaedra compat \
  --fuzz-target ./target/debug/fuzz_my_parser \
  --fuzz-dir ./fuzz \
  --description "parses length-prefixed binary frames"
```

Phaedra imports your existing `fuzz/corpus/` seeds, then drives the harness using its own mutation engine and crash triage.

## Live dashboard

```bash
phaedra fuzz --target ./my_parser --tui
```

Shows real-time corpus size, edge coverage, exec/s, crash count, and mutation strategy weights in a terminal dashboard.

## Building from source

```bash
git clone https://github.com/zaydmulani09/phaedra
cd phaedra
cargo build --release
./target/release/phaedra --help
```

Requires Rust stable 1.75+. No system dependencies beyond a C compiler for the SanCov runtime shim.

## License

MIT
