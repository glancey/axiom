use anyhow::Result;
use formalisms::{term, TermType, FormulaType, operation_symbol};
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

/// Parses a formula string of the form `(b1 ∧ … ∧ bm) -> (h1 ∧ … ∧ hn)` into a [`rule`].
///
/// Uses [`axiom_parser::parse_formula`] to consume the formula, then converts the
/// top-level implication into `head :- body` form. Each conjunction side is
/// recursively flattened so that nested `∧` chains all become flat literal lists.
///
/// # Accepted connective spellings
/// The `axiom_parser` normalises `" and "` → `∧` before parsing, so both
/// `(b1 and b2) -> h1` and `(b1 ∧ b2) -> h1` are accepted.
///
/// # Errors
/// Returns an error if the formula is not a top-level `->` combination, or if
/// any operand cannot be converted to a syntalog literal.
/// Wraps each bare `lhs=rhs` equality in parentheses so the axiom parser
/// (which requires every binary combination to be explicitly parenthesised)
/// can parse them when they appear inside a disjunction or conjunction.
/// e.g. `Day==saturday or Day==sunday` → `(Day==saturday) or (Day==sunday)`.
fn is_standalone_eq(chars: &[char], i: usize) -> bool {
    let prev = i.checked_sub(1).map(|j| chars[j]).unwrap_or('\0');
    let next = chars.get(i + 1).copied().unwrap_or('\0');
    // Skip compound operators: ->, =>, <=, !=, >=, ==
    !(prev == '!' || prev == '<' || prev == '>' || prev == '-' || prev == '='
        || next == '>' || next == '=')
}

fn is_token_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

fn wrap_equalities(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result: Vec<char> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '=' && is_standalone_eq(&chars, i) {
            // Walk back in result past spaces to find end of lhs token
            let mut back = result.len();
            while back > 0 && result[back - 1] == ' ' { back -= 1; }
            // Walk back through the lhs token
            let mut lhs_start = back;
            while lhs_start > 0 && is_token_char(result[lhs_start - 1]) { lhs_start -= 1; }
            let prefix: Vec<char> = result[..lhs_start].to_vec();
            let lhs: String = result[lhs_start..back].iter().collect();
            result = prefix;
            // Walk forward past spaces after '=' to find rhs token
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' { j += 1; }
            let rhs_start = j;
            while j < chars.len() && is_token_char(chars[j]) { j += 1; }
            let rhs: String = chars[rhs_start..j].iter().collect();
            // Write wrapped equality
            result.push('(');
            result.extend(lhs.chars());
            result.extend(" = ".chars());
            result.extend(rhs.chars());
            result.push(')');
            i = j;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result.into_iter().collect()
}

pub fn parse_formula_as_rule(s: &str) -> Result<rule> {
    let preprocessed = wrap_equalities(s.trim());
    let ft = axiom_parser::parse_formula(&preprocessed)?;
    match ft {
        FormulaType::Combination(sym, mut parts)
            if sym.symbol() == "->" && parts.len() == 2 =>
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
            "formula must be a top-level implication of the form (body) -> (head)"
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
/// - `Combination(¬, [atom])` → negative literal wrapping the inner atom.
/// - `Combination(¬, [compound])` → `naf` literal (negation-as-failure of a conjunction).
/// - `Combination(=, [lhs, rhs])` → equality literal.
fn formula_type_to_literal(ft: FormulaType) -> Result<literal> {
    match ft {
        // Positive: relation atom, e.g. happy(A)
        FormulaType::Relation(rel_sym, terms) => {
            let pred = predicate_symbol(rel_sym.0);
            let a = atom::new(pred, terms)?;
            Ok(literal::positive_literal(a))
        }
        // Positive: zero-arity propositional atom — constant (e.g. "sunny") or variable (e.g. "P")
        FormulaType::Term(t) => match t.term_type {
            TermType::Constant(c) => {
                let pred = predicate_symbol::new(c.0.symbol, 0)?;
                let a = atom::new(pred, vec![])?;
                Ok(literal::positive_literal(a))
            }
            TermType::Variable(v) => {
                let pred = predicate_symbol(operation_symbol { symbol: v.name, rank: 0 });
                let a = atom::new(pred, vec![])?;
                Ok(literal::positive_literal(a))
            }
            _ => anyhow::bail!("unexpected term type as bare literal"),
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
                    TermType::Variable(v) => {
                        let pred = predicate_symbol(operation_symbol { symbol: v.name, rank: 0 });
                        Ok(literal::negative(pred, vec![])?)
                    }
                    _ => anyhow::bail!("unexpected term type as bare literal"),
                },
                compound => {
                    let lits = flatten_conjunction(compound)?;
                    Ok(literal::naf(lits))
                }
            }
        }
        // Equality: lhs=rhs
        FormulaType::Combination(sym, mut parts)
            if sym.symbol() == "=" && parts.len() == 2 =>
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
        // (b(X)) -> (h(X))  ≡  h(X) :- b(X)
        let r = parse_formula_as_rule("(b(X)) -> (h(X))").unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.rule_type, RuleType::General));
        assert_eq!(r.to_string(), "h(X) :- b(X)");
    }

    #[test]
    fn formula_conjunction_body_single_head() {
        // (lego_builder(A) and enjoys_lego(A)) -> (happy(A))
        let r = parse_formula_as_rule(
            "(lego_builder(A) and enjoys_lego(A)) -> (happy(A))"
        ).unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 2);
        assert_eq!(r.to_string(), "happy(A) :- lego_builder(A), enjoys_lego(A)");
    }

    #[test]
    fn formula_conjunction_body_conjunction_head() {
        // (b1(X) and b2(X)) -> (h1(X) and h2(X))
        let r = parse_formula_as_rule(
            "(b1(X) and b2(X)) -> (h1(X) and h2(X))"
        ).unwrap();
        assert_eq!(r.head.len(), 2);
        assert_eq!(r.body.len(), 2);
    }

    #[test]
    fn formula_nested_conjunction_is_flattened() {
        // ((b1(X) and b2(X)) and b3(X)) -> h(X)  →  body has 3 literals
        let r = parse_formula_as_rule(
            "((b1(X) and b2(X)) and b3(X)) -> (h(X))"
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
        // (¬dangerous(A)) -> (safe(A))
        let r = parse_formula_as_rule("(¬dangerous(A)) -> (safe(A))").unwrap();
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.body[0], literal::negative_literal(_, _)));
        assert_eq!(r.to_string(), "safe(A) :- ¬dangerous(A)");
    }

    #[test]
    fn formula_disjunction_with_equality_and_multichar_variable() {
        let r = parse_formula_as_rule(
            "(Day = saturday or Day = sunday) -> is_weekend(Day)"
        ).unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 1);
        assert!(matches!(r.body[0], literal::disjunction(_)));
        assert_eq!(r.to_string(), "is_weekend(Day) :- (Day = saturday ; Day = sunday)");
    }
}
