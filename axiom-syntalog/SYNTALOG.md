# syntalog

A crate for clausal logic — the layer of the formal language concerned with rules, atoms, literals, and substitution. It sits above `formalisms` (which defines variables, constants, terms, and formulas) and provides the building blocks for logic programming.

---

## Types

### `predicate_symbol`

An `operation_symbol` whose name begins with a **lowercase ASCII letter**. Predicate symbols name relations in the head and body of rules.

```rust
predicate_symbol::new("likes".to_string(), 2)   // ok
predicate_symbol::new("F".to_string(), 1)        // err: must start with lowercase
```

---

### `atom`

A formula of the form `p(t1, …, tn)` where `p` is a `predicate_symbol` of rank `n` and each `ti` is a `term`.

```rust
// loves(alice, bob)
let p = predicate_symbol::new("loves".to_string(), 2)?;
let terms = vec![
    term::new("alice".to_string(), Some(0), vec![])?,
    term::new("bob".to_string(),   Some(0), vec![])?,
];
let a = atom::new(p, terms)?;
```

**Methods**

| Method | Description |
|--------|-------------|
| `variables()` | Returns distinct `individual_variable`s in appearance order |
| `unifies(subs, other)` | Substitutes `subs` into both atoms positionally and checks equality |
| `is_ground()` | `true` if no `individual_variable`s appear in any term |

---

### `literal`

A positive or negative occurrence of a predicate applied to terms.

| Variant | Meaning |
|---------|---------|
| `positive_literal(atom)` | The atom itself — an `atom` is always a positive literal |
| `negative_literal(predicate_symbol, Vec<term>)` | The predicate negated with `¬`, `-`, or `not`; **not** an atom |

```rust
// positive: lego_builder(alice)
let pos = literal::positive_literal(atom::new(...)?);

// negative: ¬lego_builder(alice)
let neg = literal::negative(predicate_symbol::new("lego_builder".to_string(), 1)?, terms)?;
```

---

### `RuleType`

Discriminates the structural form of a `rule`.

| Variant | Form | Constraint |
|---------|------|------------|
| `General` | `h1, …, hn :- b1, …, bm` | None |
| `UnitClause` | `h1, …, hn` | Non-empty head, no body |
| `Goal` | `:- b1, …, bm` | No head, non-empty body |
| `DefiniteClause` | `h :- b1, …, bm` | Exactly one positive head literal |
| `HornRule` | `h1, …, hn :- b1, …, bm` | At most one positive literal total |
| `Fact` | `h1, …, hn` | Ground `UnitClause` — no variables |

---

### `rule`

A clause `h1, …, hn :- b1, …, bm`. All rule subtypes are constructed as `rule` values carrying a `RuleType` discriminant.

```rust
// General
let r = rule::new(head, body);

// Unit clause: loves(alice, bob)
let r = rule::unit_clause(head)?;

// Goal: :- happy(A), lego_builder(A)
let r = rule::goal(body)?;

// Definite clause: qsort(A,B) :- empty(A), empty(B)
let r = rule::definite_clause(head, body)?;

// Horn rule: at most one positive literal
let r = rule::horn(head, body)?;

// Fact: loves(andrew, laura)
let r = rule::fact(head)?;
```

**Methods**

| Method | Description |
|--------|-------------|
| `variables()` | Distinct `individual_variable`s in appearance order (head then body) |
| `substitution(subs)` | Replaces each variable with the corresponding term in `subs` |
| `is_ground()` | `true` if no `individual_variable`s appear anywhere |
| `to_json()` | Compact JSON string |
| `to_json_pretty()` | Indented JSON string |

---

### `is_ground` trait

Applied to `term`, `operation`, `atom`, `literal`, and `rule`. Returns `true` if no `individual_variable`s are present at any depth.

```rust
// op(a, b) — ground
// op(A, b) — not ground
assert!(atom.is_ground());
```

---

### `clausal_theory`

A collection of `rule`s forming a logical theory.

```rust
let theory = clausal_theory::new(vec![rule1, rule2, rule3]);
```

---

## Parsing

```rust
use syntalog::parse_rule;

let r = parse_rule("happy(A) :- lego_builder(A), enjoys_lego(A)")?;
let r = parse_rule(":- happy(A)")?;           // goal
let r = parse_rule("loves(alice, bob)")?;     // unit clause
let r = parse_rule("safe(A) :- ¬danger(A)")?; // negative literal
let r = parse_rule("safe(A) :- not danger(A)")?;
let r = parse_rule("safe(A) :- -danger(A)")?;
```

### Rule Grammar

```
rule         := ':-' literal_list
              | literal_list ':-' literal_list
              | literal_list
literal_list := literal (',' literal)*
literal      := negation atom | atom
negation     := '¬' | '-' | 'not' whitespace
atom         := name '(' term_list ')' | name
term_list    := term (',' term)*
term         := variable | name '(' term_list ')' | name
variable     := [A-Z] '\''*
name         := [a-z][a-zA-Z0-9_]*
```

---

## Substitution

`rule::substitution` replaces each `individual_variable` in the rule with the corresponding term from `subs`, in the order variables first appear (head then body left to right).

```rust
// likes(X, Y) :- knows(X, Y) → likes(alice, bob) :- knows(alice, bob)
let subs = vec![
    term::new("alice".to_string(), Some(0), vec![])?,
    term::new("bob".to_string(),   Some(0), vec![])?,
];
let r2 = r.substitution(subs)?;
```

`subs.len()` must equal the number of distinct variables in the rule.

---

## Unification

`atom::unifies` substitutes the same `subs` vector into both `self` and `other` (using each atom's own variable ordering) and checks whether the results are equal.

```rust
// likes(X, Y).unifies([alice, bob], likes(A, B)) → true
let result = self_atom.unifies(subs, &other_atom)?;
```

Requirements:
- `subs.len()` == number of distinct variables in `self`
- `self` and `other` have the same number of variables
- `self` and `other` share no variable names

---

## JSON Serialization

```rust
r.to_json()        // compact
r.to_json_pretty() // indented
```

Example output for `happy(A) :- lego_builder(A), enjoys_lego(A)`:

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
  "body": [
    {
      "polarity": "positive",
      "atom": {
        "predicate": "lego_builder",
        "terms": [
          { "type": "variable", "name": "A" }
        ]
      }
    },
    {
      "polarity": "positive",
      "atom": {
        "predicate": "enjoys_lego",
        "terms": [
          { "type": "variable", "name": "A" }
        ]
      }
    }
  ]
}
```

---

## Bibliography

Cropper, Andrew, and Sebastijan Dumančić. "Inductive Logic Programming At 30: A New Introduction." *Journal of Artificial Intelligence Research* 74 (2022): 765–850.
