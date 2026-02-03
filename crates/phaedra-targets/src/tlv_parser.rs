use std::io::Read;

const MAGIC: [u8; 4] = [0x50, 0x48, 0x44, 0x52];

#[derive(Debug)]
struct TlvRecord {
    typ: u8,
    length: u16,
    value: Vec<u8>,
}

fn parse_tlv(input: &[u8]) -> Result<Vec<TlvRecord>, String> {
    if input.len() < 4 {
        return Err("too short for magic".to_string());
    }
    if input[0..4] != MAGIC {
        return Err(format!(
            "bad magic: {:02x} {:02x} {:02x} {:02x}",
            input[0], input[1], input[2], input[3]
        ));
    }

    let mut records = Vec::new();
    let mut offset = 4usize;

    while offset < input.len() {
        if offset >= input.len() {
            break;
        }
        let typ = input[offset];
        offset += 1;

        if offset + 2 > input.len() {
            return Err("truncated length field".to_string());
        }
        let length = u16::from_be_bytes([input[offset], input[offset + 1]]) as usize;
        offset += 2;

        // BUG: no bounds check — panics on truncated record
        let value = input[offset..offset + length].to_vec();
        offset += length;

        records.push(TlvRecord {
            typ,
            length: length as u16,
            value,
        });
    }

    Ok(records)
}

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap_or(0);

    match parse_tlv(&input) {
        Ok(records) => {
            for r in &records {
                let hex: String = r.value.iter().map(|b| format!("{b:02x}")).collect();
                println!("TLV: type={} len={} value={}", r.typ, r.length, hex);
            }
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("PARSE ERROR: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_tlv() {
        let mut input = vec![0x50, 0x48, 0x44, 0x52]; // magic
        input.push(0x01); // type
        input.extend_from_slice(&3u16.to_be_bytes()); // length = 3
        input.extend_from_slice(b"ABC"); // value
        let records = parse_tlv(&input).expect("should parse");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].typ, 1);
        assert_eq!(records[0].length, 3);
        assert_eq!(records[0].value, b"ABC");
    }

    #[test]
    fn test_truncated_value_panics() {
        // length=100 but only 3 bytes of value follow — should panic
        let mut input = vec![0x50, 0x48, 0x44, 0x52]; // magic
        input.push(0x02); // type
        input.extend_from_slice(&100u16.to_be_bytes()); // length = 100
        input.extend_from_slice(b"XY"); // only 2 bytes
        let result = std::panic::catch_unwind(|| parse_tlv(&input));
        assert!(result.is_err(), "should panic on truncated value");
    }
}
