use crate::parser::hex_decode;
use crate::types::{Field, FieldType, Schema};
use rand::Rng;

const INTERESTING_U8: &[u8] = &[0, 1, 127, 128, 255];
const INTERESTING_U16: &[u16] = &[0, 1, 0x7fff, 0x8000, 0xffff];
const INTERESTING_U32: &[u32] = &[0, 1, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff];

fn field_runtime_size(field: &Field, input: &[u8], offset: usize) -> usize {
    match field.field_type {
        FieldType::U8 => 1,
        FieldType::U16Be | FieldType::U16Le => 2,
        FieldType::U32Be | FieldType::U32Le => 4,
        FieldType::U64Be | FieldType::U64Le => 8,
        FieldType::Bytes | FieldType::Magic | FieldType::Padding => field.length.unwrap_or(0),
        FieldType::Cstring => {
            if offset < input.len() {
                input[offset..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|p| p + 1)
                    .unwrap_or(input.len() - offset)
            } else {
                1
            }
        }
        FieldType::LpBytes8 => {
            if offset < input.len() {
                1 + input[offset] as usize
            } else {
                1
            }
        }
        FieldType::LpBytes16Be => {
            if offset + 1 < input.len() {
                let len = u16::from_be_bytes([input[offset], input[offset + 1]]) as usize;
                2 + len
            } else {
                2
            }
        }
        FieldType::LpBytes32Be => {
            if offset + 3 < input.len() {
                let len = u32::from_be_bytes([
                    input[offset],
                    input[offset + 1],
                    input[offset + 2],
                    input[offset + 3],
                ]) as usize;
                4 + len
            } else {
                4
            }
        }
        FieldType::Repeated => {
            let count = field.length.unwrap_or(0);
            let mut total = 0usize;
            for _ in 0..count {
                for sub in &field.fields {
                    total += field_runtime_size(sub, input, offset + total);
                }
            }
            total
        }
    }
}

#[allow(clippy::ptr_arg)]
fn collect_mutable<'a>(
    input: &[u8],
    fields: &'a [Field],
    base: usize,
    out: &mut Vec<(usize, &'a Field)>,
) {
    let mut offset = base;
    for field in fields {
        let size = field_runtime_size(field, input, offset);
        if field.field_type == FieldType::Repeated {
            if field.mutable {
                let count = field.length.unwrap_or(0);
                let mut rep_offset = offset;
                for _ in 0..count {
                    collect_mutable(input, &field.fields, rep_offset, out);
                    let rep_size: usize = field
                        .fields
                        .iter()
                        .map(|f| field_runtime_size(f, input, rep_offset))
                        .sum();
                    rep_offset += rep_size;
                }
            }
        } else if field.mutable {
            out.push((offset, field));
        }
        offset += size;
    }
}

fn apply_mutation(buf: &mut Vec<u8>, offset: usize, field: &Field, rng: &mut impl Rng) {
    match field.field_type {
        FieldType::U8 => {
            if offset < buf.len() {
                buf[offset] = if rng.gen_bool(0.3) {
                    INTERESTING_U8[rng.gen_range(0..INTERESTING_U8.len())]
                } else {
                    rng.gen()
                };
            }
        }
        FieldType::U16Be => {
            if offset + 1 < buf.len() {
                let v: u16 = if rng.gen_bool(0.3) {
                    INTERESTING_U16[rng.gen_range(0..INTERESTING_U16.len())]
                } else {
                    rng.gen()
                };
                let bytes = v.to_be_bytes();
                buf[offset] = bytes[0];
                buf[offset + 1] = bytes[1];
            }
        }
        FieldType::U16Le => {
            if offset + 1 < buf.len() {
                let v: u16 = if rng.gen_bool(0.3) {
                    INTERESTING_U16[rng.gen_range(0..INTERESTING_U16.len())]
                } else {
                    rng.gen()
                };
                let bytes = v.to_le_bytes();
                buf[offset] = bytes[0];
                buf[offset + 1] = bytes[1];
            }
        }
        FieldType::U32Be => {
            if offset + 3 < buf.len() {
                let v: u32 = if rng.gen_bool(0.3) {
                    INTERESTING_U32[rng.gen_range(0..INTERESTING_U32.len())]
                } else {
                    rng.gen()
                };
                let bytes = v.to_be_bytes();
                buf[offset..offset + 4].copy_from_slice(&bytes);
            }
        }
        FieldType::U32Le => {
            if offset + 3 < buf.len() {
                let v: u32 = if rng.gen_bool(0.3) {
                    INTERESTING_U32[rng.gen_range(0..INTERESTING_U32.len())]
                } else {
                    rng.gen()
                };
                let bytes = v.to_le_bytes();
                buf[offset..offset + 4].copy_from_slice(&bytes);
            }
        }
        FieldType::U64Be => {
            if offset + 7 < buf.len() {
                let v: u64 = rng.gen();
                let bytes = v.to_be_bytes();
                buf[offset..offset + 8].copy_from_slice(&bytes);
            }
        }
        FieldType::U64Le => {
            if offset + 7 < buf.len() {
                let v: u64 = rng.gen();
                let bytes = v.to_le_bytes();
                buf[offset..offset + 8].copy_from_slice(&bytes);
            }
        }
        FieldType::Bytes => {
            let len = field.length.unwrap_or(0);
            if len == 0 || offset >= buf.len() {
                return;
            }
            let end = (offset + len).min(buf.len());
            match rng.gen_range(0u8..3) {
                0 => {
                    // flip a byte
                    let idx = rng.gen_range(offset..end);
                    buf[idx] = rng.gen();
                }
                1 if end > offset => {
                    // insert a byte
                    let idx = rng.gen_range(offset..end);
                    buf.insert(idx, rng.gen());
                }
                _ if end > offset + 1 => {
                    // delete a byte
                    let idx = rng.gen_range(offset..end);
                    buf.remove(idx);
                }
                _ => {
                    let idx = rng.gen_range(offset..end);
                    buf[idx] = rng.gen();
                }
            }
        }
        FieldType::Cstring => {
            let end = if offset < buf.len() {
                buf[offset..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|p| offset + p)
                    .unwrap_or(buf.len())
            } else {
                return;
            };
            match rng.gen_range(0u8..3) {
                0 if end > offset => {
                    // modify a content byte
                    let idx = rng.gen_range(offset..end);
                    buf[idx] = rng.gen::<u8>().max(1); // avoid null in content
                }
                1 if end < buf.len() => {
                    // remove null terminator (overread test)
                    buf[end] = rng.gen::<u8>().max(1);
                }
                _ if end > offset => {
                    // insert early null
                    let idx = rng.gen_range(offset..end);
                    buf[idx] = 0;
                }
                _ => {
                    if offset < buf.len() {
                        buf[offset] = rng.gen();
                    }
                }
            }
        }
        FieldType::LpBytes8 => {
            if offset >= buf.len() {
                return;
            }
            if rng.gen_bool(0.4) {
                // mutate length prefix
                buf[offset] = match rng.gen_range(0u8..3) {
                    0 => 0,
                    1 => 255,
                    _ => rng.gen(),
                };
            } else {
                // mutate payload byte
                let payload_len = buf[offset] as usize;
                let payload_start = offset + 1;
                let payload_end = (payload_start + payload_len).min(buf.len());
                if payload_end > payload_start {
                    let idx = rng.gen_range(payload_start..payload_end);
                    buf[idx] = rng.gen();
                }
            }
        }
        FieldType::LpBytes16Be => {
            if offset + 1 >= buf.len() {
                return;
            }
            if rng.gen_bool(0.4) {
                let v: u16 = match rng.gen_range(0u8..3) {
                    0 => 0,
                    1 => u16::MAX,
                    _ => rng.gen(),
                };
                let bytes = v.to_be_bytes();
                buf[offset] = bytes[0];
                buf[offset + 1] = bytes[1];
            } else {
                let payload_len =
                    u16::from_be_bytes([buf[offset], buf[offset + 1]]) as usize;
                let payload_start = offset + 2;
                let payload_end = (payload_start + payload_len).min(buf.len());
                if payload_end > payload_start {
                    let idx = rng.gen_range(payload_start..payload_end);
                    buf[idx] = rng.gen();
                }
            }
        }
        FieldType::LpBytes32Be => {
            if offset + 3 >= buf.len() {
                return;
            }
            if rng.gen_bool(0.4) {
                let v: u32 = match rng.gen_range(0u8..3) {
                    0 => 0,
                    1 => u32::MAX,
                    _ => rng.gen(),
                };
                let bytes = v.to_be_bytes();
                buf[offset..offset + 4].copy_from_slice(&bytes);
            } else {
                let payload_len = u32::from_be_bytes([
                    buf[offset],
                    buf[offset + 1],
                    buf[offset + 2],
                    buf[offset + 3],
                ]) as usize;
                let payload_start = offset + 4;
                let payload_end = (payload_start + payload_len).min(buf.len());
                if payload_end > payload_start {
                    let idx = rng.gen_range(payload_start..payload_end);
                    buf[idx] = rng.gen();
                }
            }
        }
        FieldType::Magic => {
            // mutable=true magic: flip a random bit
            let len = field.length.unwrap_or(0);
            if len == 0 || offset >= buf.len() {
                return;
            }
            let end = (offset + len).min(buf.len());
            let idx = rng.gen_range(offset..end);
            let bit: u8 = 1 << rng.gen_range(0u8..8);
            buf[idx] ^= bit;
        }
        FieldType::Padding => {
            let len = field.length.unwrap_or(0);
            if len == 0 || offset >= buf.len() {
                return;
            }
            let end = (offset + len).min(buf.len());
            let fill: u8 = if rng.gen_bool(0.5) { 0x00 } else { 0xff };
            buf[offset..end].fill(fill);
        }
        FieldType::Repeated => {
            // handled in collect_mutable by expanding sub-fields
        }
    }
}

fn schema_min_size(fields: &[Field]) -> usize {
    fields.iter().map(|f| f.field_type.min_size()).sum()
}

pub fn schema_mutate(input: &[u8], schema: &Schema, rng: &mut impl Rng) -> Vec<u8> {
    let min_size = schema_min_size(&schema.fields);
    let mut buf = input.to_vec();
    if buf.len() < min_size {
        buf.resize(min_size, 0);
    }

    let mut locations: Vec<(usize, &Field)> = Vec::new();
    collect_mutable(&buf, &schema.fields, 0, &mut locations);

    if locations.is_empty() {
        return buf;
    }

    let idx = rng.gen_range(0..locations.len());
    let (offset, field) = locations[idx];
    apply_mutation(&mut buf, offset, field, rng);
    buf
}

fn generate_fields(fields: &[Field], out: &mut Vec<u8>) {
    for field in fields {
        match field.field_type {
            FieldType::U8 => out.push(1),
            FieldType::U16Be => out.extend_from_slice(&1u16.to_be_bytes()),
            FieldType::U16Le => out.extend_from_slice(&1u16.to_le_bytes()),
            FieldType::U32Be => out.extend_from_slice(&1u32.to_be_bytes()),
            FieldType::U32Le => out.extend_from_slice(&1u32.to_le_bytes()),
            FieldType::U64Be => out.extend_from_slice(&1u64.to_be_bytes()),
            FieldType::U64Le => out.extend_from_slice(&1u64.to_le_bytes()),
            FieldType::Bytes => {
                let len = field.length.unwrap_or(0);
                out.extend(std::iter::repeat_n(0x41u8, len));
            }
            FieldType::Cstring => {
                out.extend_from_slice(b"test\0");
            }
            FieldType::LpBytes8 => {
                out.push(4);
                out.extend_from_slice(b"AAAA");
            }
            FieldType::LpBytes16Be => {
                out.extend_from_slice(&4u16.to_be_bytes());
                out.extend_from_slice(b"AAAA");
            }
            FieldType::LpBytes32Be => {
                out.extend_from_slice(&4u32.to_be_bytes());
                out.extend_from_slice(b"AAAA");
            }
            FieldType::Magic => {
                if let Some(ref hex) = field.value {
                    out.extend_from_slice(&hex_decode(hex));
                } else {
                    let len = field.length.unwrap_or(0);
                    out.extend(std::iter::repeat_n(0u8, len));
                }
            }
            FieldType::Padding => {
                let len = field.length.unwrap_or(0);
                out.extend(std::iter::repeat(0u8).take(len));
            }
            FieldType::Repeated => {
                let count = field.length.unwrap_or(0);
                for _ in 0..count {
                    generate_fields(&field.fields, out);
                }
            }
        }
    }
}

pub fn schema_generate(schema: &Schema, _rng: &mut impl Rng) -> Vec<u8> {
    let mut out = Vec::new();
    generate_fields(&schema.fields, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_schema;
    use rand::SeedableRng;

    fn rng() -> rand::rngs::SmallRng {
        rand::rngs::SmallRng::seed_from_u64(0xc0ffee)
    }

    const SIMPLE_SCHEMA: &str = r#"
name = "simple"
[[fields]]
name = "version"
type = "u8"
[[fields]]
name = "flags"
type = "u8"
[[fields]]
name = "length"
type = "u16_be"
"#;

    #[test]
    fn test_schema_generate_produces_correct_size() {
        let schema = parse_schema(SIMPLE_SCHEMA).unwrap();
        let mut r = rng();
        let out = schema_generate(&schema, &mut r);
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn test_schema_mutate_same_length_fixed_schema() {
        let schema = parse_schema(SIMPLE_SCHEMA).unwrap();
        let mut r = rng();
        let input = schema_generate(&schema, &mut r);
        let mutated = schema_mutate(&input, &schema, &mut r);
        assert_eq!(mutated.len(), input.len());
    }

    #[test]
    fn test_schema_mutate_short_input_padded() {
        let schema = parse_schema(SIMPLE_SCHEMA).unwrap();
        let mut r = rng();
        let mutated = schema_mutate(&[0x01], &schema, &mut r);
        assert!(!mutated.is_empty());
    }

    #[test]
    fn test_schema_generate_magic_correct() {
        let toml = r#"
name = "magic_test"
[[fields]]
name = "header"
type = "magic"
length = 4
value = "deadbeef"
"#;
        let schema = parse_schema(toml).unwrap();
        let mut r = rng();
        let out = schema_generate(&schema, &mut r);
        assert_eq!(&out[0..4], &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_schema_mutate_no_panic_on_empty_input() {
        let schema = parse_schema(SIMPLE_SCHEMA).unwrap();
        let mut r = rng();
        let _out = schema_mutate(&[], &schema, &mut r);
    }

    #[test]
    fn test_immutable_field_not_changed() {
        let toml = r#"
name = "immut_test"
[[fields]]
name = "magic"
type = "magic"
length = 2
value = "cafe"
mutable = false
[[fields]]
name = "version"
type = "u8"
"#;
        let schema = parse_schema(toml).unwrap();
        let mut r = rand::rngs::SmallRng::seed_from_u64(0);
        let input = schema_generate(&schema, &mut r);
        for i in 0..100u64 {
            let mut r2 = rand::rngs::SmallRng::seed_from_u64(i);
            let mutated = schema_mutate(&input, &schema, &mut r2);
            assert_eq!(
                &mutated[0..2],
                &[0xca, 0xfe],
                "magic bytes changed on iteration {i}"
            );
        }
    }
}
