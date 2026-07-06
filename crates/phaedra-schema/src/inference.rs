use crate::types::{Field, FieldType, Schema};
use anyhow::{Context, Result};
use std::collections::HashMap;

pub fn schema_to_toml(schema: &Schema) -> Result<String> {
    toml::to_string(schema).context("Failed to serialize schema to TOML")
}

pub fn infer_schema(samples: &[Vec<u8>], name: &str) -> Schema {
    if samples.is_empty() {
        return Schema {
            name: name.to_string(),
            description: None,
            fields: vec![bytes_field("data", 16)],
        };
    }

    let mut fields: Vec<Field> = Vec::new();

    // H1: Magic bytes
    let magic_end = detect_magic(samples, &mut fields);

    // H2: Fixed vs variable length
    let all_same_len = samples.iter().all(|s| s.len() == samples[0].len());

    if all_same_len {
        let l = samples[0].len();
        if magic_end == 0 {
            // H5: Fixed layout (no magic)
            fixed_layout(l, &mut fields);
        } else {
            // Magic detected + fixed length: add remaining as Bytes
            let remaining = l.saturating_sub(magic_end);
            if remaining > 0 {
                fields.push(bytes_field("payload", remaining));
            }
        }
    } else {
        // H3: Length-prefix detection
        let lp_found = detect_lp(samples, magic_end, &mut fields);
        if !lp_found {
            // H4: Null terminator or raw bytes
            detect_null_or_bytes(samples, magic_end, &mut fields);
        }
    }

    // H6: Fallback — no heuristic produced any fields
    if fields.is_empty() {
        let med = median_len(samples);
        fields.push(bytes_field("data", if med == 0 { 16 } else { med }));
    }

    ensure_unique_names(&mut fields);

    Schema {
        name: name.to_string(),
        description: None,
        fields,
    }
}

fn detect_magic(samples: &[Vec<u8>], fields: &mut Vec<Field>) -> usize {
    let min_len = samples.iter().map(|s| s.len()).min().unwrap_or(0);
    let max_prefix = min_len.min(8);

    let mut common_len = 0;
    for i in 0..max_prefix {
        let b = samples[0][i];
        if samples.iter().all(|s| s[i] == b) {
            common_len += 1;
        } else {
            break;
        }
    }

    if common_len == 0 {
        return 0;
    }

    let prefix = &samples[0][..common_len];
    let hex: String = prefix.iter().map(|b| format!("{:02x}", b)).collect();

    fields.push(Field {
        name: "magic".to_string(),
        field_type: FieldType::Magic,
        length: Some(common_len),
        value: Some(hex),
        fields: vec![],
        mutable: false,
        description: None,
    });

    common_len
}

fn detect_lp(samples: &[Vec<u8>], magic_end: usize, fields: &mut Vec<Field>) -> bool {
    let total = samples.len();
    let threshold = |n: usize| n * 10 >= total * 7; // ≥70%

    // u8 prefix
    let u8_ok = samples
        .iter()
        .filter(|s| {
            s.len() > magic_end && {
                let plen = s[magic_end] as usize;
                s.len() == magic_end + 1 + plen
            }
        })
        .count();
    if threshold(u8_ok) {
        fields.push(lp_field("payload", FieldType::LpBytes8));
        return true;
    }

    // u16be prefix
    let u16_ok = samples
        .iter()
        .filter(|s| {
            s.len() >= magic_end + 2 && {
                let plen = u16::from_be_bytes([s[magic_end], s[magic_end + 1]]) as usize;
                s.len() == magic_end + 2 + plen
            }
        })
        .count();
    if threshold(u16_ok) {
        fields.push(lp_field("payload", FieldType::LpBytes16Be));
        return true;
    }

    // u32be prefix
    let u32_ok = samples
        .iter()
        .filter(|s| {
            s.len() >= magic_end + 4 && {
                let plen = u32::from_be_bytes([
                    s[magic_end],
                    s[magic_end + 1],
                    s[magic_end + 2],
                    s[magic_end + 3],
                ]) as usize;
                s.len() == magic_end + 4 + plen
            }
        })
        .count();
    if threshold(u32_ok) {
        fields.push(lp_field("payload", FieldType::LpBytes32Be));
        return true;
    }

    false
}

fn detect_null_or_bytes(samples: &[Vec<u8>], magic_end: usize, fields: &mut Vec<Field>) {
    let total = samples.len();

    let null_ok = samples
        .iter()
        .filter(|s| {
            if s.len() <= magic_end {
                return false;
            }
            let payload = &s[magic_end..];
            if let Some(null_pos) = payload.iter().position(|&b| b == 0) {
                null_pos == 0
                    || payload[..null_pos]
                        .iter()
                        .all(|&b| (0x20..=0x7e).contains(&b))
            } else {
                false
            }
        })
        .count();

    if null_ok * 10 >= total * 6 {
        // ≥60%
        fields.push(Field {
            name: "data".to_string(),
            field_type: FieldType::Cstring,
            length: None,
            value: None,
            fields: vec![],
            mutable: true,
            description: None,
        });
    } else {
        let med = median_payload_len(samples, magic_end);
        fields.push(bytes_field("data", if med == 0 { 1 } else { med }));
    }
}

fn fixed_layout(l: usize, fields: &mut Vec<Field>) {
    if l == 0 {
        return;
    }
    let mut offset = 0;

    fields.push(int_field("byte_0", FieldType::U8));
    offset += 1;

    if l >= 3 {
        fields.push(int_field("word_1", FieldType::U16Be));
        offset += 2;
    }

    if l >= 7 {
        fields.push(int_field("dword_3", FieldType::U32Be));
        offset += 4;
    }

    let remaining = l.saturating_sub(offset);
    if remaining > 0 {
        fields.push(bytes_field("payload", remaining));
    }
}

fn ensure_unique_names(fields: &mut Vec<Field>) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for f in fields.iter() {
        *counts.entry(f.name.clone()).or_insert(0) += 1;
    }
    let mut idx: HashMap<String, usize> = HashMap::new();
    for f in fields.iter_mut() {
        if counts[&f.name] > 1 {
            let i = idx.entry(f.name.clone()).or_insert(0);
            f.name = format!("{}_{}", f.name, i);
            *i += 1;
        }
    }
}

fn median_len(samples: &[Vec<u8>]) -> usize {
    if samples.is_empty() {
        return 0;
    }
    let mut lens: Vec<usize> = samples.iter().map(|s| s.len()).collect();
    lens.sort_unstable();
    lens[lens.len() / 2]
}

fn median_payload_len(samples: &[Vec<u8>], magic_end: usize) -> usize {
    let mut lens: Vec<usize> = samples
        .iter()
        .map(|s| s.len().saturating_sub(magic_end))
        .collect();
    if lens.is_empty() {
        return 0;
    }
    lens.sort_unstable();
    lens[lens.len() / 2]
}

fn bytes_field(name: &str, length: usize) -> Field {
    Field {
        name: name.to_string(),
        field_type: FieldType::Bytes,
        length: Some(length),
        value: None,
        fields: vec![],
        mutable: true,
        description: None,
    }
}

fn int_field(name: &str, ft: FieldType) -> Field {
    Field {
        name: name.to_string(),
        field_type: ft,
        length: None,
        value: None,
        fields: vec![],
        mutable: true,
        description: None,
    }
}

fn lp_field(name: &str, ft: FieldType) -> Field {
    Field {
        name: name.to_string(),
        field_type: ft,
        length: None,
        value: None,
        fields: vec![],
        mutable: true,
        description: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_schema;

    #[test]
    fn test_magic_detection_common_prefix() {
        let samples = vec![
            vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02],
            vec![0xDE, 0xAD, 0xBE, 0xEF, 0x03, 0x04],
            vec![0xDE, 0xAD, 0xBE, 0xEF, 0x05, 0x06],
            vec![0xDE, 0xAD, 0xBE, 0xEF, 0x07, 0x08],
            vec![0xDE, 0xAD, 0xBE, 0xEF, 0x09, 0x0A],
        ];
        let schema = infer_schema(&samples, "test");
        let magic = &schema.fields[0];
        assert_eq!(magic.field_type, FieldType::Magic);
        assert_eq!(magic.value.as_deref(), Some("deadbeef"));
        assert_eq!(magic.length, Some(4));
    }

    #[test]
    fn test_no_magic_when_prefix_differs() {
        let samples = vec![
            vec![0x01u8, 0x02, 0x03],
            vec![0x04u8, 0x05, 0x06],
            vec![0x07u8, 0x08, 0x09],
        ];
        let schema = infer_schema(&samples, "test");
        assert!(!schema.fields.iter().any(|f| f.field_type == FieldType::Magic));
    }

    #[test]
    fn test_fixed_length_produces_fields() {
        let samples: Vec<Vec<u8>> = (0..5)
            .map(|i| (0..8u8).map(|b| b.wrapping_add(i * 10)).collect())
            .collect();
        let schema = infer_schema(&samples, "test");
        // U8(1) + U16Be(2) + U32Be(4) + Bytes(1) = 8
        let total: usize = schema.fields.iter().map(|f| match f.field_type {
            FieldType::U8 => 1,
            FieldType::U16Be => 2,
            FieldType::U32Be => 4,
            FieldType::Bytes => f.length.unwrap_or(0),
            _ => 0,
        }).sum();
        assert_eq!(total, 8);
    }

    #[test]
    fn test_variable_length_lp_u8() {
        let samples = vec![
            vec![3u8, 0xAA, 0xBB, 0xCC],
            vec![4u8, 0x11, 0x22, 0x33, 0x44],
            vec![2u8, 0x55, 0x66],
        ];
        let schema = infer_schema(&samples, "test");
        assert!(schema.fields.iter().any(|f| f.field_type == FieldType::LpBytes8));
    }

    #[test]
    fn test_fallback_on_empty_samples() {
        let schema = infer_schema(&[], "test");
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.fields[0].field_type, FieldType::Bytes);
    }

    #[test]
    fn test_field_names_unique() {
        let samples: Vec<Vec<u8>> = (0..5)
            .map(|i| vec![i as u8, i as u8 + 1, i as u8 + 2, i as u8 + 3])
            .collect();
        let schema = infer_schema(&samples, "test");
        let names: std::collections::HashSet<&str> =
            schema.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names.len(), schema.fields.len());
    }

    #[test]
    fn test_schema_to_toml_roundtrip() {
        let samples = vec![
            vec![0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            vec![0x09u8, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10],
        ];
        let schema = infer_schema(&samples, "roundtrip");
        let toml_str = schema_to_toml(&schema).unwrap();
        let parsed = parse_schema(&toml_str).unwrap();
        assert_eq!(parsed.name, "roundtrip");
        assert_eq!(parsed.fields.len(), schema.fields.len());
    }

    #[test]
    fn test_infer_name_used() {
        let samples = vec![vec![0x01u8, 0x02]];
        let schema = infer_schema(&samples, "my_proto");
        assert_eq!(schema.name, "my_proto");
    }
}
