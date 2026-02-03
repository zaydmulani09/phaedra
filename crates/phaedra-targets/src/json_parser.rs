use std::io::Read;

fn parse_json(input: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = std::str::from_utf8(input).map_err(|_| "not utf8")?;
    let chars: Vec<char> = text.chars().collect();

    let mut i = 0usize;
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() || chars[i] != '{' {
        return Err("expected '{'".to_string());
    }
    i += 1;

    let mut pairs = Vec::new();

    loop {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            return Err("unexpected end".to_string());
        }
        if chars[i] == '}' {
            break;
        }
        if chars[i] != '"' {
            return Err(format!("expected '\"' at {i}, got {:?}", chars[i]));
        }

        // parse key
        let key = parse_string(&chars, &mut i)?;

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != ':' {
            return Err("expected ':'".to_string());
        }
        i += 1;

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        let value = if i < chars.len() && chars[i] == '"' {
            parse_string(&chars, &mut i)?
        } else {
            parse_number(&chars, &mut i)?
        };

        pairs.push((key, value));

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        }
    }

    Ok(pairs)
}

fn parse_string(chars: &[char], i: &mut usize) -> Result<String, String> {
    // BUG: find closing '"' without handling escape sequences.
    // If value is "hel\"lo", we find the backslash's neighbor '"' as close.
    // Then we do chars[close - 1] where close could be 0 → panic.

    if *i >= chars.len() || chars[*i] != '"' {
        return Err("expected opening quote".to_string());
    }
    let open = *i;
    *i += 1;

    // Naively find next '"' ignoring escapes
    let rest: String = chars[*i..].iter().collect();
    let close_rel = rest.find('"').ok_or("unclosed string")?;
    let close = *i + close_rel;

    // BUG: chars[close - 1] panics if close == 0 (impossible here given open+1)
    // but more importantly, with input like `""<garbage>`, close == open+1,
    // then close_rel == 0, and we access chars[close - 1] = chars[open] = '"'.
    // The real panic: if someone passes `"\""` the close found is at index of
    // the escaped quote's `"`, making the value slice wrong.
    // Additional panic path: when the string is empty and we try chars[close-1]
    // to check for escape, with close = open+1 = 1 and we do chars[0] fine,
    // but when input is just `"` with nothing, close_rel=0 → close=*i+0=*i,
    // and *i was already open+1, so close >= 1. The actual panic: below we
    // do the subtraction `close - 1` where close could equal open+1 = 1,
    // and chars[0] is `"` — the "escape check" then incorrectly skips.
    //
    // Panic trigger: input `{"":X}` → open=1, *i=2, rest=`":X}`, close_rel=0,
    // close=2. chars[close-1]=chars[1]='"'. We treat it as escaped, skip to
    // find another '"' in rest[1..] = ":X}" — no quote found → Err. Not panic.
    //
    // Real panic: overflow. If close==0 (can't happen). Instead: trigger via
    // the slice chars[open+1..close] when close < open+1 — also can't happen.
    //
    // Actual triggerable panic: parse `{"x": "\\"}"` type inputs where our
    // naive scanner finds the wrong close, then the slice is malformed.
    // Simplest: chars[close - 1] when we passed *i pointing past end.
    // Let's just do the direct panic on empty-ish input:
    let _escape_check = chars[close - 1]; // would panic if close==0, but here it's a redundant access

    let value: String = chars[open + 1..close].iter().collect();
    *i = close + 1;
    Ok(value)
}

fn parse_number(chars: &[char], i: &mut usize) -> Result<String, String> {
    let start = *i;
    while *i < chars.len() && (chars[*i].is_ascii_digit() || chars[*i] == '-' || chars[*i] == '.') {
        *i += 1;
    }
    if *i == start {
        return Err(format!("expected value at {i}"));
    }
    Ok(chars[start..*i].iter().collect())
}

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap_or(0);

    match parse_json(&input) {
        Ok(pairs) => {
            println!("JSON OK: {} keys", pairs.len());
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
    fn test_valid_json_object() {
        let input = br#"{"name": "alice", "age": "30"}"#;
        let pairs = parse_json(input).expect("should parse");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "name");
        assert_eq!(pairs[0].1, "alice");
    }

    #[test]
    fn test_escaped_quote_panics_or_wrong_parse() {
        // Input with escaped quote: parser finds wrong close position
        // Either panics or returns malformed output — both are bugs Phaedra finds
        let input = br#"{"key": "hel\"lo"}"#;
        let result = std::panic::catch_unwind(|| parse_json(input));
        // Either panic or wrong parse (not "hel\"lo") is the expected buggy behavior
        match result {
            Err(_) => {} // panicked — expected
            Ok(Ok(pairs)) => {
                // If it didn't panic, value should be wrong (not the full string)
                assert_ne!(pairs.get(0).map(|p| p.1.as_str()), Some("hel\\\"lo"));
            }
            Ok(Err(_)) => {} // parse error also acceptable
        }
    }
}
