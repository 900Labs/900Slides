# Wave 6 — v0.2.0 spell-check

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 ("Spell-check (en-US) with a user
dictionary folder") and the `slides-spell` crate (currently a stub)
Last updated: 2026-07-28

Wave 6 adds **offline spell-check** for en-US text. Unlike the content-type
waves (1–5), spell-check is a **tooling service**, not a deck model feature:
it checks text on demand and does not alter the `slides-core` model or
persist to the PPTX package. This keeps it decoupled — two components only:
the `slides-spell` crate and the desktop wiring.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Spell-check crate | `crates/slides-spell/` | New (was stub): dictionary + checker + suggestions |
| 2 | Desktop | `apps/desktop/` | Squiggles, suggestion menu, add-to-dictionary, user dictionary folder |

## Explicitly out of this wave

- Languages other than en-US.
- Grammar checking.
- Persistent per-deck dictionaries (the user dictionary is a single shared
  folder, not per-deck). The deck model is untouched.
- Suggestion ranking beyond edit distance (no frequency/learning model).
- Thesaurus / synonyms.

## Dictionary data

A bundled en-US word list lives at `crates/slides-spell/data/en-US.txt`
(~233,000 lowercase ASCII words plus contractions). Source: the public-domain
Webster's 2nd `words` file, filtered and de-duplicated. The crate embeds it
via `include_str!` so spell-check works **fully offline** with no network
dependency (PRODUCT_SPEC.md §4 — local-first, offline-first).

## Component 1 — Spell-check crate (`slides-spell`, no longer a stub)

Depends on `slides-core` only for sharing text types if needed (it can also
be standalone). Provides:

```rust
/// An offline en-US spell checker backed by a bundled word list plus an
/// optional user dictionary (learned words).
pub struct SpellChecker { /* bundled set + user set */ }

impl SpellChecker {
    /// Loads the bundled en-US dictionary. Deterministic and offline.
    pub fn new() -> Self;

    /// True if `word` is spelled correctly (case-insensitive). Considers the
    /// user dictionary in addition to the bundled list.
    pub fn is_correct(&self, word: &str) -> bool;

    /// Tokenizes `text` into words and returns each misspelling with its
    /// byte span. A "word" is a maximal run of ASCII letters and apostrophes
    /// (so contractions like "don't" are one token). Pure numbers and
    /// single letters are treated as correct (not flagged).
    pub fn check(&self, text: &str) -> Vec<Misspelling>;

    /// Returns up to `max` correction suggestions for `word`, ranked by
    /// edit distance then alphabetical. Uses the Norvig generate-and-test
    /// approach (edits at distance 1 and 2) so it does NOT scan all 233k
    /// words per call.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String>;

    /// Learns a word into the user dictionary (case-insensitive).
    pub fn add_user_word(&mut self, word: &str);

    /// Returns whether a word is in the user dictionary.
    pub fn is_user_word(&self, word: &str) -> bool;
}

/// A misspelled word and its byte span within the checked text.
#[derive(Debug, Clone, PartialEq)]
pub struct Misspelling {
    pub word: String,
    pub byte_start: usize,
    pub byte_end: usize,
}
```

### Algorithm notes

- **Membership**: a `HashSet<String>` of lowercased words. Exact (no false
  positives that silently accept errors), built once at `new()`. ~233k
  entries; acceptable memory for a desktop app.
- **Tokenization**: iterate bytes; accumulate `[a-zA-Z']`; on a boundary,
  check the token. Skip tokens that are pure digits or length 1. This makes
  `check` O(n) in the text length.
- **Suggestions** (the performance-critical part): brute-forcing edit
  distance over 233k words is too slow. Instead use the Norvig method:
  generate all single-character edits (deletions, insertions, substitutions,
  adjacent transpositions) of the misspelled word, filter to those present in
  the dictionary; if fewer than `max` results, expand to distance-2 edits.
  This visits a few hundred candidates, not 233k. Cap distance-2 generation
  to avoid combinatorial blowup on long inputs.
- **Determinism**: suggestion order must be stable — sort candidates by
  (distance asc, then alphabetical). No HashMap iteration in output.

### Tests

- `is_correct_common_words`: "hello", "world", "color" correct; "colour" (en-US) flagged.
- `is_correct_case_insensitive`: "Hello", "HELLO" both correct.
- `is_correct_contractions`: "don't", "can't" correct.
- `check_finds_misspellings_with_spans`: a sentence with one typo returns one Misspelling with correct byte span.
- `check_skips_numbers_and_single_letters`: "3.14" and "a I" not flagged.
- `suggest_returns_near_corrections`: suggest("teh") includes "the"; suggest("recieve") includes "receive".
- `suggest_caps_at_max`: suggest(..., 5) returns at most 5.
- `suggest_deterministic`: same input -> identical output order.
- `add_user_word_makes_it_correct`: add "xyzzy", then is_correct("xyzzy").
- `user_word_case_insensitive`: add "FooBar", is_correct("foobar").
- `no_suggestion_for_very_long_word`: a 50-char nonsense word returns [] quickly (no hang).

## Component 2 — Desktop (`apps/desktop/`)

Mirrors existing conventions (study the chart/animation command wiring in
`commands.rs` and the Svelte components).

Tauri commands (hold a `SpellChecker` in shared state, built once at startup):
- `spell_check(text: String) -> Vec<MisspellingDto>` — check arbitrary text.
- `spell_suggest(word: String, max: usize) -> Vec<String>` — suggestions.
- `spell_add_word(word: String)` — learn into user dictionary (also persists
  to the user dictionary folder).

User dictionary folder: a file under the app's data dir (use the existing
app-data path the recovery code already resolves). `spell_add_word` appends;
on startup the checker loads it. Keep it simple: one newline-delimited file.

Frontend (Svelte):
- When editing a text box, debounce-check the text and render **red squiggles**
  under misspelled words (CSS `text-decoration: underline wavy red`). The
  existing text-box editor is where squiggles render.
- **Right-click a misspelled word** → context menu listing suggestions (click
  replaces the word) plus "Add to dictionary".
- Keep it non-blocking: spell-check runs in the background; never blocks typing.

## Dependency ordering

1. **Crate** (component 1) — single worktree, merged first.
2. **Desktop** (component 2) — after the crate lands.

## Acceptance criteria

1. "teh" is flagged and "the" is suggested; "receive" is correct and "recieve"
   is flagged with "receive" suggested.
2. `check` returns correct byte spans for misspellings in a sentence.
3. Suggestions are deterministic and never scan the full dictionary per call
   (no perceptible latency on a typical word).
4. Adding a user word makes it correct for the session and persists to the
   user dictionary folder.
5. En-US only: "colour" is flagged (US spelling is "color").
6. Works fully offline; no network dependency.
7. Quality gate green. Privacy gate passes. No telemetry.

## Test fixtures

- No PPTX fixtures (spell-check does not touch the format layer).
- Unit tests in the crate; the desktop side verified via svelte-check + build.
