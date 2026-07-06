//! Demo fuzzing targets with intentional bugs for testing Phaedra.
//!
//! Three binary targets are included: `phaedra-target-http` parses a minimal HTTP/1.1 request and
//! panics when the `Content-Length` header value exceeds the actual body length (`&body[0..cl]` out of
//! bounds); `phaedra-target-tlv` parses a PHDR-magic TLV frame and panics when a record's u16be length
//! field extends past the end of the buffer; `phaedra-target-json` uses a hand-rolled JSON object parser
//! that ignores escape sequences, producing an incorrect slice index on inputs containing `\"` and
//! panicking on crafted strings. All three are found by Phaedra within seconds using `--demo http|tlv|json`.
pub const HTTP_TARGET_NAME: &str = "phaedra-target-http";
pub const TLV_TARGET_NAME: &str = "phaedra-target-tlv";
pub const JSON_TARGET_NAME: &str = "phaedra-target-json";
