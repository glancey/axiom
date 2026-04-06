use anyhow::Result;
use formalisms::{individual_variable, individual_constant, operation_symbol, operation, term, TermType};
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
/// term         := variable | name '(' term_list ')' | name
/// variable     := [A-Z] '\''*
/// name         := [a-z][a-zA-Z0-9_]*
/// ```
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
            return Ok(rule::new(head, body));
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
            if self.rest().starts_with(',') { self.pos += 1; } else { break; }
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
}
