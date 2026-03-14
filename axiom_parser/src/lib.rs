use anyhow::Result;
use formalisms::{
    individual_variable, logical_symbol, operation_symbol, individual_constant,
    relation_symbol, operation, term, Formula, FormulaType, TermType,
};

/// Parses a string into a [`FormulaType`].
///
/// # Grammar
/// ```text
/// formula  := quantifier | negation | combination | atomic
/// quantifier := ('∀' | 'Ǝ') variable '.' formula
/// negation   := '~' formula
/// combination := '(' formula ('∧'|'∨'|'/\'|'\/'|'=>'|'<=>'|'==') formula ')'
/// atomic     := variable | relation | constant
/// relation   := name '(' term (',' term)* ')'
/// term       := variable | constant | operation
/// operation  := name '(' term (',' term)* ')'
/// constant   := name
/// variable   := [A-Z] '\''*
/// name       := [a-z][a-zA-Z0-9_]*
/// ```
pub fn parse_formula(s: &str) -> Result<FormulaType> {
    let s = s.trim();
    let s = s.replace("for all", "\u{2200}");
    let s = s.replace("there exists", "\u{018E}");
    let s = s.replace(" and ", " \u{2227} ");
    let s = s.replace(" or ", " \u{2228} ");
    let s = s.replace(" -", " ~");
    let s = s.trim().to_string();
    let s = s.as_str();
    let normalized = if s.starts_with('(')
        || s.starts_with('~')
        || s.starts_with('\u{2200}')
        || s.starts_with('\u{018E}')
    {
        s.to_string()
    } else {
        format!("({s})")
    };
    let mut p = Parser::new(&normalized);
    let ft = p.formula()?;
    p.skip_ws();
    if p.pos < p.input.len() {
        anyhow::bail!("unexpected input at position {}: {:?}", p.pos, &p.input[p.pos..]);
    }
    Ok(ft)
}

struct Parser {
    input: String,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser { input: input.to_string(), pos: 0 }
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

    fn formula(&mut self) -> Result<FormulaType> {
        self.skip_ws();
        if self.rest().starts_with('\u{2200}') || self.rest().starts_with('\u{018E}') {
            return self.quantifier();
        }
        if self.rest().starts_with('~') {
            self.pos += 1;
            let sym = logical_symbol::new("~".to_string())?;
            let body = Formula { formula_type: self.formula()?, value: None };
            return Ok(FormulaType::Combination(sym, vec![body]));
        }
        if self.rest().starts_with('(') {
            return self.combination();
        }
        self.atomic()
    }

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

    fn combination(&mut self) -> Result<FormulaType> {
        self.consume("(")?;
        let lhs = self.formula()?;
        self.skip_ws();
        if self.rest().starts_with(')') {
            self.pos += 1;
            return Ok(lhs);
        }
        let op = self.binary_connective()?;
        let rhs = Formula { formula_type: self.formula()?, value: None };
        self.consume(")")?;
        let sym = logical_symbol::new(op)?;
        Ok(FormulaType::Combination(sym, vec![
            Formula { formula_type: lhs, value: None },
            rhs,
        ]))
    }

    fn binary_connective(&mut self) -> Result<String> {
        self.skip_ws();
        for op in &["<=>", "=>", "\u{2227}", "\u{2228}", "=="] {
            if self.rest().starts_with(op) {
                self.pos += op.len();
                return Ok(op.to_string());
            }
        }
        anyhow::bail!("expected connective at position {}", self.pos)
    }

    fn atomic(&mut self) -> Result<FormulaType> {
        self.skip_ws();
        let start = self.pos;
        if let Some(c) = self.rest().chars().next() {
            if c.is_ascii_uppercase() {
                self.pos += c.len_utf8();
                while self.rest().starts_with('\'') { self.pos += 1; }
                let s = self.input[start..self.pos].to_string();
                self.skip_ws();
                if !self.rest().starts_with('(') {
                    let v = individual_variable::new(&s)?;
                    return Ok(FormulaType::Term(term { term_type: TermType::Variable(v) }));
                }
                self.pos = start;
            }
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
        let start = self.pos;
        if let Some(c) = self.rest().chars().next() {
            if c.is_ascii_uppercase() {
                self.pos += c.len_utf8();
                while self.rest().starts_with('\'') { self.pos += 1; }
                let s = self.input[start..self.pos].to_string();
                self.skip_ws();
                if !self.rest().starts_with('(') {
                    let v = individual_variable::new(&s)?;
                    return Ok(term { term_type: TermType::Variable(v) });
                }
                self.pos = start;
            }
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

    fn individual_variable(&mut self) -> Result<individual_variable> {
        self.skip_ws();
        let start = self.pos;
        match self.rest().chars().next() {
            Some(c) if c.is_ascii_uppercase() => {
                self.pos += c.len_utf8();
                while self.rest().starts_with('\'') { self.pos += 1; }
                individual_variable::new(&self.input[start..self.pos])
            }
            _ => anyhow::bail!("expected individual variable at position {}", self.pos),
        }
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
        assert!(matches!(parse_formula("~A"), Ok(FormulaType::Combination(_, _))));
    }

    #[test]
    fn parse_formula_combination() {
        assert!(matches!(parse_formula("(A ∧ B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A ∨ B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A => B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("(A <=> B)"), Ok(FormulaType::Combination(_, _))));
    }

    #[test]
    fn parse_formula_quantifier() {
        assert!(matches!(parse_formula("∀X.A"), Ok(FormulaType::Quantifier(_, _, _))));
        assert!(matches!(parse_formula("ƎX.A"), Ok(FormulaType::Quantifier(_, _, _))));
    }

    #[test]
    fn parse_formula_nested() {
        assert!(matches!(parse_formula("(~A => B)"), Ok(FormulaType::Combination(_, _))));
        assert!(matches!(parse_formula("∀X.(A ∧ B)"), Ok(FormulaType::Quantifier(_, _, _))));
    }

    #[test]
    fn parse_formula_invalid() {
        assert!(parse_formula("").is_err());
        assert!(parse_formula("ABC").is_err());
    }

    #[test]
    fn parse_formula_auto_parenthesized() {
        assert!(parse_formula("(A)").is_ok());
        assert!(parse_formula("P=>Q").is_ok());
    }

    #[test]
    fn parse_formula_implication_p_implies_q() {
        assert!(matches!(parse_formula("P=>Q"), Ok(FormulaType::Combination(_, _))));
    }
}
