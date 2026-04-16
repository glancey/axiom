# axiom

A CLI tool for constructing, validating, and evaluating well-formed formulas (wffs) of a first-order logic formal language, based on Monk's formalization in his book, *Introduction to Set Theory*.

## Workspace

| Crate | Role |
| ----- | ---- |
| `axiom` | CLI entry point |
| `formalisms` | Core domain types and formula evaluation |
| `axiom_parser` | Recursive-descent formula parser |
| `axiom-syntalog` | Clausal logic — rules, atoms, literals, substitution ([docs](syntalog/SYNTALOG.md)) |
| `axiom-prolog` | Formal grammar-to-Prolog format compiler and interactive query REPL |

## Running axiom

Before building, the `scryer-prolog` dependency in [axiom-prolog/Cargo.toml](axiom-prolog/Cargo.toml) must point to a local clone of the source:

```toml
scryer-prolog = { path = "/path/to/scryer-prolog", default-features = false }
```

Clone the source with:

```sh
git clone https://github.com/mthom/scryer-prolog.git
```

Then update the `path` in `axiom-prolog/Cargo.toml` to match the location of your clone.

## Installation

```sh
cargo build --release
```

## Commands

### `validate`

Validates a string against one of six language construct types (interactive selection).

```sh
axiom validate <value> [args...]
```

After running, select a type:

```text
1. individual_variable
2. logical_symbol
3. operation_symbol
4. individual_constant
5. relation_symbol
6. term
```

**Choices 3 and 5** derive rank from trailing `args` or by parsing a `name(arg1, arg2, ...)` string:

```sh
axiom validate "f(a, b, c)"          # choice 3 → operation_symbol(f, rank=3)
axiom validate f a b c               # choice 3 → operation_symbol(f, rank=3)
axiom validate "rel(a, b)"           # choice 5 → relation_symbol(rel, rank=2)
```

---

### `check-formula`

Parses a string and reports whether it is a valid formula.

```sh
axiom check-formula <value>
```

**Examples:**

```sh
axiom check-formula "P->Q"
# Valid formula: P->Q

axiom check-formula "not(A -> B)"
# Valid formula: ¬(A -> B)

axiom check-formula "rel(a, b)"
# relation_symbol(rel, rank=2), args: ["a", "b"]
# Valid formula: rel(a, b)
```

---

### `tautological-proof`

Evaluates whether a formula is a tautology (true under every possible truth assignment).

```sh
axiom tautological-proof <value>
```

**Examples:**

```sh
axiom tautological-proof "P->P"
# Tautology: P->P

axiom tautological-proof "P->Q"
# Not a tautology: P->Q

axiom tautological-proof "(not(P and Q) <-> (notP or notQ))"
# Tautology: (not(P and Q) <-> (notP or notQ))
```

---

## axiom-syntalog

`axiom-syntalog` provides clausal logic — rules, atoms, literals, and substitution. Full documentation is in [axiom-syntalog/SYNTALOG.md](axiom-syntalog/SYNTALOG.md).

### Key types

| Type | Description |
| ---- | ----------- |
| `predicate_symbol` | A lowercase-named relation symbol |
| `atom` | `p(t1, ..., tn)` — a predicate applied to terms |
| `literal` | Positive or negative occurrence of an atom |
| `rule` | A clause `h1, ..., hn :- b1, ..., bm` with a `RuleType` discriminant |
| `RuleType` | `General`, `UnitClause`, `Goal`, `DefiniteClause`, `HornRule`, `Fact` |

---

### `serialize-rule`

Parses a rule string and prints its JSON representation (pretty-printed).

```sh
axiom serialize-rule <rule>
```

**Example:**

```sh
axiom serialize-rule "happy(A) :- lego_builder(A), enjoys_lego(A)"
```

```json
{
  "rule_type": "General",
  "head": [
    {
      "polarity": "positive",
      "atom": {
        "predicate": "happy",
        "terms": [
          { "type": "variable", "name": "A" }
        ]
      }
    }
  ],
  "body": [...]
}
```

---

### `substitution`

Parses a rule, substitutes a comma-separated list of terms for the rule's variables (in appearance order), and pretty-prints the resulting rule as JSON.

```sh
axiom substitution <terms> <rule>
```

**Example:**

```sh
axiom substitution "alice,bob" "happy(A,B) :- likes(A,B)"
```

`subs` must have exactly as many entries as there are distinct variables in the rule.

---

### `glossary`

Prints descriptions of all language constructs.

```sh
axiom glossary
```

---

## axiom-prolog

`axiom-prolog` compiles `.apl` source files to standard Prolog (`.pl`) and provides an interactive query REPL backed by [scryer-prolog](https://github.com/mthom/scryer-prolog).

`.apl` files may use either Prolog-style syntax (`head :- body`) or axiom formula syntax (`(body) -> (head)`). Each line is converted to canonical Prolog by `to_prolog_string`.

### `query`

Loads a `.pl` Prolog file and starts an interactive query REPL.

```sh
axiom-prolog query
# Prolog file path: facts.pl
# Loaded 'facts.pl' as module 'facts'.
# Enter a Prolog query (e.g. member(X, [1,2,3]).) or Ctrl-D to quit.
#
# ?- happy(alice).
```

Queries may be entered in Prolog syntax or axiom formula syntax; axiom inputs are converted via `to_prolog_string` before being run.

---

### `compile`

Converts a `.apl` source file to a `.pl` file by running each line through `to_prolog_string`.

```sh
axiom-prolog compile
# File path (.apl): rules.apl
# Compiled 'rules.apl' → 'rules.pl'
```

**Example `.apl` input:**

```text
is_weekday(monday).
(Day = saturday or Day = sunday) -> is_weekend(Day)
```

**Compiled `.pl` output:**

```prolog
is_weekday(monday).
is_weekend(Day) :- (Day = saturday ; Day = sunday).
```

---

## Formula Grammar

```text
formula     := quantifier | negation | combination | atomic
quantifier  := ('∀' | 'Ǝ') variable '.' formula
negation    := '¬' formula
combination := '(' formula connective formula ')'
atomic      := variable | relation | constant
relation    := name '(' term (',' term)* ')'
term        := variable | constant | operation
operation   := name '(' term (',' term)* ')'
constant    := name
variable    := [A-Z] '\''*
name        := [a-z][a-zA-Z0-9_]*
connective  := '<->' | '->' | '∧' | '∨' | '='
```

Binary combinations must be **fully parenthesized**: `(A -> B)`, not `A -> B`.
A single formula in parentheses `(A)` is unwrapped transparently.

### Natural-language aliases

The parser and CLI normalize the following before parsing:

| Input | Normalized |
| ----- | ---------- |
| `for all` | `∀` |
| `there exists` | `Ǝ` |
| ` and ` | ` ∧ ` |
| ` or ` | ` ∨ ` |
| ` not ` | `¬` |
| `not(expr)` | `¬(expr)` |
| `notX` (X uppercase) | `¬X` |
| `~(expr)` | `¬(expr)` |
| `~X` | `¬X` |

---

## Running Tests

```sh
cargo test --workspace
```

---

## To Do

Create and display a formal proof ala Kalish and Montague in their book, *Logic*.

---

## Bibliography

Monk, J. Donald. *Introduction to Set Theory*. McGraw-Hill, Inc., 1969. Library of Congress Card Number 68-20056.

Kalish, Donald, and Richard Montague. *Logic: Techniques of Formal Reasoning*. Harcourt, Brace & World, Inc., 1964. ISBN 0-15-551180-7.

Cropper, Andrew, and Sebastijan Dumančić. "Inductive Logic Programming At 30: A New Introduction." *Journal of Artificial Intelligence Research* 74 (2022): 765–850.

---

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
