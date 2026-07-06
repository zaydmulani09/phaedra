# Contributing

Bug reports and pull requests are welcome.

## Development setup

```bash
cargo build --workspace
cargo test --workspace
```

## Adding a mutation strategy

1. Add the function to `crates/phaedra-mutator/src/strategies.rs`
2. Add the variant to `Strategy` in `engine.rs` and update `Strategy::all()`
3. Handle it in `MutationEngine::mutate`
4. Write at least 2 unit tests covering empty input and non-empty input

## Adding a schema field type

1. Add the variant to `FieldType` in `crates/phaedra-schema/src/types.rs`
2. Handle it in `schema_mutate` and `schema_generate` in `mutator.rs`
3. Add validation in `parser.rs` if the type has required fields

## Running benchmarks

```bash
cargo bench
```

## Commit convention

`feat:` `fix:` `perf:` `refactor:` `test:` `docs:` `chore:`
