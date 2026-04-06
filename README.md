# axiom

A CLI tool for constructing, validating, and evaluating well-formed formulas (wffs) of a first-order logic formal language, based on Monk's formalization in his book, *Introduction to Set Theory*.

## Workspace

| Crate | Role |
| ----- | ---- |
| `axiom` | CLI entry point |
| `formalisms` | Core domain types and formula evaluation |
| `axiom_parser` | Recursive-descent formula parser |
| `syntalog` | Clausal logic — rules, atoms, literals, substitution ([docs](syntalog/SYNTALOG.md)) |

## Installation

```sh
cargo build --release
```

## Commands

### `hello`

Greets a name, validating it as an `individual_variable`.

```sh
axiom hello --name X
```

---

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
axiom check-formula "P=>Q"
# Valid formula: P=>Q

axiom check-formula "not(A => B)"
# Valid formula: ¬(A => B)

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
axiom tautological-proof "P=>P"
# Tautology: P=>P

axiom tautological-proof "P=>Q"
# Not a tautology: P=>Q

axiom tautological-proof "(not(P and Q) <=> (notP or notQ))"
# Tautology: (not(P and Q) <=> (notP or notQ))
```

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
connective  := '<=>' | '=>' | '∧' | '∨' | '=='
```

Binary combinations must be **fully parenthesized**: `(A => B)`, not `A => B`.
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

---

## Language Constructs

### `individual_variable`

A single uppercase letter, optionally followed by apostrophes.

```text
A   B'   X'''
```

### `logical_symbol`

One of the fixed connectives: `∧` `∨` `=>` `¬` `<=>` `∀` `Ǝ` `==` `(` `)`

### `operation_symbol`

A named symbol of rank m ≥ 0. Cannot be a `logical_symbol` or `individual_variable`.

```text
f(a, b, c)   →  operation_symbol f of rank 3
```

### `individual_constant`

An `operation_symbol` of rank 0 — names a fixed individual.

```text
socrates   zero   c1
```

### `relation_symbol`

An `operation_symbol` of rank 1–5 used to express a relation between individuals.

```text
rel(a, b)   →  relation_symbol rel of rank 2
```

### `term`

One of: an `individual_variable`, an `individual_constant`, or an `operation_symbol` of rank m > 0 applied to m sub-terms.

### `Formula`

A well-formed formula (wff). Supports evaluation via `is_true`, `evaluate` (under a truth assignment), and `is_tautology`.

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
