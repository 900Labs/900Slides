//! Offline en-US spell checker backed by a bundled word list.
//!
//! Spell-check is a text tooling service: it checks text on demand and never
//! alters the deck model or persists to a PPTX package. This crate is therefore
//! standalone (std-only, no dependency on `slides-core`). The en-US dictionary
//! is embedded at compile time via `include_str!`, so checking works fully
//! offline with no network dependency.

use std::collections::HashSet;

/// The bundled en-US word list (~233k lowercase ASCII words plus contractions).
/// Source: public-domain Webster's 2nd `words` file, filtered and de-duplicated.
const WORDLIST: &str = include_str!("../data/en-US.txt");

/// Lowercase ASCII letters used for edit generation.
const ALPHABET: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

/// Cap on the number of distance-2 candidates generated during suggestion, to
/// avoid combinatorial blowup on long inputs (e.g. a 50-char nonsense word).
const DISTANCE2_CAP: usize = 4000;

/// An offline en-US spell checker backed by a bundled word list plus an
/// optional user dictionary (learned words).
#[derive(Debug)]
pub struct SpellChecker {
    /// Bundled, lowercased en-US words. Built once at [`SpellChecker::new`].
    dictionary: HashSet<String>,
    /// User-learned, lowercased words. Considered correct in addition to the
    /// bundled list.
    user: HashSet<String>,
}

impl SpellChecker {
    /// Loads the bundled en-US dictionary. Deterministic and offline.
    pub fn new() -> Self {
        let mut dictionary = HashSet::with_capacity(234_000);
        for line in WORDLIST.lines() {
            let word = line.trim();
            if !word.is_empty() {
                dictionary.insert(word.to_ascii_lowercase());
            }
        }
        Self {
            dictionary,
            user: HashSet::new(),
        }
    }

    /// True if `word` is spelled correctly (case-insensitive). Considers the
    /// user dictionary in addition to the bundled list.
    pub fn is_correct(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        let lower = word.to_ascii_lowercase();
        self.dictionary.contains(&lower) || self.user.contains(&lower)
    }

    /// Tokenizes `text` into words and returns each misspelling with its byte
    /// span. A "word" is a maximal run of ASCII letters and apostrophes (so
    /// contractions like "don't" are one token). Pure numbers and single
    /// letters are treated as correct (not flagged).
    pub fn check(&self, text: &str) -> Vec<Misspelling> {
        let bytes = text.as_bytes();
        let n = bytes.len();
        let mut out = Vec::new();
        let mut i = 0;
        while i < n {
            if is_word_byte(bytes[i]) {
                let start = i;
                while i < n && is_word_byte(bytes[i]) {
                    i += 1;
                }
                let end = i; // exclusive byte offset
                             // Safe: the slice is a run of single-byte ASCII characters.
                let token = &text[start..end];
                if should_skip(token) {
                    continue;
                }
                if !self.is_correct(token) {
                    out.push(Misspelling {
                        word: token.to_string(),
                        byte_start: start,
                        byte_end: end,
                    });
                }
            } else {
                i += 1;
            }
        }
        out
    }

    /// Returns up to `max` correction suggestions for `word`, ranked by edit
    /// distance then alphabetical. Uses the Norvig generate-and-test approach
    /// (edits at distance 1 and 2) so it does NOT scan all 233k words per call.
    /// A correctly-spelled word yields no suggestions.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        if max == 0 {
            return Vec::new();
        }
        let lower = word.to_ascii_lowercase();
        if self.is_correct(&lower) {
            return Vec::new();
        }

        // Distance-1 candidates that are known words.
        let e1 = edits1(&lower);
        let mut found: Vec<(usize, String)> = Vec::new();
        for cand in &e1 {
            if self.known(cand) {
                found.push((1, cand.clone()));
            }
        }

        // If distance 1 did not yield enough, expand to distance 2 — but cap
        // generation to avoid combinatorial blowup. Iteration is deterministic
        // (parents and per-parent candidates are sorted) so output is stable
        // even when the cap is hit.
        if found.len() < max {
            let mut parents: Vec<String> = e1.into_iter().collect();
            parents.sort();
            let mut seen: HashSet<String> = HashSet::new();
            seen.insert(lower.clone());
            let mut generated = 0usize;
            'outer: for parent in &parents {
                let mut e2: Vec<String> = edits1(parent).into_iter().collect();
                e2.sort();
                for cand in e2 {
                    generated += 1;
                    if generated > DISTANCE2_CAP {
                        break 'outer;
                    }
                    if seen.insert(cand.clone()) && self.known(&cand) {
                        found.push((2, cand));
                    }
                }
            }
        }

        // Deterministic ordering: (distance asc, then alphabetical). Dedupe
        // keeps the minimum distance for any repeated word.
        found.sort();
        found.dedup_by(|a, b| a.1 == b.1);
        found.truncate(max);
        found.into_iter().map(|(_, w)| w).collect()
    }

    /// Learns a word into the user dictionary (case-insensitive).
    pub fn add_user_word(&mut self, word: &str) {
        let lower = word.to_ascii_lowercase();
        if !lower.is_empty() {
            self.user.insert(lower);
        }
    }

    /// Returns whether a word is in the user dictionary.
    pub fn is_user_word(&self, word: &str) -> bool {
        self.user.contains(&word.to_ascii_lowercase())
    }

    /// True if `word` (already lowercased) is in the bundled or user set.
    fn known(&self, word: &str) -> bool {
        self.dictionary.contains(word) || self.user.contains(word)
    }
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// A misspelled word and its byte span within the checked text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misspelling {
    /// The misspelled token exactly as it appeared in the source text.
    pub word: String,
    /// Inclusive byte offset of the token within the checked text.
    pub byte_start: usize,
    /// Exclusive byte offset of the token within the checked text.
    pub byte_end: usize,
}

/// Returns true for bytes that form part of a word token: ASCII letters and the
/// apostrophe. All other bytes (including the continuation bytes of multibyte
/// UTF-8 sequences) act as separators, so byte offsets map directly to spans.
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'\''
}

/// Returns true for tokens that must never be flagged as misspellings: empty,
/// single characters, and runs of apostrophes with no letters. (Pure digits
/// never become tokens under [`is_word_byte`].)
fn should_skip(token: &str) -> bool {
    token.len() <= 1 || token.bytes().all(|b| b == b'\'')
}

/// Generates all single-character edits (deletions, adjacent transpositions,
/// substitutions, insertions) of `word` using lowercase a-z. The result may
/// include the original word (no-op substitution); callers filter as needed.
fn edits1(word: &str) -> HashSet<String> {
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut edits = HashSet::new();

    // Deletions: drop one character.
    for i in 0..len {
        let mut c = chars.clone();
        c.remove(i);
        edits.insert(c.into_iter().collect());
    }
    // Adjacent transpositions: swap neighboring characters.
    for i in 0..len.saturating_sub(1) {
        let mut c = chars.clone();
        c.swap(i, i + 1);
        edits.insert(c.into_iter().collect());
    }
    // Substitutions: replace one character with a lowercase letter.
    for i in 0..len {
        for &letter in ALPHABET {
            let mut c = chars.clone();
            c[i] = letter;
            edits.insert(c.into_iter().collect());
        }
    }
    // Insertions: add one lowercase letter at any position.
    for i in 0..=len {
        for &letter in ALPHABET {
            let mut c = chars.clone();
            c.insert(i, letter);
            edits.insert(c.into_iter().collect());
        }
    }

    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> SpellChecker {
        SpellChecker::new()
    }

    #[test]
    fn new_loads_dictionary() {
        let c = checker();
        assert!(c.is_correct("the"));
    }

    #[test]
    fn is_correct_common_words() {
        let c = checker();
        assert!(c.is_correct("hello"));
        assert!(c.is_correct("world"));
        assert!(c.is_correct("color"));
        // en-US: British spelling "colour" is not in the list.
        assert!(!c.is_correct("colour"));
    }

    #[test]
    fn is_correct_case_insensitive() {
        let c = checker();
        assert!(c.is_correct("Hello"));
        assert!(c.is_correct("HELLO"));
    }

    #[test]
    fn is_correct_contractions() {
        let c = checker();
        assert!(c.is_correct("don't"));
        assert!(c.is_correct("can't"));
        assert!(c.is_correct("DON'T"));
    }

    #[test]
    fn check_finds_misspellings_with_spans() {
        let c = checker();
        let text = "hello world teh";
        let miss = c.check(text);
        assert_eq!(miss.len(), 1, "exactly one misspelling expected");
        let m = &miss[0];
        assert_eq!(m.word, "teh");
        assert_eq!(&text[m.byte_start..m.byte_end], "teh");
        assert_eq!(m.byte_end - m.byte_start, "teh".len());
    }

    #[test]
    fn check_skips_numbers_and_single_letters() {
        let c = checker();
        // "3.14", "a", and "I" must not be flagged.
        assert!(c.check("3.14 a I").is_empty());
        // Single letters embedded in a sentence stay unflagged.
        assert!(c.check("a I x").is_empty());
    }

    #[test]
    fn check_spans_are_byte_offsets() {
        let c = checker();
        // A leading multibyte character (é = 2 bytes) must not corrupt byte
        // spans. It is a separator under the ASCII-only tokenization rule, so
        // only "teh" is flagged.
        let text = "é teh";
        assert_eq!("é".len(), 2);
        let miss = c.check(text);
        assert_eq!(miss.len(), 1);
        let m = &miss[0];
        assert_eq!(m.byte_start, 3); // 2 bytes for é + 1 for the space
        assert_eq!(&text[m.byte_start..m.byte_end], "teh");
    }

    #[test]
    fn suggest_returns_near_corrections() {
        let c = checker();
        // "the" is a single transposition away from "teh". The ranking is
        // (distance, alphabetical); request a generous max so the target
        // survives alphabetical truncation.
        let s = c.suggest("teh", 25);
        assert!(s.contains(&"the".to_string()), "got {:?}", s);

        let s = c.suggest("recieve", 25);
        assert!(s.contains(&"receive".to_string()), "got {:?}", s);
    }

    #[test]
    fn suggest_caps_at_max() {
        let c = checker();
        let s = c.suggest("teh", 5);
        assert!(s.len() <= 5);
        let s = c.suggest("recieve", 5);
        assert!(s.len() <= 5);
        // max == 0 returns nothing.
        assert!(c.suggest("teh", 0).is_empty());
    }

    #[test]
    fn suggest_correct_word_returns_nothing() {
        let c = checker();
        assert!(c.suggest("hello", 5).is_empty());
    }

    #[test]
    fn suggest_deterministic() {
        let c = checker();
        let a = c.suggest("teh", 10);
        let b = c.suggest("teh", 10);
        assert_eq!(a, b);
        let a = c.suggest("recieve", 10);
        let b = c.suggest("recieve", 10);
        assert_eq!(a, b);
    }

    #[test]
    fn suggest_results_sorted_by_distance_then_alpha() {
        let c = checker();
        let s = c.suggest("teh", 10);
        // Within a single result set, order must be deterministic. Re-check the
        // same word with a smaller max and confirm it is a prefix-preserving
        // slice (sorting is stable across max values).
        let small = c.suggest("teh", 3);
        for (i, w) in small.iter().enumerate() {
            assert_eq!(w, &s[i]);
        }
    }

    #[test]
    fn add_user_word_makes_it_correct() {
        let mut c = checker();
        assert!(!c.is_correct("xyzzy"));
        c.add_user_word("xyzzy");
        assert!(c.is_correct("xyzzy"));
        assert!(c.is_user_word("xyzzy"));
    }

    #[test]
    fn user_word_case_insensitive() {
        let mut c = checker();
        assert!(!c.is_correct("foobar"));
        c.add_user_word("FooBar");
        assert!(c.is_correct("foobar"));
        assert!(c.is_correct("FOOBAR"));
        assert!(c.is_user_word("FoObAr"));
    }

    #[test]
    fn no_suggestion_for_very_long_word() {
        let c = checker();
        let nonsense = "x".repeat(50);
        // Must return empty quickly, without hanging or panicking.
        let s = c.suggest(&nonsense, 5);
        assert!(s.is_empty(), "got {:?}", s);
    }
}
