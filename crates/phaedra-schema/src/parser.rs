use crate::types::{Field, FieldType, Schema};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;

pub fn load_schema(path: &std::path::Path) -> Result<Schema> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read schema file: {}", path.display()))?;
    parse_schema(&content)
}

pub fn parse_schema(toml: &str) -> Result<Schema> {
    let schema: Schema = toml::from_str(toml).context("Failed to parse schema TOML")?;
    validate_schema(&schema)?;
    Ok(schema)
}

pub fn validate_schema(schema: &Schema) -> Result<()> {
    if schema.name.is_empty() {
        bail!("Schema name must not be empty");
    }
    if schema.fields.is_empty() {
        bail!("Schema '{}' must have at least one field", schema.name);
    }
    validate_fields(&schema.fields, &schema.name)
}

fn validate_fields(fields: &[Field], ctx: &str) -> Result<()> {
    let mut seen = HashSet::new();
    for field in fields {
        if field.name.is_empty() {
            bail!("[{}] field has empty name", ctx);
        }
        if !seen.insert(field.name.clone()) {
            bail!("[{}] duplicate field name '{}'", ctx, field.name);
        }
        validate_field(field, ctx)?;
    }
    Ok(())
}

fn validate_field(field: &Field, ctx: &str) -> Result<()> {
    let loc = format!("{}/{}", ctx, field.name);
    match field.field_type {
        FieldType::Bytes | FieldType::Padding => {
            match field.length {
                None => bail!("[{}] field type '{:?}' requires 'length'", loc, field.field_type),
                Some(0) => bail!("[{}] field type '{:?}' requires length > 0", loc, field.field_type),
                _ => {}
            }
        }
        FieldType::Magic => {
            match field.length {
                None => bail!("[{}] 'magic' field requires 'length'", loc),
                Some(0) => bail!("[{}] 'magic' field requires length > 0", loc),
                Some(len) => {
                    match &field.value {
                        None => bail!("[{}] 'magic' field requires 'value'", loc),
                        Some(hex) => {
                            if hex.len() % 2 != 0 {
                                bail!("[{}] 'magic' value '{}' has odd hex length", loc, hex);
                            }
                            if hex.chars().any(|c| !c.is_ascii_hexdigit()) {
                                bail!("[{}] 'magic' value '{}' contains non-hex chars", loc, hex);
                            }
                            let decoded_len = hex.len() / 2;
                            if decoded_len != len {
                                bail!(
                                    "[{}] 'magic' value decodes to {} bytes but length={} (must match)",
                                    loc, decoded_len, len
                                );
                            }
                        }
                    }
                }
            }
        }
        FieldType::Repeated => {
            if field.length.is_none() {
                bail!("[{}] 'repeated' field requires 'length' (repeat count)", loc);
            }
            if field.fields.is_empty() {
                bail!("[{}] 'repeated' field requires at least one sub-field", loc);
            }
            validate_fields(&field.fields, &loc)?;
        }
        ref ft if ft.is_integer() => {
            if field.length.is_some() {
                bail!(
                    "[{}] integer field '{:?}' must not have 'length' (size is implied by type)",
                    loc, field.field_type
                );
            }
        }
        _ => {}
    }
    Ok(())
}

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0))
        .collect()
}

pub(crate) fn hex_decode(s: &str) -> Vec<u8> {
    decode_hex(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SCHEMA: &str = r#"
name = "test_proto"
description = "A test protocol"

[[fields]]
name = "version"
type = "u8"

[[fields]]
name = "payload"
type = "lp_bytes8"
"#;

    #[test]
    fn test_parse_valid_schema() {
        let schema = parse_schema(VALID_SCHEMA).unwrap();
        assert_eq!(schema.name, "test_proto");
        assert_eq!(schema.fields.len(), 2);
    }

    #[test]
    fn test_empty_name_rejected() {
        let toml = r#"name = ""
[[fields]]
name = "x"
type = "u8"
"#;
        assert!(parse_schema(toml).is_err());
    }

    #[test]
    fn test_no_fields_rejected() {
        let toml = r#"name = "x"
fields = []
"#;
        assert!(parse_schema(toml).is_err());
    }

    #[test]
    fn test_bytes_without_length_rejected() {
        let toml = r#"name = "x"
[[fields]]
name = "data"
type = "bytes"
"#;
        assert!(parse_schema(toml).is_err());
    }

    #[test]
    fn test_magic_without_value_rejected() {
        let toml = r#"name = "x"
[[fields]]
name = "hdr"
type = "magic"
length = 4
"#;
        assert!(parse_schema(toml).is_err());
    }

    #[test]
    fn test_duplicate_field_names_rejected() {
        let toml = r#"name = "x"
[[fields]]
name = "foo"
type = "u8"
[[fields]]
name = "foo"
type = "u8"
"#;
        assert!(parse_schema(toml).is_err());
    }

    #[test]
    fn test_example_tlv_schema_loads() {
        let toml = std::fs::read_to_string("../../examples/binary_tlv.schema.toml").unwrap();
        parse_schema(&toml).unwrap();
    }

    #[test]
    fn test_mutable_defaults_true() {
        let schema = parse_schema(VALID_SCHEMA).unwrap();
        assert!(schema.fields[0].mutable);
    }
}
