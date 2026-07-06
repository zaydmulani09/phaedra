use anyhow::Result;

#[derive(Debug, Clone)]
pub struct GeneratedSeed {
    pub label: String,
    pub data: Vec<u8>,
    pub note: String,
}

pub fn parse_llm_response(response: &str) -> Vec<GeneratedSeed> {
    // Strategy 1: whole response is a JSON array
    if let Some(seeds) = try_parse_array(response) {
        tracing::debug!("parse_llm_response: strategy 1 extracted {} seeds", seeds.len());
        return seeds;
    }

    // Strategy 2: find first '[' and last ']' and parse substring
    if let (Some(start), Some(end)) = (response.find('['), response.rfind(']')) {
        if start < end {
            let slice = &response[start..=end];
            if let Some(seeds) = try_parse_array(slice) {
                tracing::debug!("parse_llm_response: strategy 2 extracted {} seeds", seeds.len());
                return seeds;
            }
        }
    }

    // Strategy 3: scan for top-level {...} objects
    let objects = extract_objects(response);
    let seeds: Vec<GeneratedSeed> = objects
        .iter()
        .filter_map(|obj| {
            let v: serde_json::Value = serde_json::from_str(obj).ok()?;
            seed_from_value(&v)
        })
        .collect();

    tracing::debug!("parse_llm_response: strategy 3 extracted {} seeds", seeds.len());
    if seeds.is_empty() {
        tracing::warn!("parse_llm_response: all strategies failed, returning empty");
    }
    seeds
}

fn try_parse_array(s: &str) -> Option<Vec<GeneratedSeed>> {
    let value: serde_json::Value = serde_json::from_str(s.trim()).ok()?;
    let arr = value.as_array()?;
    let seeds: Vec<GeneratedSeed> = arr.iter().filter_map(seed_from_value).collect();
    Some(seeds)
}

fn seed_from_value(v: &serde_json::Value) -> Option<GeneratedSeed> {
    let hex = v.get("hex")?.as_str()?;
    let data = if hex.is_empty() {
        vec![]
    } else {
        let bytes = decode_hex(hex);
        if bytes.is_empty() {
            return None; // non-empty hex but invalid
        }
        bytes
    };
    let label = v.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string();
    let note = v.get("note").and_then(|n| n.as_str()).unwrap_or("").to_string();
    Some(GeneratedSeed { label, data, note })
}

fn extract_objects(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 0usize;
            while i < bytes.len() {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            result.push(s[start..=i].to_string());
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    result
}

pub(crate) fn decode_hex(hex: &str) -> Vec<u8> {
    if hex.is_empty() {
        return vec![];
    }
    if !hex.len().is_multiple_of(2) {
        return vec![];
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_hex_valid() {
        assert_eq!(decode_hex("deadbeef"), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_decode_hex_empty() {
        assert_eq!(decode_hex(""), Vec::<u8>::new());
    }

    #[test]
    fn test_decode_hex_odd_length() {
        assert_eq!(decode_hex("abc"), Vec::<u8>::new());
    }

    #[test]
    fn test_decode_hex_invalid_chars() {
        assert_eq!(decode_hex("zzzz"), Vec::<u8>::new());
    }

    #[test]
    fn test_parse_valid_json_array() {
        let response = r#"[
            {"label": "empty", "hex": "", "note": "empty input"},
            {"label": "hello", "hex": "68656c6c6f", "note": "ascii hello"}
        ]"#;
        let seeds = parse_llm_response(response);
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0].data, Vec::<u8>::new());
        assert_eq!(seeds[1].data, b"hello");
    }

    #[test]
    fn test_parse_json_with_preamble() {
        let response = r#"Here are your seeds:
[
  {"label": "test", "hex": "41424344", "note": "ABCD"}
]"#;
        let seeds = parse_llm_response(response);
        assert!(!seeds.is_empty());
        assert_eq!(seeds[0].data, b"ABCD");
    }

    #[test]
    fn test_parse_completely_invalid() {
        let seeds = parse_llm_response("this is not json at all lol");
        assert_eq!(seeds.len(), 0);
    }

    #[test]
    fn test_parse_partial_valid() {
        let response = r#"[
            {"label": "ok", "hex": "ff00", "note": "valid"},
            {"label": "bad", "hex": "ZZZZ", "note": "invalid hex"}
        ]"#;
        let seeds = parse_llm_response(response);
        assert!(seeds.iter().any(|s| s.data == vec![0xff, 0x00]));
    }

    #[test]
    fn test_parse_missing_note_field() {
        let response = r#"[{"label": "x", "hex": "01"}]"#;
        let seeds = parse_llm_response(response);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].note, "");
    }
}
