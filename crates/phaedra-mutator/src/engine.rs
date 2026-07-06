use rand::distributions::WeightedIndex;
use rand::prelude::*;

use crate::strategies;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Strategy {
    BitFlip,
    ByteSubstitute,
    ByteInsert,
    ByteDelete,
    BlockFlip,
    BlockSubstitute,
    BlockInsert,
    BlockDelete,
    InterestingByte,
    InterestingU16,
    InterestingU32,
    RepeatByte,
    Arithmetic,
    Splice,
    TokenInsert,
    TokenReplace,
    Recombine,
    Havoc,
}

impl Strategy {
    pub fn all() -> &'static [Strategy] {
        &[
            Strategy::BitFlip,
            Strategy::ByteSubstitute,
            Strategy::ByteInsert,
            Strategy::ByteDelete,
            Strategy::BlockFlip,
            Strategy::BlockSubstitute,
            Strategy::BlockInsert,
            Strategy::BlockDelete,
            Strategy::InterestingByte,
            Strategy::InterestingU16,
            Strategy::InterestingU32,
            Strategy::RepeatByte,
            Strategy::Arithmetic,
            Strategy::Splice,
            Strategy::TokenInsert,
            Strategy::TokenReplace,
            Strategy::Recombine,
            Strategy::Havoc,
        ]
    }

    fn index(self) -> usize {
        Self::all().iter().position(|&s| s == self).unwrap()
    }
}

pub struct MutationEngine {
    weights: Vec<f64>,
    weights_dirty: bool,
    cached_dist: Option<WeightedIndex<f64>>,
    rng: rand::rngs::SmallRng,
    /// Token dictionary — byte patterns known to be meaningful to the target.
    tokens: Vec<Vec<u8>>,
    decay_counter: u64,
    decay_interval: u64,
    decay_rate: f64,
}

impl MutationEngine {
    pub fn new() -> Self {
        Self {
            weights: vec![1.0; Strategy::all().len()],
            weights_dirty: true,
            cached_dist: None,
            rng: rand::rngs::SmallRng::from_entropy(),
            tokens: Vec::new(),
            decay_counter: 0,
            decay_interval: 500,
            decay_rate: 0.05,
        }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            weights: vec![1.0; Strategy::all().len()],
            weights_dirty: true,
            cached_dist: None,
            rng: rand::rngs::SmallRng::seed_from_u64(seed),
            tokens: Vec::new(),
            decay_counter: 0,
            decay_interval: 500,
            decay_rate: 0.05,
        }
    }

    pub fn add_token(&mut self, token: Vec<u8>) {
        self.tokens.push(token);
    }

    pub fn add_tokens(&mut self, tokens: impl IntoIterator<Item = Vec<u8>>) {
        self.tokens.extend(tokens);
    }

    pub fn tokens(&self) -> &[Vec<u8>] {
        &self.tokens
    }

    pub fn set_decay(&mut self, interval: u64, rate: f64) {
        self.decay_interval = interval;
        self.decay_rate = rate;
    }

    pub fn mutate(&mut self, input: &[u8], corpus: &[Vec<u8>]) -> (Vec<u8>, Strategy) {
        if self.weights_dirty {
            self.cached_dist = Some(WeightedIndex::new(&self.weights).unwrap());
            self.weights_dirty = false;
        }
        let idx = self.cached_dist.as_ref().unwrap().sample(&mut self.rng);
        let mut strategy = Strategy::all()[idx];

        if strategy == Strategy::Splice && corpus.is_empty() {
            let non_splice: Vec<Strategy> = Strategy::all()
                .iter()
                .copied()
                .filter(|&s| s != Strategy::Splice)
                .collect();
            strategy = *non_splice.choose(&mut self.rng).unwrap();
        }

        let output = self.apply(input, corpus, strategy);

        self.decay_counter += 1;
        if self.decay_counter.is_multiple_of(self.decay_interval) {
            for w in &mut self.weights {
                *w = (*w - 1.0) * (1.0 - self.decay_rate) + 1.0;
                if *w < 1.0 {
                    *w = 1.0;
                }
            }
            self.weights_dirty = true;
        }

        (output, strategy)
    }

    fn apply(&mut self, input: &[u8], corpus: &[Vec<u8>], strategy: Strategy) -> Vec<u8> {
        match strategy {
            Strategy::BitFlip => strategies::bit_flip(input, &mut self.rng),
            Strategy::ByteSubstitute => strategies::byte_substitute(input, &mut self.rng),
            Strategy::ByteInsert => strategies::byte_insert(input, &mut self.rng),
            Strategy::ByteDelete => strategies::byte_delete(input, &mut self.rng),
            Strategy::BlockFlip => strategies::block_flip(input, &mut self.rng),
            Strategy::BlockSubstitute => strategies::block_substitute(input, &mut self.rng),
            Strategy::BlockInsert => strategies::block_insert(input, &mut self.rng),
            Strategy::BlockDelete => strategies::block_delete(input, &mut self.rng),
            Strategy::InterestingByte => strategies::interesting_byte(input, &mut self.rng),
            Strategy::InterestingU16 => strategies::interesting_u16(input, &mut self.rng),
            Strategy::InterestingU32 => strategies::interesting_u32(input, &mut self.rng),
            Strategy::RepeatByte => strategies::repeat_byte(input, &mut self.rng),
            Strategy::Arithmetic => strategies::arithmetic(input, &mut self.rng),
            Strategy::Splice => {
                if corpus.is_empty() {
                    strategies::block_substitute(input, &mut self.rng)
                } else {
                    let other_idx = self.rng.gen_range(0..corpus.len());
                    strategies::splice(input, &corpus[other_idx], &mut self.rng)
                }
            }
            Strategy::TokenInsert => {
                let token_refs: Vec<&[u8]> = self.tokens.iter().map(|t| t.as_slice()).collect();
                strategies::token_insert(input, &token_refs, &mut self.rng)
            }
            Strategy::TokenReplace => {
                let token_refs: Vec<&[u8]> = self.tokens.iter().map(|t| t.as_slice()).collect();
                strategies::token_replace(input, &token_refs, &mut self.rng)
            }
            Strategy::Recombine => {
                if corpus.is_empty() {
                    strategies::block_substitute(input, &mut self.rng)
                } else {
                    let other_idx = self.rng.gen_range(0..corpus.len());
                    strategies::recombine(input, &corpus[other_idx], &mut self.rng)
                }
            }
            Strategy::Havoc => strategies::havoc(input, &mut self.rng),
        }
    }

    pub fn record_success(&mut self, s: Strategy) {
        self.weights[s.index()] += 1.0;
        self.weights_dirty = true;
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn best_strategy(&self) -> Strategy {
        let idx = self
            .weights
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        Strategy::all()[idx]
    }

    pub fn reset_weights(&mut self) {
        self.weights.iter_mut().for_each(|w| *w = 1.0);
        self.weights_dirty = true;
    }
}

impl Default for MutationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_with_seed_is_deterministic() {
        let input = b"hello world";
        let corpus = vec![b"foo".to_vec(), b"bar".to_vec()];
        let mut e1 = MutationEngine::with_seed(42);
        let mut e2 = MutationEngine::with_seed(42);
        for _ in 0..20 {
            let (out1, s1) = e1.mutate(input, &corpus);
            let (out2, s2) = e2.mutate(input, &corpus);
            assert_eq!(out1, out2);
            assert_eq!(s1, s2);
        }
    }

    #[test]
    fn test_record_success_increases_weight() {
        let mut engine = MutationEngine::with_seed(1);
        let initial = engine.weights()[0];
        engine.record_success(Strategy::BitFlip);
        assert!(engine.weights()[0] > initial);
    }

    #[test]
    fn test_best_strategy_after_success() {
        let mut engine = MutationEngine::with_seed(1);
        for _ in 0..20 {
            engine.record_success(Strategy::Arithmetic);
        }
        assert_eq!(engine.best_strategy(), Strategy::Arithmetic);
    }

    #[test]
    fn test_mutate_empty_corpus_no_splice() {
        let mut engine = MutationEngine::with_seed(99);
        for _ in 0..50 {
            let (out, _strategy) = engine.mutate(b"test", &[]);
            let _ = out; // must not panic
        }
    }

    #[test]
    fn test_reset_weights() {
        let mut engine = MutationEngine::with_seed(1);
        engine.record_success(Strategy::BitFlip);
        engine.record_success(Strategy::BitFlip);
        engine.reset_weights();
        assert!(engine.weights().iter().all(|&w| (w - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_all_returns_18_strategies() {
        assert_eq!(Strategy::all().len(), 18);
    }

    #[test]
    fn test_add_token_and_retrieve() {
        let mut engine = MutationEngine::with_seed(7);
        assert!(engine.tokens().is_empty());
        engine.add_token(b"GET".to_vec());
        engine.add_token(b"POST".to_vec());
        assert_eq!(engine.tokens().len(), 2);
        assert_eq!(engine.tokens()[0], b"GET");
        assert_eq!(engine.tokens()[1], b"POST");
    }

    #[test]
    fn test_token_mutate_uses_dictionary() {
        let mut engine = MutationEngine::with_seed(42);
        engine.add_token(b"FUZZ".to_vec());
        // Bias heavily toward TokenInsert by recording many successes on it
        engine.reset_weights();
        for _ in 0..10000 {
            engine.record_success(Strategy::TokenInsert);
        }
        let mut found = false;
        for _ in 0..50 {
            let (out, _) = engine.mutate(b"hello", &[]);
            if out.windows(4).any(|w| w == b"FUZZ") {
                found = true;
                break;
            }
        }
        assert!(found, "TokenInsert should produce output containing FUZZ");
    }

    #[test]
    fn test_weight_decay_reduces_high_weight() {
        let mut engine = MutationEngine::with_seed(1);
        engine.set_decay(1, 0.05);
        for _ in 0..10 {
            engine.record_success(Strategy::BitFlip);
        }
        let bit_flip_idx = Strategy::all()
            .iter()
            .position(|&s| s == Strategy::BitFlip)
            .unwrap();
        let before = engine.weights()[bit_flip_idx];
        assert!((before - 11.0).abs() < f64::EPSILON);
        // one mutate() call → decay_counter = 1, 1 % 1 = 0 → decay fires
        engine.mutate(b"test", &[]);
        let after = engine.weights()[bit_flip_idx];
        assert!(after < before, "weight should have decayed toward 1.0");
    }
}
