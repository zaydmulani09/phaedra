use rand::Rng;

fn random_fallback(rng: &mut impl Rng) -> Vec<u8> {
    (0..4).map(|_| rng.gen()).collect()
}

pub fn bit_flip(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let mut result = input.to_vec();
    let idx = rng.gen_range(0..result.len());
    let bit = rng.gen_range(0u8..8);
    result[idx] ^= 1 << bit;
    result
}

pub fn byte_substitute(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let mut result = input.to_vec();
    let idx = rng.gen_range(0..result.len());
    result[idx] = rng.gen();
    result
}

pub fn byte_insert(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let pos = rng.gen_range(0..=input.len());
    let byte: u8 = rng.gen();
    let mut result = input.to_vec();
    result.insert(pos, byte);
    result
}

pub fn byte_delete(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.len() <= 1 {
        return input.to_vec();
    }
    let idx = rng.gen_range(0..input.len());
    let mut result = input.to_vec();
    result.remove(idx);
    result
}

pub fn block_flip(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let block_len = rng.gen_range(1..=8_usize.min(input.len()));
    let start = rng.gen_range(0..=(input.len() - block_len));
    let mut result = input.to_vec();
    for b in &mut result[start..start + block_len] {
        *b = !*b;
    }
    result
}

pub fn block_substitute(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let block_len = rng.gen_range(1..=8_usize.min(input.len()));
    let start = rng.gen_range(0..=(input.len() - block_len));
    let mut result = input.to_vec();
    for b in &mut result[start..start + block_len] {
        *b = rng.gen();
    }
    result
}

pub fn block_insert(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let count = rng.gen_range(1..=16_usize);
    let pos = rng.gen_range(0..=input.len());
    let mut result = input.to_vec();
    let bytes: Vec<u8> = (0..count).map(|_| rng.gen()).collect();
    result.splice(pos..pos, bytes);
    result
}

pub fn block_delete(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let mut block_len = rng.gen_range(1..=8_usize.min(input.len()));
    if block_len >= input.len() {
        block_len = 1;
    }
    let start = rng.gen_range(0..=(input.len() - block_len));
    let mut result = input.to_vec();
    result.drain(start..start + block_len);
    result
}

pub fn interesting_byte(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    const VALS: &[u8] = &[0x00, 0x01, 0x7f, 0x80, 0xff];
    if input.is_empty() {
        return vec![VALS[rng.gen_range(0..VALS.len())]];
    }
    let mut result = input.to_vec();
    let idx = rng.gen_range(0..result.len());
    result[idx] = VALS[rng.gen_range(0..VALS.len())];
    result
}

pub fn interesting_u16(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.len() < 2 {
        return interesting_byte(input, rng);
    }
    const VALS: &[u16] = &[0, 1, 255, 256, 32767, 32768, 65535];
    let val = VALS[rng.gen_range(0..VALS.len())];
    let idx = rng.gen_range(0..=(input.len() - 2));
    let mut result = input.to_vec();
    let bytes = val.to_le_bytes();
    result[idx] = bytes[0];
    result[idx + 1] = bytes[1];
    result
}

pub fn interesting_u32(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.len() < 4 {
        return interesting_u16(input, rng);
    }
    const VALS: &[u32] = &[0, 1, 255, 256, 65535, 65536, 2147483647, 2147483648, 4294967295];
    let val = VALS[rng.gen_range(0..VALS.len())];
    let idx = rng.gen_range(0..=(input.len() - 4));
    let mut result = input.to_vec();
    result[idx..idx + 4].copy_from_slice(&val.to_le_bytes());
    result
}

pub fn splice(input: &[u8], other: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if other.is_empty() {
        return block_substitute(input, rng);
    }
    let split_a = rng.gen_range(0..=input.len());
    let split_b = rng.gen_range(0..=other.len());
    let mut result = input[..split_a].to_vec();
    result.extend_from_slice(&other[split_b..]);
    result
}

pub fn repeat_byte(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let count = rng.gen_range(1..=16_usize);
    let byte: u8 = rng.gen();
    let pos = rng.gen_range(0..=input.len());
    let mut result = input.to_vec();
    let repeated: Vec<u8> = std::iter::repeat(byte).take(count).collect();
    result.splice(pos..pos, repeated);
    result
}

pub fn arithmetic(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    if input.is_empty() {
        return random_fallback(rng);
    }
    let mut result = input.to_vec();
    let idx = rng.gen_range(0..result.len());
    let delta = rng.gen_range(1u8..=35);
    if rng.gen_bool(0.5) {
        result[idx] = result[idx].wrapping_add(delta);
    } else {
        result[idx] = result[idx].wrapping_sub(delta);
    }
    result
}

pub fn token_insert(input: &[u8], tokens: &[&[u8]], rng: &mut impl Rng) -> Vec<u8> {
    if tokens.is_empty() {
        return block_insert(input, rng);
    }
    let token = tokens[rng.gen_range(0..tokens.len())];
    let pos = rng.gen_range(0..=input.len());
    let mut result = input.to_vec();
    result.splice(pos..pos, token.iter().copied());
    result
}

pub fn token_replace(input: &[u8], tokens: &[&[u8]], rng: &mut impl Rng) -> Vec<u8> {
    if tokens.is_empty() || input.is_empty() {
        return block_substitute(input, rng);
    }
    let token = tokens[rng.gen_range(0..tokens.len())];
    let pos = rng.gen_range(0..input.len());
    let available = input.len() - pos;
    let copy_len = token.len().min(available);
    let mut result = input.to_vec();
    result[pos..pos + copy_len].copy_from_slice(&token[..copy_len]);
    result
}

pub fn recombine(a: &[u8], b: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let mut best_len = 0usize;
    let mut best_a = 0usize;
    let mut best_b = 0usize;
    for i in 0..a.len() {
        for j in 0..b.len() {
            let mut len = 0;
            while i + len < a.len() && j + len < b.len() && a[i + len] == b[j + len] {
                len += 1;
            }
            if len >= 2 && len > best_len {
                best_len = len;
                best_a = i;
                best_b = j;
            }
        }
    }
    if best_len < 2 {
        return splice(a, b, rng);
    }
    let mut result = a[..best_a].to_vec();
    result.extend_from_slice(&a[best_a..best_a + best_len]);
    result.extend_from_slice(&b[best_b + best_len..]);
    result
}

pub fn havoc(input: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let n = rng.gen_range(2..=8usize);
    let mut current = input.to_vec();
    for _ in 0..n {
        let choice = rng.gen_range(0..13usize);
        current = match choice {
            0 => bit_flip(&current, rng),
            1 => byte_substitute(&current, rng),
            2 => byte_insert(&current, rng),
            3 => byte_delete(&current, rng),
            4 => block_flip(&current, rng),
            5 => block_substitute(&current, rng),
            6 => block_insert(&current, rng),
            7 => block_delete(&current, rng),
            8 => interesting_byte(&current, rng),
            9 => interesting_u16(&current, rng),
            10 => interesting_u32(&current, rng),
            11 => repeat_byte(&current, rng),
            12 => arithmetic(&current, rng),
            _ => unreachable!(),
        };
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn rng() -> rand::rngs::SmallRng {
        rand::rngs::SmallRng::seed_from_u64(0xdeadbeef)
    }

    const SMALL: &[u8] = b"hello world fuzz target test input bytes";
    const LARGE: &[u8] = &[0xABu8; 1000];

    // bit_flip
    #[test]
    fn bit_flip_empty() {
        let out = bit_flip(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn bit_flip_one_byte() {
        let out = bit_flip(&[0xFFu8], &mut rng());
        assert_eq!(out.len(), 1);
    }
    #[test]
    fn bit_flip_large() {
        let out = bit_flip(LARGE, &mut rng());
        assert_eq!(out.len(), LARGE.len());
    }

    // byte_substitute
    #[test]
    fn byte_substitute_empty() {
        let out = byte_substitute(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn byte_substitute_nonempty() {
        let out = byte_substitute(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }

    // byte_insert
    #[test]
    fn byte_insert_empty() {
        let out = byte_insert(&[], &mut rng());
        assert_eq!(out.len(), 1);
    }
    #[test]
    fn byte_insert_nonempty() {
        let out = byte_insert(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len() + 1);
    }

    // byte_delete
    #[test]
    fn byte_delete_empty() {
        let out = byte_delete(&[], &mut rng());
        assert_eq!(out.len(), 0);
    }
    #[test]
    fn byte_delete_one_byte() {
        let out = byte_delete(&[42u8], &mut rng());
        assert_eq!(out.len(), 1); // unchanged
    }
    #[test]
    fn byte_delete_nonempty() {
        let out = byte_delete(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len() - 1);
    }

    // block_flip
    #[test]
    fn block_flip_empty() {
        let out = block_flip(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn block_flip_nonempty() {
        let out = block_flip(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }

    // block_substitute
    #[test]
    fn block_substitute_empty() {
        let out = block_substitute(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn block_substitute_nonempty() {
        let out = block_substitute(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }

    // block_insert
    #[test]
    fn block_insert_empty() {
        let out = block_insert(&[], &mut rng());
        assert!(out.len() >= 1 && out.len() <= 16);
    }
    #[test]
    fn block_insert_nonempty() {
        let out = block_insert(SMALL, &mut rng());
        assert!(out.len() >= SMALL.len() + 1 && out.len() <= SMALL.len() + 16);
    }

    // block_delete
    #[test]
    fn block_delete_empty() {
        let out = block_delete(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn block_delete_one_byte() {
        let out = block_delete(&[1u8], &mut rng());
        // block_len = 1, >= len(1), set to 1 → deletes 1 byte → empty
        assert_eq!(out.len(), 0);
    }
    #[test]
    fn block_delete_nonempty() {
        let out = block_delete(LARGE, &mut rng());
        assert!(out.len() >= LARGE.len() - 8 && out.len() < LARGE.len());
    }

    // interesting_byte
    #[test]
    fn interesting_byte_empty() {
        let out = interesting_byte(&[], &mut rng());
        assert_eq!(out.len(), 1);
    }
    #[test]
    fn interesting_byte_nonempty() {
        let out = interesting_byte(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }

    // interesting_u16
    #[test]
    fn interesting_u16_empty() {
        let out = interesting_u16(&[], &mut rng());
        assert_eq!(out.len(), 1); // falls back to interesting_byte
    }
    #[test]
    fn interesting_u16_nonempty() {
        let out = interesting_u16(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }

    // interesting_u32
    #[test]
    fn interesting_u32_short() {
        let out = interesting_u32(&[1, 2], &mut rng()); // < 4 bytes, falls back
        assert!(out.len() >= 1 && out.len() <= 2);
    }
    #[test]
    fn interesting_u32_nonempty() {
        let out = interesting_u32(LARGE, &mut rng());
        assert_eq!(out.len(), LARGE.len());
    }

    // splice
    #[test]
    fn splice_empty_other_fallback() {
        let out = splice(SMALL, &[], &mut rng());
        // falls back to block_substitute → same length
        assert_eq!(out.len(), SMALL.len());
    }
    #[test]
    fn splice_nonempty() {
        let a = b"abcdefgh";
        let b = b"12345678";
        let out = splice(a, b, &mut rng());
        assert!(out.len() <= a.len() + b.len());
    }

    // repeat_byte
    #[test]
    fn repeat_byte_empty() {
        let out = repeat_byte(&[], &mut rng());
        assert!(out.len() >= 1 && out.len() <= 16);
    }
    #[test]
    fn repeat_byte_nonempty() {
        let out = repeat_byte(SMALL, &mut rng());
        assert!(out.len() >= SMALL.len() + 1 && out.len() <= SMALL.len() + 16);
    }

    // arithmetic
    #[test]
    fn arithmetic_empty() {
        let out = arithmetic(&[], &mut rng());
        assert_eq!(out.len(), 4);
    }
    #[test]
    fn arithmetic_nonempty() {
        let out = arithmetic(SMALL, &mut rng());
        assert_eq!(out.len(), SMALL.len());
    }
    #[test]
    fn arithmetic_wrapping() {
        // 0x00 - 1 wraps to 0xFF — must not panic
        let out = arithmetic(&[0x00u8], &mut rng());
        assert_eq!(out.len(), 1);
    }

    // token_insert
    #[test]
    fn test_token_insert_with_tokens() {
        let mut r = rng();
        let tokens: &[&[u8]] = &[b"GET", b"POST"];
        let out = token_insert(b"hello", tokens, &mut r);
        let contains = out.windows(3).any(|w| w == b"GET") || out.windows(4).any(|w| w == b"POST");
        assert!(contains, "output should contain one of the tokens");
    }

    #[test]
    fn test_token_insert_empty_tokens_no_panic() {
        let out = token_insert(b"hello", &[], &mut rng());
        assert!(!out.is_empty());
    }

    // token_replace
    #[test]
    fn test_token_replace_shrinks_long_token() {
        let input = b"abc";
        let long_token: &[u8] = b"TOOLONGTOKENVALUE";
        let tokens: &[&[u8]] = &[long_token];
        let out = token_replace(input, tokens, &mut rng());
        assert_eq!(out.len(), input.len(), "length must not change");
    }

    // recombine
    #[test]
    fn test_recombine_finds_common_substring() {
        let a = b"hello world";
        let b = b"say world bye";
        let out = recombine(a, b, &mut rng());
        let contains_world = out.windows(5).any(|w| w == b"world");
        assert!(contains_world, "output should contain common substring 'world'");
    }

    #[test]
    fn test_recombine_no_common_falls_back() {
        // no common substring >= 2 → splice fallback, must not panic
        let out = recombine(b"AAAA", b"ZZZZ", &mut rng());
        let _ = out;
    }

    // havoc
    #[test]
    fn test_havoc_chains_mutations() {
        let input = b"hello world";
        let mut r = rng();
        let mut same_count = 0usize;
        for _ in 0..20 {
            let out = havoc(input, &mut r);
            if out == input {
                same_count += 1;
            }
        }
        assert!(same_count <= 1, "havoc should almost always change input");
    }

    #[test]
    fn test_havoc_empty_input_no_panic() {
        let out = havoc(&[], &mut rng());
        let _ = out;
    }
}
