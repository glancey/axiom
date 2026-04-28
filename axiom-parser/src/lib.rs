use anyhow::Result;
use formalisms::{
    individual_variable, logical_symbol, operation_symbol, individual_constant,
    relation_symbol, operation, term, Formula, FormulaType, TermType,
};

/// Parses a string into a [`FormulaType`].
///
/// # Grammar (precedence, lowest → highest)
/// ```text
/// formula    := iff
/// iff        := implies (('<->'|'=') implies)*
/// implies    := or ('->' implies)?
/// or         := and ('∨' and)*
/// and        := negation ('∧' negation)*
/// negation   := '¬' negation | primary
/// primary    := quantifier | '(' formula ')' | atomic
/// quantifier := ('∀' | 'Ǝ') variable '.' formula
/// atomic     := variable | relation | constant
/// relation   := name '(' term (',' term)* ')'
/// term       := variable | constant | operation
/// operation  := name '(' term (',' term)* ')'
/// constant   := name
/// variable   := [A-Z] (alnum | '_' | '\'')*
/// name       := [a-z] (alnum | '_')*
/// ```
pub fn parse_formula(s: &str) -> Result<FormulaType> {
    let s = s.trim();
    let s = s.replace("for all", "\u{2200}");
    let s = s.replace("there exists", "\u{018E}");
    let s = s.replace(" and ", " \u{2227} ");
    let s = s.replace(" or ", " \u{2228} ");
    let s = s.replace(" not ", "\u{00AC}");
    let s = s.trim().to_string();
    let mut p = Parser::new(&s);
    let ft = p.formula()?;
    p.skip_ws();
    if p.pos < p.input.len() {
        anyhow::bail!("unexpected input at position {}: {:?}", p.pos, &p.input[p.pos..]);
    }
    Ok(ft)
}

/// Recursive-descent parser that consumes a formula string character by character.
/// The struct and its cursor methods are public so that `axiom-syntalog` can reuse
/// the shared term/name/variable parsing without duplicating the implementation.
pub struct Parser {
    input: String,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Parser { input: input.to_string(), pos: 0 }
    }

    /// Current byte offset into the input.
    pub fn pos(&self) -> usize { self.pos }

    /// Returns `true` when all input has been consumed.
    pub fn is_done(&self) -> bool { self.pos >= self.input.len() }

    /// Advances the cursor by `n` bytes.
    pub fn advance(&mut self, n: usize) { self.pos += n; }

    /// Returns the unparsed remainder of the input.
    pub fn rest(&self) -> &str {
        &self.input[self.pos..]
    }

    /// Advances `pos` past any leading whitespace.
    pub fn skip_ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c.is_whitespace() { self.pos += c.len_utf8(); } else { break; }
        }
    }

    /// Skips whitespace then expects and consumes the literal string `s`, returning an
    /// error if it is not present.
    pub fn consume(&mut self, s: &str) -> Result<()> {
        self.skip_ws();
        if self.rest().starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else {
            anyhow::bail!("expected {:?} at position {}", s, self.pos)
        }
    }

    /// Top-level formula entry point. Precedence (lowest → highest):
    /// `<->` / `=`  →  `->`  →  `∨`  →  `∧`  →  `¬`  →  primary.
    fn formula(&mut self) -> Result<FormulaType> {
        self.parse_iff()
    }

    fn parse_iff(&mut self) -> Result<FormulaType> {
        let lhs = self.parse_implies()?;
        self.skip_ws();
        let saved = self.pos;
        for op in &["<->", "="] {
            if self.rest().starts_with(op) {
                self.pos += op.len();
                let rhs = self.parse_iff()?;
                let sym = logical_symbol::new(op.to_string())?;
                return Ok(FormulaType::Combination(sym, vec![
                    Formula { formula_type: lhs, value: None },
                    Formula { formula_type: rhs, value: None },
                ]));
            }
        }
        self.pos = saved;
        Ok(lhs)
    }

    fn parse_implies(&mut self) -> Result<FormulaType> {
        let lhs = self.parse_or()?;
        self.skip_ws();
        let saved = self.pos;
        if self.rest().starts_with("->") {
            self.pos += 2;
            let rhs = self.parse_implies()?;
            let sym = logical_symbol::new("->".to_string())?;
            return Ok(FormulaType::Combination(sym, vec![
                Formula { formula_type: lhs, value: None },
                Formula { formula_type: rhs, value: None },
            ]));
        }
        self.pos = saved;
        Ok(lhs)
    }

    fn parse_or(&mut self) -> Result<FormulaType> {
        let mut lhs = self.parse_and()?;
        loop {
            self.skip_ws();
            let saved = self.pos;
            if self.rest().starts_with('\u{2228}') {
                self.pos += '\u{2228}'.len_utf8();
                let rhs = self.parse_and()?;
                let sym = logical_symbol::new("\u{2228}".to_string())?;
                lhs = FormulaType::Combination(sym, vec![
                    Formula { formula_type: lhs, value: None },
                    Formula { formula_type: rhs, value: None },
                ]);
            } else {
                self.pos = saved;
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<FormulaType> {
        let mut lhs = self.parse_negation()?;
        loop {
            self.skip_ws();
            let saved = self.pos;
            if self.rest().starts_with('\u{2227}') {
                self.pos += '\u{2227}'.len_utf8();
                let rhs = self.parse_negation()?;
                let sym = logical_symbol::new("\u{2227}".to_string())?;
                lhs = FormulaType::Combination(sym, vec![
                    Formula { formula_type: lhs, value: None },
                    Formula { formula_type: rhs, value: None },
                ]);
            } else {
                self.pos = saved;
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_negation(&mut self) -> Result<FormulaType> {
        self.skip_ws();
        if self.rest().starts_with('\u{00AC}') {
            self.pos += '\u{00AC}'.len_utf8();
            let sym = logical_symbol::new("\u{00AC}".to_string())?;
            let body = Formula { formula_type: self.parse_negation()?, value: None };
            return Ok(FormulaType::Combination(sym, vec![body]));
        }
        self.formula_primary()
    }

    /// Parses a primary: quantifier, grouped `( formula )`, or atomic.
    fn formula_primary(&mut self) -> Result<FormulaType> {
        self.skip_ws();
        if self.rest().starts_with('\u{2200}') || self.rest().starts_with('\u{018E}') {
            return self.quantifier();
        }
        if self.rest().starts_with('(') {
            return self.grouped();
        }
        self.atomic()
    }

    /// Parses a quantified formula: `(∀ | Ǝ) variable . formula`.
    fn quantifier(&mut self) -> Result<FormulaType> {
        let q = if self.rest().starts_with('\u{2200}') {
            self.pos += '\u{2200}'.len_utf8(); "\u{2200}"
        } else {
            self.pos += '\u{018E}'.len_utf8(); "\u{018E}"
        };
        let sym = logical_symbol::new(q.to_string())?;
        let var = self.individual_variable()?;
        self.consume(".")?;
        let body = Formula { formula_type: self.formula()?, value: None };
        Ok(FormulaType::Quantifier(sym, var, Box::new(body)))
    }

    /// Parses a parenthesised formula: `( formula )`. The inner formula may itself
    /// contain connectives; parentheses serve only for grouping.
    fn grouped(&mut self) -> Result<FormulaType> {
        self.consume("(")?;
        let ft = self.formula()?;
        self.consume(")")?;
        Ok(ft)
    }

/// Parses an atomic formula: an individual variable, a relation application
    /// `name(term, ...)`, or an individual constant.
    ///
    /// Variables are distinguished from relation/constant names by beginning with an
    /// uppercase ASCII letter.  A trailing `(` after a variable-like token signals that
    /// it is actually a relation name, so the position is reset and the name is re-parsed.
    fn atomic(&mut self) -> Result<FormulaType> {
        self.skip_ws();
        if let Some(v) = self.try_parse_variable()? {
            return Ok(FormulaType::Term(term { term_type: TermType::Variable(v) }));
        }
        let name = self.name()?;
        self.skip_ws();
        if self.rest().starts_with('(') {
            self.consume("(")?;
            let args = self.term_list()?;
            self.consume(")")?;
            let rel = relation_symbol::new(name, args.len() as u32)?;
            Ok(FormulaType::Relation(rel, args))
        } else {
            let c = individual_constant::new(name)?;
            Ok(FormulaType::Term(term { term_type: TermType::Constant(c) }))
        }
    }

    /// Parses a comma-separated list of terms, stopping before a closing `)`.
    pub fn term_list(&mut self) -> Result<Vec<term>> {
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

    /// Parses a single term: a variable (`[A-Z]'*`), an operation `name(term, ...)`,
    /// or a constant (`name`).
    pub fn term(&mut self) -> Result<term> {
        self.skip_ws();
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

    /// Tries to parse a variable token (uppercase letter + optional primes) not followed by `(`.
    /// Returns `Ok(Some(v))` and advances `pos` on success.
    /// Returns `Ok(None)` without advancing if the first char is not uppercase or if the token
    /// is followed by `(` (which signals a relation/operation name instead).
    pub fn try_parse_variable(&mut self) -> Result<Option<individual_variable>> {
        let start = self.pos;
        if let Some(c) = self.rest().chars().next()
            && c.is_ascii_uppercase() {
                self.pos += c.len_utf8();
                while let Some(c) = self.rest().chars().next() {
                    if c.is_alphanumeric() || c == '_' || c == '\'' {
                        self.pos += c.len_utf8();
                    } else {
                        break;
                    }
                }
                let s = self.input[start..self.pos].to_string();
                self.skip_ws();
                if !self.rest().starts_with('(') {
                    let v = individual_variable::new(&s)?;
                    return Ok(Some(v));
                }
                self.pos = start;
        }
        Ok(None)
    }

    fn individual_variable(&mut self) -> Result<individual_variable> {
        self.skip_ws();
        let pos = self.pos;
        self.try_parse_variable()?
            .ok_or_else(|| anyhow::anyhow!("expected individual variable at position {}", pos))
    }

    /// Parses a name token: a lowercase ASCII letter followed by zero or more
    /// alphanumeric characters or underscores.
    pub fn name(&mut self) -> Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_formula_individual_variable() {
        assert!(matches!(parse_formula("A"), Ok(FormulaType::Term(_))));
        assert!(matches!(parse_formula("B'"), Ok(FormulaType::Term(_))));
        assert!(matches!(parse_formula("Z'''"), Ok(FormulaType::Term(_))));
    }

    #[test]
    fn parse_formula_individual_constant() {
        assert!(matches!(parse_formula("foo"), Ok(FormulaType::Term(_))));
        assert!(matches!(parse_formula("c1"), Ok(FormulaType::Term(_))));
    }

    #[test]
    fn parse_formula_relation() {
        assert!(matches!(parse_formula("r(A)"), Ok(FormulaType::Relation(_, _))));
        assert!(matches!(parse_formula("rel(A, B)"), Ok(FormulaType::Relation(_, _))));
    }

    #[test]
    fn parse_formula_negation() {
        assert!(matches!(parse_formula("¬A"), Ok(FormulaType::Combination(_, _))));
    }

    #[test]
    fn parse_formula_combination() {
        assert!(matches!(parse_formula("(A ∧ B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A ∨ B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A -> B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A <-> B)"), Ok(FormulaType::Combination(_, _))));
    }

    #[test]
    fn parse_formula_quantifier() {
        assert!(matches!(parse_formula("∀X.A"), Ok(FormulaType::Quantifier(_, _, _))));
        assert!(matches!(parse_formula("ƎX.A"), Ok(FormulaType::Quantifier(_, _, _))));
    }

    #[test]
    fn parse_formula_nested() {
        assert!(matches!(parse_formula("(¬A -> B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("∀X.(A ∧ B)"), Ok(FormulaType::Quantifier(_, _, _))));
    }

    #[test]
    fn parse_formula_invalid() {
        assert!(parse_formula("").is_err());
        assert!(parse_formula("123").is_err());
    }

    #[test]
    fn parse_formula_grouped() {
        assert!(parse_formula("(A)").is_ok());
    }

    #[test]
    fn parse_formula_bare_connective() {
        assert!(matches!(parse_formula("P->Q"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("A -> B"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A ∨ B) -> C"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(
            parse_formula("(p(X) or (q(A) and r(X, A))) -> s(X)"),
            Ok(FormulaType::Combination(_, _))
        ));
    }

    #[test]
    fn parse_formula_precedence() {
        // `or` binds tighter than `->`, so this must parse as (A ∨ B) -> C, not A ∨ (B -> C)
        let f = parse_formula("A or B -> C").unwrap();
        // top-level connective must be `->`
        assert!(matches!(f, FormulaType::Combination(_, _)));
        if let FormulaType::Combination(sym, args) = &f {
            assert_eq!(sym.symbol(), "->", "top connective should be ->");
            // lhs should be A ∨ B (a Combination)
            assert!(matches!(args[0].formula_type, FormulaType::Combination(_, _)));
        }
        // bare implies without parens: no wrapping needed
        assert!(matches!(
            parse_formula("tweets(X, B) or (tweets(A, B) and follows(X, A)) -> receives(X, B)"),
            Ok(FormulaType::Combination(_, _))
        ));
    }
}
