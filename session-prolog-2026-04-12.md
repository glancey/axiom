# axiom-prolog Session — 2026-04-12

## Overview

This session built out the `axiom-prolog` crate and extended axiom-syntalog and formalisms to support Prolog rule compilation from axiom formula notation.

---

## Requests and Changes (chronological)

### 1. Rename `scryer-prolog-embed` → `axiom-prolog`
- Renamed directory and package name in `Cargo.toml`

### 2. Branch setup
- Stashed changes, checked out new branch `prolog`, popped stash

### 3. Create `axiom-prolog/src/lib.rs`
- Created with `to_prolog_string(input: &str) -> String` calling `normalize_for_parse`

### 4. Edit `main.rs` to call `to_prolog_string`
- Applied conversion to REPL queries before passing to scryer-prolog

### 5. Refactor: move `main()` logic to `lib.rs`
- Extracted interactive REPL into `pub fn query(path: PathBuf)` in lib.rs
- `main.rs` delegates to `axiom_prolog::query`

### 6. Add CLI subcommands (`Query`, `Normalize`, `Compile`)
- Used `clap` with `#[derive(Subcommand)]`
- Each command prompts interactively via a `prompt()` helper

### 7. Fix `to_prolog_string` to use axiom-syntalog
- Replaced `normalize_for_parse` fallback-only approach
- Now calls `parse_formula_as_rule` (for `=>` input) or `parse_rule` (for Prolog `:-` input)
- Falls back to `normalize_for_parse` on parse failure

### 8. Parse `(Day==saturday or Day==sunday) => is_weekend(Day)`
- Target output: `is_weekend(Day) :- (Day = saturday ; Day = sunday).`
- Required multiple extensions (see below)

### 9. `literal` enum: new variants in `axiom-syntalog/src/lib.rs`
- Added `equality_literal(term, term)` — displays as `X = Y` (single `=`, Prolog style)
- Added `disjunction(Vec<Vec<literal>>)` — n-way disjunction, each branch is a conjunction
- Display for `disjunction`: `(branch1 ; branch2 ; ...)` with `,` within branches

### 10. Multi-char `individual_variable` support
- `formalisms/src/lib.rs`: changed validation from `[A-Z][']*` to `[A-Z][a-zA-Z0-9_']*`
- `axiom-parser/src/lib.rs`: `try_parse_variable` / `individual_variable` extended to read full token

### 11. `normalize_eq` in `axiom-prolog/src/lib.rs`
- Converts standalone `=` (not part of `=>` or `<=>`) to `==` before parsing
- Enables input like `Day=saturday` to reach the axiom parser as `Day==saturday`

### 12. Pipeline in `parse_formula_as_rule` (`axiom-syntalog/src/parse.rs`)
```
input
  → normalize_eq          (standalone = → ==, in axiom-prolog)
  → wrap_equalities        (bare X==Y → (X==Y), in parse.rs)
  → right_nest_disjunctions (flat (A or B or C) → right-nested binary, in parse.rs)
  → axiom_parser::parse_formula
  → formula_type_to_literal conversion
  → rule.to_string()       (Prolog output)
```

### 13. N-way disjunction pre-processing (new helpers in `parse.rs`)
- `find_matching_close(s, open) -> Option<usize>` — finds matching `)`
- `split_top_level_or(s) -> Vec<&str>` — splits on depth-0 ` or `
- `right_nest_or(parts) -> String` — right-nests: `[A,B,C]` → `A or (B or C)`
- `right_nest_disjunctions(s) -> String` — recursively processes all parenthesised groups

### 14. `compile` function + `Compile` CLI command
- `compile(path: PathBuf)`: reads `.apl` file, applies `to_prolog_string` per line, writes `.pl`
- Error if extension is not `.apl`

### 15. Fix `helpers.rs` `"equals"` mapping
- Changed `"equals" => "=="` to `"equals" => "="`
- Updated `normalize_eq` in `axiom-prolog/src/lib.rs` accordingly

### 16. Fix UTF-8 bug in `right_nest_disjunctions`
- Fallthrough char handling used `s.as_bytes()[i] as char` — broken for multi-byte chars (e.g. `¬`)
- Fixed to:
  ```rust
  let c = s[i..].chars().next().unwrap();
  result.push(c);
  i += c.len_utf8();
  ```
- This fixed the failing test `parse::tests::formula_negated_body_literal`

---

## Final State of Key Files

### `axiom-prolog/src/lib.rs`
```rust
use axiom::helpers::normalize_for_parse;
use axiom_syntalog::{parse_rule, parse_formula_as_rule};
use scryer_prolog::{LeafAnswer, MachineBuilder, Term};
use serde_json::{json, Value};

fn normalize_eq(s: &str) -> String { /* standalone = → == */ }

pub fn to_prolog_string(input: &str) -> String {
    let rule_str = if input.contains("=>") {
        let normalized = normalize_eq(input);
        parse_formula_as_rule(&normalized).map(|r| r.to_string())
    } else {
        parse_rule(input).map(|r| r.to_string())
    }
    .unwrap_or_else(|_| normalize_for_parse(input));
    if rule_str.ends_with('.') { rule_str } else { format!("{rule_str}.") }
}

pub fn compile(path: PathBuf) { /* .apl → .pl line-by-line */ }
pub fn query(path: PathBuf)   { /* interactive REPL */ }
```

### `axiom-prolog/src/main.rs`
```rust
#[derive(Subcommand)]
enum Commands {
    Query,
    Normalize,
    Compile,
}

fn main() {
    match cli.command {
        Commands::Query    => axiom_prolog::query(PathBuf::from(prompt("Prolog file path: "))),
        Commands::Normalize => println!("{}", axiom_prolog::to_prolog_string(&prompt("Input: "))),
        Commands::Compile  => axiom_prolog::compile(PathBuf::from(prompt("File path (.apl): "))),
    }
}
```

---

## Test Results (final)

```
test result: ok. 40 passed; 0 failed  (axiom root)
test result: ok. 46 passed; 0 failed  (axiom-syntalog)
test result: ok. 39 passed; 0 failed  (formalisms)
test result: ok. 10 passed; 0 failed  (axiom-parser)
```

All 135 tests pass across the workspace.

---

## Example Conversions

| Input (`.apl`) | Output (Prolog) |
|---|---|
| `(Day==saturday or Day==sunday) => is_weekend(Day)` | `is_weekend(Day) :- (Day = saturday ; Day = sunday).` |
| `(Day=monday or Day=tuesday or Day=wednesday or Day=thursday or Day=friday) => is_weekday(Day)` | `is_weekday(Day) :- (Day = monday ; Day = tuesday ; Day = wednesday ; Day = thursday ; Day = friday).` |
| `member(X, [H\|T]) :- member(X, T)` | `member(X,[H\|T]) :- member(X,T).` |

---

## Fix: `query` command returning spurious `false` on successful queries

**Problem:** The `query` REPL returned both the real answers and a trailing `{"result": false}` for any query that succeeded. A genuinely failing query correctly returned only `false`.

**Cause:** Scryer-prolog always appends a final `LeafAnswer::False` after all solutions to signal "no more results". The code collected it alongside the real answers.

**Fix** in `axiom-prolog/src/lib.rs`:

```rust
let mut answers: Vec<Value> = machine.run_query(query).map(answer_to_json).collect();
// scryer-prolog appends a final False to signal "no more solutions"; drop it
// when preceding answers exist so False only appears for genuinely failed queries.
if answers.len() > 1 {
    if answers.last() == Some(&json!({ "result": false })) {
        answers.pop();
    }
}
```
