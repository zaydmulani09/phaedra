//! TOML schema DSL for field-level protocol mutation and schema inference.
//!
//! A `Schema` is a named list of `Field` values, each carrying a `FieldType` variant (15 total: `U8`,
//! `U16Be`, `U16Le`, `U32Be`, `U32Le`, `U64Be`, `U64Le`, `Bytes`, `Cstring`, `LpBytes8`,
//! `LpBytes16Be`, `LpBytes32Be`, `Magic`, `Padding`, `Repeated`) and a `mutable` flag. `schema_mutate`
//! walks the field list, applies type-appropriate mutations (e.g., arithmetic on integers, length
//! corruption on length-prefixed byte fields, random bytes on `Bytes` fields), and respects
//! `mutable = false` for magic constants. `schema_generate` produces a valid synthetic seed suitable
//! for bootstrapping. `infer_schema` analyzes a sample corpus with six heuristics -- magic prefix
//! detection, length-prefix detection (u8/u16be/u32be), null-terminator detection, fixed-layout
//! decomposition, and a median-length fallback -- to produce an initial schema without user input.

pub mod inference;
pub mod mutator;
pub mod parser;
pub mod types;

pub use inference::{infer_schema, schema_to_toml};
pub use mutator::{schema_generate, schema_mutate};
pub use parser::{load_schema, parse_schema};
pub use types::{Field, FieldType, Schema};
