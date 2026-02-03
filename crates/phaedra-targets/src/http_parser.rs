use std::io::Read;

#[allow(dead_code)]
#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn parse_http(input: &[u8]) -> Result<HttpRequest, String> {
    let text = String::from_utf8_lossy(input);
    let text = text.as_ref();

    // Split headers from body on \r\n\r\n
    let (header_section, body_bytes) = if let Some(idx) = find_header_end(input) {
        let body = input[idx + 4..].to_vec();
        let headers_text = &text[..idx];
        (headers_text.to_string(), body)
    } else {
        (text.to_string(), vec![])
    };

    let mut lines = header_section.split("\r\n");

    let request_line = lines.next().ok_or("empty input")?;
    let parts: Vec<&str> = request_line.splitn(3, ' ').collect();
    if parts.len() != 3 {
        return Err(format!("bad request line: {request_line}"));
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();
    let version = parts[2].to_string();

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(colon) = line.find(": ") {
            let key = line[..colon].to_string();
            let val = line[colon + 2..].to_string();
            headers.push((key, val));
        }
    }

    // BUG: parse Content-Length and slice body without bounds check
    for (key, val) in &headers {
        if key.eq_ignore_ascii_case("Content-Length") {
            let content_length: usize = val.trim().parse().map_err(|_| "bad Content-Length")?;
            // This panics when content_length > body_bytes.len()
            let _body_slice = &body_bytes[0..content_length];
            break;
        }
    }

    Ok(HttpRequest {
        method,
        path,
        version,
        headers,
        body: body_bytes,
    })
}

fn find_header_end(input: &[u8]) -> Option<usize> {
    input.windows(4).position(|w| w == b"\r\n\r\n")
}

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap_or(0);

    match parse_http(&input) {
        Ok(req) => {
            println!(
                "OK: method={} path={} headers={}",
                req.method,
                req.path,
                req.headers.len()
            );
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
    fn test_valid_http_request() {
        let input = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\n\r\n";
        let req = parse_http(input).expect("should parse");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/index.html");
        assert_eq!(req.headers.len(), 2);
    }

    #[test]
    fn test_content_length_overflow_panics() {
        // Content-Length: 999 but body is empty — triggers index-out-of-bounds panic
        let input = b"POST /upload HTTP/1.1\r\nContent-Length: 999\r\n\r\n";
        let result = std::panic::catch_unwind(|| parse_http(input));
        assert!(result.is_err(), "should panic on Content-Length overflow");
    }
}
