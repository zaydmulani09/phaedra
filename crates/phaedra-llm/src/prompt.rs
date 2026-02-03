pub fn seed_generation_system() -> &'static str {
    r#"You are a fuzzing expert helping generate seed inputs for coverage-guided fuzzing.
Your job is to produce realistic, varied byte sequences that exercise different code paths
in the target program. Follow instructions precisely and output only what is asked."#
}

pub fn seed_generation_user(description: &str, count: usize) -> String {
    format!(
        r#"Target description: {description}

Generate exactly {count} seed inputs for fuzzing this target.
Each seed must be a realistic example of input the target would receive.
Include variety: valid inputs, boundary values, empty inputs, very long inputs,
inputs with special characters or binary bytes where appropriate.

Output format — respond with ONLY a JSON array, no explanation, no markdown:
[
  {{"label": "short description", "hex": "deadbeef", "note": "why this is interesting"}},
  ...
]

Rules:
- "hex" field must be a valid lowercase hex string (even number of characters)
- "hex" may be empty string "" for an empty input
- Each seed must be different
- No markdown, no code blocks, no explanation — pure JSON array only"#
    )
}

pub fn crash_analysis_user(description: &str, input_hex: &str, status: &str) -> String {
    format!(
        r#"Target description: {description}
Crash status: {status}
Crashing input (hex): {input_hex}

Analyze this fuzzing crash. Explain in 2-3 sentences:
1. What the input likely triggered
2. What class of bug this might be
3. What the developer should investigate

Be concise and technical. No fluff."#
    )
}
