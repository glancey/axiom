use anyhow::Result;
use formalisms::{individual_variable, individual_constant, operation_symbol, operation, term, TermType, FormulaType};
use crate::{predicate_symbol, atom, literal, rule};

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
/// term         := variable | list | name '(' term_list ')' | name
/// list         := '[' ']' | '[' term (',' term)* ']'
/// variable     := [A-Z] '\''*
/// name         := [a-z][a-zA-Z0-9_]*
/// ```
///
/// List syntax `[t1, …, tn]` desugars to Prolog dot notation:
/// `'.'(t1, '.'(…, '.'(tn, [])))`.  The empty list `[]` becomes the
/// individual constant `[]`.
/// Parses a string as a single [`term`].
///
/// Accepts variables (`X`), constants (`alice`), operations (`f(a, b)`),
/// and list syntax (`[a, b, c]`, `[]`).
pub fn parse_term(s: &str) -> Result<formalisms::term> {
    let mut p = Parser::new(s);
    let t = p.term()?;
    p.skip_ws();
    if p.pos < p.input.len() {
        anyhow::bail!("unexpected input at position {}: {:?}", p.pos, &p.input[p.pos..]);
    }
    Ok(t)
}

pub fn parse_rule(s: &str) -> Result<rule> {
    let mut p = Parser::new(s);
    let r = p.rule()?;
    p.skip_ws();
    if p.pos < p.input.len() {
        anyhow::bail!("unexpected input at position {}: {:?}", p.pos, &p.input[p.pos..]);
    }
    Ok(r)
}

struct Parser {
    input: String,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser { input: input.trim().to_string(), pos: 0 }
    }

    fn rest(&self) -> &str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c.is_whitespace() { self.pos += c.len_utf8(); } else { break; }
        }
    }

    fn consume(&mut self, s: &str) -> Result<()> {
        self.skip_ws();
        if self.rest().starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else {
            anyhow::bail!("expected {:?} at position {}", s, self.pos)
        }
    }

    fn rule(&mut self) -> Result<rule> {
        self.skip_ws();
        // Goal: starts with ':-'
        if self.rest().starts_with(":-") {
            self.pos += 2;
            let body = self.literal_list()?;
            return rule::goal(body);
        }
        let head = self.literal_list()?;
        self.skip_ws();
        if self.rest().starts_with(":-") {
            self.pos += 2;
            let body = self.literal_list()?;
            return rule::new(head, body);
        }
        // Unit clause: head only
        rule::unit_clause(head)
    }

    fn literal_list(&mut self) -> Result<Vec<literal>> {
        let mut lits = Vec::new();
        loop {
            self.skip_ws();
            if self.rest().is_empty() || self.rest().starts_with(":-") { break; }
            lits.push(self.literal()?);
            self.skip_ws();
            if self.rest().starts_with(',') || self.rest().starts_with('.') { self.pos += 1; } else { break; }
        }
        Ok(lits)
    }

    fn literal(&mut self) -> Result<literal> {
        self.skip_ws();
        // Negation: ¬, -, or 'not' followed by whitespace
        let negated =
            if self.rest().starts_with('¬') {
                self.pos += '¬'.len_utf8();
                true
            } else if self.rest().starts_with('-') {
                self.pos += 1;
                true
            } else if self.rest().starts_with("not")
                && self.rest()[3..].starts_with(|c: char| c.is_whitespace() || c == '(') {
                self.pos += 3;
                true
            } else {
                false
            };

        let a = self.atom()?;
        if negated {
            literal::negative(a.predicate, a.terms)
        } else {
            Ok(literal::positive_literal(a))
        }
    }

    fn atom(&mut self) -> Result<atom> {
        self.skip_ws();
        let name = self.name()?;
        self.skip_ws();
        let terms = if self.rest().starts_with('(') {
            self.consume("(")?;
            let ts = self.term_list()?;
            self.consume(")")?;
            ts
        } else {
            vec![]
        };
        let rank = terms.len() as u32;
        let pred = predicate_symbol::new(name, rank)?;
        atom::new(pred, terms)
    }

    fn term_list(&mut self) -> Result<Vec<term>> {
        let mut terms = Vec::new();
        loop {
            self.skip_ws();
            if self.rest().starts_with(')') { break; }
            terms.push(self.term()?);
            self.skip_ws();
            if self.rest().starts_with(',') { self.pos += 1; } else { break; }
        }
        Ok(terms)
    }

    fn term(&mut self) -> Result<term> {
        self.skip_ws();
        if self.rest().starts_with('[') {
            return self.list_term();
        }
        if let Some(v) = self.try_parse_variable()? {
            return Ok(term { term_type: TermType::Variable(v) });
        }
        let name = self.name()?;
        self.skip_ws();
        if self.rest().starts_with('(') {
            self.consume("(")?;
            let vars = self.term_list()?;
            self.consume(")")?;
            let sym = operation_symbol::new(name, vars.len() as u32)?;
            let op = operation::new(sym, vars)?;
            Ok(term { term_type: TermType::Operation(op) })
        } else {
            let c = individual_constant::new(name)?;
            Ok(term { term_type: TermType::Constant(c) })
        }
    }

    /// Parses `[t1, …, tn]` and desugars to `'.'(t1, '.'(…, '.'(tn, [])))`.
    /// The empty list `[]` becomes the individual constant `[]`.
    fn list_term(&mut self) -> Result<term> {
        self.consume("[")?;
        self.skip_ws();
        if self.rest().starts_with(']') {
            self.pos += 1;
            let c = individual_constant::new("[]".to_string())?;
            return Ok(term { term_type: TermType::Constant(c) });
        }
        let mut elems = Vec::new();
        loop {
            elems.push(self.term()?);
            self.skip_ws();
            if self.rest().starts_with(',') {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.consume("]")?;
        // Build '.'(t1, '.'(t2, ...'.'(tn, [])...)) from right to left.
        let nil = {
            let c = individual_constant::new("[]".to_string())?;
            term { term_type: TermType::Constant(c) }
        };
        let mut result = nil;
        for elem in elems.into_iter().rev() {
            let sym = operation_symbol::new(".".to_string(), 2)?;
            let op = operation::new(sym, vec![elem, result])?;
            result = term { term_type: TermType::Operation(op) };
        }
        Ok(result)
    }

    fn try_parse_variable(&mut self) -> Result<Option<individual_variable>> {
        let start = self.pos;
        if let Some(c) = self.rest().chars().next() {
            if c.is_ascii_uppercase() {
                self.pos += c.len_utf8();
                while self.rest().starts_with('\'') { self.pos += 1; }
                let s = self.input[start..self.pos].to_string();
                self.skip_ws();
                if !self.rest().starts_with('(') {
                    return Ok(Some(individual_variable::new(&s)?));
                }
                self.pos = start;
            }
        }
        Ok(None)
    }

    fn name(&mut self) -> Result<String> {
        self.skip_ws();
        let start = self.pos;
        match self.rest().chars().next() {
            Some(c) if c.is_ascii_lowercase() => {
                self.pos += c.len_utf8();
                while let Some(c) = self.rest().chars().next() {
                    if c.is_alphanumeric() || c == '_' { self.pos += c.len_utf8(); } else { break; }
                }
                Ok(self.input[start..self.pos].to_string())
            }
            _ => anyhow::bail!("expected name at position {}", self.pos),
        }
    }
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
pub fn parse_formula_as_rule(s: &str) -> Result<rule> {
    // Wrap in outer parens so axiom_parser sees the top-level '=>' as a binary
    // connective rather than treating the first parenthesised sub-expression as
    // a complete formula with leftover input.
    let wrapped = format!("({})", s.trim());
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
/// A non-conjunction operand is converted directly via [`formula_type_to_literal`].
fn flatten_conjunction(ft: FormulaType) -> Result<Vec<literal>> {
    match ft {
        FormulaType::Combination(sym, parts) if sym.symbol() == "\u{2227}" => {
            let mut lits = Vec::new();
            for f in parts {
                lits.extend(flatten_conjunction(f.formula_type)?);
            }
            Ok(lits)
        }
        other => Ok(vec![formula_type_to_literal(other)?]),
    }
}

/// Converts a single (non-conjunction) `FormulaType` leaf into a syntalog [`literal`].
///
/// - `Relation(r, terms)` → positive literal with predicate `r` applied to `terms`.
/// - `Term(constant)` → zero-arity positive literal.
/// - `Combination(¬, [inner])` → negative literal wrapping the inner atom.
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
        assert_eq!(r.to_string(), "happy(A) :- lego_builder(A), enjoys_lego(A).");
    }

    #[test]
    fn parse_unit_clause() {
        let r = parse_rule("loves(alice, bob)").unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.to_string(), "loves(alice, bob).");
    }

    #[test]
    fn parse_goal() {
        let r = parse_rule(":- happy(A), lego_builder(A)").unwrap();
        assert!(matches!(r.rule_type, RuleType::Goal));
        assert_eq!(r.to_string(), ":- happy(A), lego_builder(A).");
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
        assert_eq!(r.to_string(), "h(X) :- b(X).");
    }

    #[test]
    fn formula_conjunction_body_single_head() {
        // (lego_builder(A) and enjoys_lego(A)) => (happy(A))
        let r = parse_formula_as_rule(
            "(lego_builder(A) and enjoys_lego(A)) => (happy(A))"
        ).unwrap();
        assert_eq!(r.head.len(), 1);
        assert_eq!(r.body.len(), 2);
        assert_eq!(r.to_string(), "happy(A) :- lego_builder(A), enjoys_lego(A).");
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

    // ── list term parsing ────────────────────────────────────────────────────

    #[test]
    fn parse_empty_list_as_term() {
        let r = parse_rule("p([])").unwrap();
        // p([]) is a unit clause with one term: the constant []
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        let t = &r.head[0];
        if let literal::positive_literal(a) = t {
            assert_eq!(a.terms.len(), 1);
            assert!(matches!(a.terms[0].term_type, TermType::Constant(_)));
        } else {
            panic!("expected positive literal");
        }
    }

    #[test]
    fn parse_singleton_list_as_term() {
        let r = parse_rule("p([a])").unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        if let literal::positive_literal(a) = &r.head[0] {
            assert_eq!(a.terms.len(), 1);
            // '.'(a, []) — top level should be an Operation with symbol "."
            assert!(matches!(a.terms[0].term_type, TermType::Operation(_)));
            if let TermType::Operation(op) = &a.terms[0].term_type {
                assert_eq!(op.symbol.symbol, ".");
                assert_eq!(op.vars.len(), 2);
                // first arg is constant 'a', second is constant '[]'
                assert!(matches!(op.vars[0].term_type, TermType::Constant(_)));
                assert!(matches!(op.vars[1].term_type, TermType::Constant(_)));
            }
        } else {
            panic!("expected positive literal");
        }
    }

    #[test]
    fn parse_multi_element_list_as_term() {
        // [a, b, c] → '.'(a, '.'(b, '.'(c, [])))
        let r = parse_rule("p([a, b, c])").unwrap();
        if let literal::positive_literal(a) = &r.head[0] {
            let mut cur = &a.terms[0];
            for expected in &["a", "b", "c"] {
                if let TermType::Operation(op) = &cur.term_type {
                    assert_eq!(op.symbol.symbol, ".");
                    if let TermType::Constant(c) = &op.vars[0].term_type {
                        assert_eq!(c.0.symbol, *expected);
                    } else {
                        panic!("expected constant {expected}");
                    }
                    cur = &op.vars[1];
                } else {
                    panic!("expected '.' operation");
                }
            }
            // tail should be []
            assert!(matches!(cur.term_type, TermType::Constant(_)));
            if let TermType::Constant(c) = &cur.term_type {
                assert_eq!(c.0.symbol, "[]");
            }
        } else {
            panic!("expected positive literal");
        }
    }

    #[test]
    fn parse_list_with_variables() {
        let r = parse_rule("p([X, Y])").unwrap();
        if let literal::positive_literal(a) = &r.head[0] {
            if let TermType::Operation(op) = &a.terms[0].term_type {
                assert_eq!(op.symbol.symbol, ".");
                assert!(matches!(op.vars[0].term_type, TermType::Variable(_)));
            } else {
                panic!("expected '.' operation");
            }
        }
    }

    #[test]
    fn parse_nested_list() {
        // [[a], b] → '.'('.'(a, []), '.'(b, []))
        let r = parse_rule("p([[a], b])").unwrap();
        if let literal::positive_literal(a) = &r.head[0] {
            if let TermType::Operation(outer) = &a.terms[0].term_type {
                assert_eq!(outer.symbol.symbol, ".");
                // first element is itself a '.' operation (the inner [a])
                assert!(matches!(outer.vars[0].term_type, TermType::Operation(_)));
            } else {
                panic!("expected outer '.' operation");
            }
        }
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
        assert_eq!(r.to_string(), "safe(A) :- ¬dangerous(A).");
    }
}
