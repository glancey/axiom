use anyhow::Result;
use axiom_syntalog::{is_ground, literal, parse_rule, rule, RuleType};
use formalisms::individual_variable;
use std::fs;
use std::path::Path;

pub fn read_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}

/// Parses `input` as a unit clause rule (atom or literal).
/// Returns an error if the input is already expressed as a rule (contains `:-`).
pub fn induce_rule(input: &str) -> Result<rule> {
    let s = input.trim();
    if s.contains(":-") {
        anyhow::bail!("input is already a rule; induce requires an atom or literal");
    }
    let r = parse_rule(s)?;
    match r.rule_type {
        RuleType::UnitClause => {
            if r.head.iter().all(|lit| lit.is_ground()) {
                rule::fact(r.head)
            } else {
                Ok(r)
            }
        }
        RuleType::Fact => Ok(r),
        RuleType::Goal => anyhow::bail!("input is a goal; induce requires an atom or literal"),
        _ => anyhow::bail!("input is a rule; induce requires an atom or literal"),
    }
}

pub fn classify_line(line: &str) -> Option<String> {
    let s = line.trim();
    if s.is_empty() || s.starts_with('%') {
        return None;
    }

    // A bare uppercase token (with optional apostrophes) is an individual_variable — a term.
    let looks_like_variable = s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && s.chars().all(|c| c.is_ascii_uppercase() || c == '\'');
    if looks_like_variable {
        if let Ok(v) = individual_variable::new(s) {
            return Some(format!("term (individual_variable): {}", v.name));
        }
    }

    let desc = match parse_rule(s) {
        Err(e) => format!("parse error: {e}"),
        Ok(r) => match r.rule_type {
            RuleType::Goal => format!("rule (goal): {r}"),
            RuleType::General | RuleType::DefiniteClause | RuleType::HornRule => {
                format!("rule: {r}")
            }
            RuleType::UnitClause | RuleType::Fact => match r.head.as_slice() {
                [literal::negative_literal(_, _)] => format!("literal: {}", r.head[0]),
                [literal::positive_literal(a)] if a.terms.is_empty() => {
                    format!("predicate_symbol / atom: {a}")
                }
                [literal::positive_literal(a)] => format!("atom: {a}"),
                _ => format!("rule (unit clause): {r}"),
            },
        },
    };
    Some(desc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn induce_atom_produces_unit_clause() {
        let r = induce_rule("happy(A)").unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.head.len(), 1);
        assert!(r.body.is_empty());
        assert_eq!(r.to_string(), "happy(A)");
    }

    #[test]
    fn induce_negative_literal_produces_unit_clause() {
        let r = induce_rule("¬dangerous(A)").unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.head.len(), 1);
    }

    #[test]
    fn induce_ground_atom_produces_fact_with_value_true() {
        let r = induce_rule("mild(tandoori)").unwrap();
        assert!(matches!(r.rule_type, RuleType::Fact));
        assert_eq!(r.to_string(), "mild(tandoori)");
        let formula = r.to_formula().unwrap();
        assert_eq!(formula.value, Some(true));
    }

    #[test]
    fn induce_rule_input_is_error() {
        assert!(induce_rule("happy(A) :- lego(A)").is_err());
    }

    #[test]
    fn induce_goal_input_is_error() {
        assert!(induce_rule(":- happy(A)").is_err());
    }
}
