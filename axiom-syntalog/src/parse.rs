use anyhow::Result;
use formalisms::{term, TermType, FormulaType};
use crate::{predicate_symbol, atom, literal, rule};
use axiom_parser::Parser;

/// Parses a string of the form `h1, …, hn :- b1, …, bm` into a [`rule`].
///
/// # Grammar
/// ```text
/// rule         := literal_list ':-' literal_list
///               | ':-' literal_list
///               | literal_list
/// literal_list := literal (',' literal)*
/// literal      := ('¬' | '-' | 'not' ws+) atom | atom
/// atom         := name '(' term_list ')' | name
/// term_list    := term (',' term)*
/// term         := variable | name '(' term_list ')' | name
/// variable     := [A-Z] '\''*
/// name         := [a-z][a-zA-Z0-9_]*
/// ```
pub fn parse_rule(s: &str) -> Result<rule> {
    let mut p = Parser::new(s.trim());
    let r = rule_inner(&mut p)?;
    p.skip_ws();
    if !p.is_done() {
        anyhow::bail!("unexpected input at position {}: {:?}", p.pos(), p.rest());
    }
    Ok(r)
}

fn rule_inner(p: &mut Parser) -> Result<rule> {
    p.skip_ws();
    if p.rest().starts_with(":-") {
        p.advance(2);
        let body = literal_list(p)?;
        return rule::goal(body);
    }
    let head = literal_list(p)?;
    p.skip_ws();
    if p.rest().starts_with(":-") {
        p.advance(2);
        let body = literal_list(p)?;
        return rule::new(head, body);
    }
    rule::unit_clause(head)
}

fn literal_list(p: &mut Parser) -> Result<Vec<literal>> {
    let mut lits = Vec::new();
    loop {
        p.skip_ws();
        if p.rest().is_empty() || p.rest().starts_with(":-") { break; }
        lits.push(parse_literal(p)?);
        p.skip_ws();
        if p.rest().starts_with(',') { p.advance(1); } else { break; }
    }
    Ok(lits)
}

fn parse_literal(p: &mut Parser) -> Result<literal> {
    p.skip_ws();
    let negated =
        if p.rest().starts_with('¬') {
            p.advance('¬'.len_utf8());
            true
        } else if p.rest().starts_with('-') {
            p.advance(1);
            true
        } else if p.rest().starts_with("not")
            && p.rest()[3..].starts_with(|c: char| c.is_whitespace() || c == '(') {
            p.advance(3);
            true
        } else {
            false
        };
    let a = parse_atom(p)?;
    if negated {
        literal::negative(a.predicate, a.terms)
    } else {
        Ok(literal::positive_literal(a))
    }
}

fn parse_atom(p: &mut Parser) -> Result<atom> {
    p.skip_ws();
    let name = p.name()?;
    p.skip_ws();
    let terms = if p.rest().starts_with('(') {
        p.consume("(")?;
        let ts = p.term_list()?;
        p.consume(")")?;
        ts
    } else {
        vec![]
    };
    let rank = terms.len() as u32;
    let pred = predicate_symbol::new(name, rank)?;
    atom::new(pred, terms)
}

/// Parses a formula string of the form `(b1 ∧ … ∧ bm) => (h1 ∧ … ∧ hn)` into a [`rule`].
///
/// Uses [`axiom_parser::parse_formula`] to consume the formula, then converts the
/// top-level implication into `head :- body` form. Each conjunction side is
/// recursively flattened so that nested `∧` chains all become flat literal lists.
///
/// # Accepted connective spellings
/// The `axiom_parser` normalises `" and "` → `∧` before parsing, so both
/// `(b1 and b2) => h1` and `(b1 ∧ b2) => h1` are accepted.
///
/// # Errors
/// Returns an error if the formula is not a top-level `=>` combination, or if
/// any operand cannot be converted to a syntalog literal.
/// Wraps each bare `lhs==rhs` equality in parentheses so the axiom parser
/// (which requires every binary combination to be explicitly parenthesised)
/// can parse them when they appear inside a disjunction or conjunction.
/// e.g. `Day==saturday or Day==sunday` → `(Day==saturday) or (Day==sunday)`.
fn wrap_equalities(s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;
    while let Some(eq_pos) = rest.find("==") {
        let before = &rest[..eq_pos];
        let after = &rest[eq_pos + 2..];
        let lhs_start = before
            .rfind(|c: char| !c.is_alphanumeric() && c != '\'' && c != '_')
            .map(|p| p + 1)
            .unwrap_or(0);
        let rhs_end = after
            .find(|c: char| !c.is_alphanumeric() && c != '\'' && c != '_')
            .unwrap_or(after.len());
        result.push_str(&before[..lhs_start]);
        result.push('(');
        result.push_str(&before[lhs_start..]);
        result.push_str("==");
        result.push_str(&after[..rhs_end]);
        result.push(')');
        rest = &after[rhs_end..];
    }
    result.push_str(rest);
    result
}

/// Finds the byte index of the `)` matching the `(` at byte offset `open`.
fn find_matching_close(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { return Some(open + i); } }
            _ => {}
        }
    }
    None
}

/// Splits `s` on top-level (depth-0) ` or ` occurrences, returning the pieces.
fn split_top_level_or<'a>(s: &'a str) -> Vec<&'a str> {
    let mut parts = vec![];
    let mut depth = 0usize;
    let mut start = 0;
    let mut i = 0;
    while i < s.len() {
        match s.as_bytes()[i] {
            b'(' => depth += 1,
            b')' => { if depth > 0 { depth -= 1; } }
            b' ' if depth == 0 && s[i..].starts_with(" or ") => {
                parts.push(&s[start..i]);
                i += 4;
                start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

/// Right-nests a slice of operands with ` or `:
/// `[A, B, C, D]` → `A or (B or (C or D))`.
fn right_nest_or(parts: &[&str]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.to_string(),
        _ => format!("{} or ({})", parts[0], right_nest_or(&parts[1..])),
    }
}

/// Recursively scans `s` and right-nests any parenthesised group that contains
/// multiple top-level ` or ` operands, making them parseable as binary combinations.
fn right_nest_disjunctions(s: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with('(') {
            let close = find_matching_close(s, i).expect("unmatched paren");
            let inner = &s[i + 1..close];
            let parts = split_top_level_or(inner);
            if parts.len() > 1 {
                let processed: Vec<String> = parts.iter()
                    .map(|p| right_nest_disjunctions(p))
                    .collect();
                let refs: Vec<&str> = processed.iter().map(String::as_str).collect();
                result.push('(');
                result.push_str(&right_nest_or(&refs));
                result.push(')');
            } else {
                result.push('(');
                result.push_str(&right_nest_disjunctions(inner));
                result.push(')');
            }
            i = close + 1;
        } else {
            let c = s[i..].chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    result
}

pub fn parse_formula_as_rule(s: &str) -> Result<rule> {
    // Wrap bare lhs==rhs expressions in parens, right-nest flat `or` chains so
    // the axiom parser (binary-only) can parse them, then wrap the whole formula
    // in outer parens so the top-level '=>' is seen as a binary connective.
    let preprocessed = wrap_equalities(s.trim());
    let preprocessed = right_nest_disjunctions(&preprocessed);
    let wrapped = format!("({})", preprocessed);
    let ft = axiom_parser::parse_formula(&wrapped)?;
    match ft {
        FormulaType::Combination(sym, mut parts)
            if sym.symbol() == "=>" && parts.len() == 2 =>
        {
            // parts[0] = body conjunction, parts[1] = head conjunction
            let head_f = parts.remove(1);
            let body_f = parts.remove(0);
            let head_lits = flatten_conjunction(head_f.formula_type)?;
            let body_lits = flatten_conjunction(body_f.formula_type)?;
            if head_lits.is_empty() {
                anyhow::bail!("rule head must contain at least one literal");
            }
            rule::new(head_lits, body_lits)
        }
        _ => anyhow::bail!(
            "formula must be a top-level implication of the form (body) => (head)"
        ),
    }
}

/// Recursively flattens a `FormulaType` over `∧` into a flat list of literals.
/// A `∨` combination becomes a single [`literal::disjunction`] wrapping its branches.
/// A non-conjunction/disjunction operand is converted directly via [`formula_type_to_literal`].
fn flatten_conjunction(ft: FormulaType) -> Result<Vec<literal>> {
    match ft {
        FormulaType::Combination(sym, parts) if sym.symbol() == "\u{2227}" => {
            let mut lits = Vec::new();
            for f in parts {
                lits.extend(flatten_conjunction(f.formula_type)?);
            }
            Ok(lits)
        }
        FormulaType::Combination(sym, parts) if sym.symbol() == "\u{2228}" => {
            let mut all_branches: Vec<Vec<literal>> = Vec::new();
            for part in parts {
                let lits = flatten_conjunction(part.formula_type)?;
                // Flatten: if this part is itself a single disjunction, absorb its branches
                if lits.len() == 1 {
                    match lits.into_iter().next().unwrap() {
                        literal::disjunction(inner) => { all_branches.extend(inner); }
                        other => { all_branches.push(vec![other]); }
                    }
                } else {
                    all_branches.push(lits);
                }
            }
            Ok(vec![literal::disjunction(all_branches)])
        }
        other => Ok(vec![formula_type_to_literal(other)?]),
    }
}

/// Extracts a [`term`] from a `FormulaType::Term`. Returns an error for any other variant.
fn formula_type_to_term(ft: FormulaType) -> Result<term> {
    match ft {
        FormulaType::Term(t) => Ok(t),
        _ => anyhow::bail!("expected a term (variable or constant), found a compound formula"),
    }
}

/// Converts a single (non-conjunction) `FormulaType` leaf into a syntalog [`literal`].
///
/// - `Relation(r, terms)` → positive literal with predicate `r` applied to `terms`.
/// - `Term(constant)` → zero-arity positive literal.
/// - `Combination(¬, [inner])` → negative literal wrapping the inner atom.
/// - `Combination(==, [lhs, rhs])` → equality literal.
fn formula_type_to_literal(ft: FormulaType) -> Result<literal> {
    match ft {
        // Positive: relation atom, e.g. happy(A)
        FormulaType::Relation(rel_sym, terms) => {
            let pred = predicate_symbol(rel_sym.0);
            let a = atom::new(pred, terms)?;
            Ok(literal::positive_literal(a))
        }
        // Positive: zero-arity constant used as a propositional atom, e.g. "sunny"
        FormulaType::Term(t) => match t.term_type {
            TermType::Constant(c) => {
                let pred = predicate_symbol::new(c.0.symbol, 0)?;
                let a = atom::new(pred, vec![])?;
                Ok(literal::positive_literal(a))
            }
            _ => anyhow::bail!("individual variables cannot appear as bare literals in a rule"),
        },
        // Negative: ¬atom
        FormulaType::Combination(sym, mut parts)
            if sym.symbol() == "\u{00AC}" && parts.len() == 1 =>
        {
            let inner = parts.remove(0);
            match inner.formula_type {
                FormulaType::Relation(rel_sym, terms) => {
                    let pred = predicate_symbol(rel_sym.0);
                    Ok(literal::negative(pred, terms)?)
                }
                FormulaType::Term(t) => match t.term_type {
                    TermType::Constant(c) => {
                        let pred = predicate_symbol::new(c.0.symbol, 0)?;
                        Ok(literal::negative(pred, vec![])?)
                    }
                    _ => anyhow::bail!("individual variables cannot appear as bare literals in a rule"),
                },
                _ => anyhow::bail!("negation (¬) must wrap an atom, not a compound formula"),
            }
        }
        // Equality: lhs==rhs
        FormulaType::Combination(sym, mut parts)
            if sym.symbol() == "==" && parts.len() == 2 =>
        {
            let rhs = formula_type_to_term(parts.remove(1).formula_type)?;
            let lhs = formula_type_to_term(parts.remove(0).formula_type)?;
            Ok(literal::equality_literal(lhs, rhs))
        }
        _ => anyhow::bail!("expected an atom or negated atom, found a compound formula"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuleType;

    #[test]
    fn parse_definite_clause() {
        let r = parse_rule("happy(A) :- lego_builder(A), enjoys_lego(A)").unwrap();
        assert!(matches!(r.rule_type, RuleType::General));
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 2);
        assert_eq!(r.to_string(), "happy(A) :- lego_builder(A), enjoys_lego(A)");
    }

    #[test]
    fn parse_unit_clause() {
        let r = parse_rule("loves(alice, bob)").unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.to_string(), "loves(alice, bob)");
    }

    #[test]
    fn parse_goal() {
        let r = parse_rule(":- happy(A), lego_builder(A)").unwrap();
        assert!(matches!(r.rule_type, RuleType::Goal));
        assert_eq!(r.to_string(), ":- happy(A), lego_builder(A)");
    }

    #[test]
    fn parse_negative_literal_not() {
        let r = parse_rule("safe(A) :- not dangerous(A)").unwrap();
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.body[0], literal::negative_literal(_, _)));
    }

    #[test]
    fn parse_negative_literal_negation_symbol() {
        let r = parse_rule("safe(A) :- ¬dangerous(A)").unwrap();
        assert!(matches!(r.body[0], literal::negative_literal(_, _)));
    }

    #[test]
    fn parse_negative_literal_dash() {
        let r = parse_rule("safe(A) :- -dangerous(A)").unwrap();
        assert!(matches!(r.body[0], literal::negative_literal(_, _)));
    }

    #[test]
    fn parse_multiple_head_literals() {
        let r = parse_rule("p(A), q(A) :- r(A)").unwrap();
        assert_eq!(r.head.len(), 2);
        assert_eq!(r.body.len(), 1);
    }

    #[test]
    fn parse_invalid_uppercase_predicate_is_err() {
        assert!(parse_rule("Happy(A) :- lego_builder(A)").is_err());
    }

    // ── parse_formula_as_rule ────────────────────────────────────────────────

    #[test]
    fn formula_single_body_single_head() {
        // (b(X)) => (h(X))  ≡  h(X) :- b(X)
        let r = parse_formula_as_rule("(b(X)) => (h(X))").unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.rule_type, RuleType::General));
        assert_eq!(r.to_string(), "h(X) :- b(X)");
    }

    #[test]
    fn formula_conjunction_body_single_head() {
        // (lego_builder(A) and enjoys_lego(A)) => (happy(A))
        let r = parse_formula_as_rule(
            "(lego_builder(A) and enjoys_lego(A)) => (happy(A))"
        ).unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 2);
        assert_eq!(r.to_string(), "happy(A) :- lego_builder(A), enjoys_lego(A)");
    }

    #[test]
    fn formula_conjunction_body_conjunction_head() {
        // (b1(X) and b2(X)) => (h1(X) and h2(X))
        let r = parse_formula_as_rule(
            "(b1(X) and b2(X)) => (h1(X) and h2(X))"
        ).unwrap();
        assert_eq!(r.head.len(), 2);
        assert_eq!(r.body.len(), 2);
    }

    #[test]
    fn formula_nested_conjunction_is_flattened() {
        // ((b1(X) and b2(X)) and b3(X)) => h(X)  →  body has 3 literals
        let r = parse_formula_as_rule(
            "((b1(X) and b2(X)) and b3(X)) => (h(X))"
        ).unwrap();
        assert_eq!(r.body.len(), 3);
        assert_eq!(r.head.len(), 1);
    }

    #[test]
    fn formula_not_an_implication_is_err() {
        assert!(parse_formula_as_rule("(A and B)").is_err());
    }

    #[test]
    fn formula_negated_body_literal() {
        // (¬dangerous(A)) => (safe(A))
        let r = parse_formula_as_rule("(¬dangerous(A)) => (safe(A))").unwrap();
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.body[0], literal::negative_literal(_, _)));
        assert_eq!(r.to_string(), "safe(A) :- ¬dangerous(A)");
    }

    #[test]
    fn formula_disjunction_with_equality_and_multichar_variable() {
        let r = parse_formula_as_rule(
            "(Day==saturday or Day==sunday) => is_weekend(Day)"
        ).unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.body[0], literal::disjunction(_)));
        assert_eq!(r.to_string(), "is_weekend(Day) :- (Day = saturday ; Day = sunday)");
    }
}
