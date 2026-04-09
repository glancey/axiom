pub mod induction;
use anyhow::Result;
use axiom_syntalog::{is_ground, literal, parse_rule, rule, RuleType};
use formalisms::{individual_variable, term, Formula, TermType};
use formalisms::proofs::ProofTable;
use std::collections::HashSet;
use std::fs;
use std::path::Path;


pub fn read_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}

/// Returns the rule generalized by the given theory.
pub fn proof_table_for_rule(r: &rule, theory: &crate::induction::Theory) -> Result<rule> {
    theory.generalize(r)
}

/// Builds a `ProofTable` for the formula derived from `r`, with an explicit value.
pub fn proof_table_for_rule_valued(r: &rule, value: Option<bool>) -> Result<ProofTable> {
    let formula_type = r.to_formula()?.formula_type;
    let formula = Formula { formula_type, value };
    let mut table = ProofTable::new();
    formula.is_tautology(&mut table);
    Ok(table)
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

fn normalize_line(line: &str) -> &str {
    line.trim().trim_end_matches('.')
}

fn fmt_term(t: &term) -> String {
    match &t.term_type {
        TermType::Variable(v) => v.name.clone(),
        TermType::Constant(c) => c.0.symbol.clone(),
        TermType::Operation(op) => {
            let args: Vec<String> = op.vars.iter().map(fmt_term).collect();
            format!("{}({})", op.symbol.symbol, args.join(", "))
        }
    }
}

fn collect_ground_terms_from_term(t: &term, acc: &mut HashSet<String>) {
    if t.is_ground() {
        acc.insert(fmt_term(t));
    }
}

fn collect_ground_terms_from_literal(lit: &literal, acc: &mut HashSet<String>) {
    let terms = match lit {
        literal::positive_literal(a) => &a.terms,
        literal::negative_literal(_, terms) => terms,
    };
    for t in terms {
        collect_ground_terms_from_term(t, acc);
    }
}

fn ground_terms_in_content(content: &str) -> HashSet<String> {
    let mut terms = HashSet::new();
    for line in content.lines() {
        let s = normalize_line(line);
        if s.is_empty() || s.starts_with('%') {
            continue;
        }
        if let Ok(r) = parse_rule(s) {
            for lit in r.head.iter().chain(r.body.iter()) {
                collect_ground_terms_from_literal(lit, &mut terms);
            }
        }
    }
    terms
}

/// Reads a `.pl` file and returns all ground terms found across all parseable lines.
pub fn ground_terms_in_file(path: impl AsRef<std::path::Path>) -> Result<HashSet<String>> {
    Ok(ground_terms_in_content(&read_file(path)?))
}

fn predicate_symbols_in_content(content: &str) -> HashSet<(String, usize)> {
    let mut preds = HashSet::new();
    for line in content.lines() {
        let s = normalize_line(line);
        if s.is_empty() || s.starts_with('%') {
            continue;
        }
        if let Ok(r) = parse_rule(s) {
            for lit in r.head.iter().chain(r.body.iter()) {
                match lit {
                    literal::positive_literal(a) => {
                        preds.insert((a.predicate.0.symbol.clone(), a.terms.len()));
                    }
                    literal::negative_literal(pred, terms) => {
                        preds.insert((pred.0.symbol.clone(), terms.len()));
                    }
                }
            }
        }
    }
    preds
}

/// Reads a `.pl` file and returns all (predicate_name, arity) pairs found.
pub fn predicate_symbols_in_file(path: impl AsRef<std::path::Path>) -> Result<HashSet<(String, usize)>> {
    Ok(predicate_symbols_in_content(&read_file(path)?))
}

/// For each (predicate, arity) in `predicates` and each term name in `terms`,
/// produces ground atoms by substituting that term into all argument positions.
pub fn ground_atoms_for_predicates(
    predicates: &HashSet<(String, usize)>,
    terms: &HashSet<String>,
) -> HashSet<String> {
    let mut atoms = HashSet::new();
    let mut sorted_terms: Vec<&String> = terms.iter().collect();
    sorted_terms.sort();
    for (pred, arity) in predicates {
        for t in &sorted_terms {
            if *arity == 0 {
                atoms.insert(pred.clone());
            } else {
                let args = vec![t.as_str(); *arity].join(", ");
                atoms.insert(format!("{pred}({args})"));
            }
        }
    }
    atoms
}

fn literals_in_content(content: &str) -> HashSet<String> {
    let mut lits = HashSet::new();
    for line in content.lines() {
        let s = normalize_line(line);
        if s.is_empty() || s.starts_with('%') {
            continue;
        }
        if let Ok(r) = parse_rule(s) {
            for lit in r.head.iter().chain(r.body.iter()) {
                lits.insert(lit.to_string());
            }
        }
    }
    lits
}

/// Reads a `.pl` file and returns all literals found across all parseable lines.
pub fn literals_in_file(path: impl AsRef<std::path::Path>) -> Result<HashSet<String>> {
    Ok(literals_in_content(&read_file(path)?))
}

/// Returns induced ground rules: ground literals as-is, non-ground literals substituted
/// with each ground term. Only rules produced by substitution are included in the
/// Returns `(all, valued_true, valued_false)` where:
/// - `all`: string form of all induced ground rules
/// - `valued_true`: ground literals from the file (value = true)
/// - `valued_false`: rules produced by substituting ground terms into non-ground literals (value = false)
fn induced_ground_rules_in_content(content: &str) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let ground_term_names = ground_terms_in_content(content);
    let ground_terms: Vec<term> = ground_term_names
        .iter()
        .filter_map(|name| term::new(name.clone(), Some(0), vec![]).ok())
        .collect();

    let mut all: HashSet<String> = HashSet::new();
    let mut valued_true: HashSet<String> = HashSet::new();
    let mut valued_false: HashSet<String> = HashSet::new();

    for line in content.lines() {
        let s = normalize_line(line);
        if s.is_empty() || s.starts_with('%') {
            continue;
        }
        if let Ok(r) = parse_rule(s) {
            for lit in r.head.iter().chain(r.body.iter()) {
                let unit = match rule::unit_clause(vec![lit.clone()]) {
                    Ok(u) => u,
                    Err(_) => continue,
                };
                let var_count = unit.variables().len();
                if var_count == 0 {
                    let key = unit.to_string();
                    all.insert(key.clone());
                    if lit.is_ground() {
                        valued_true.insert(key);
                    }
                    continue;
                }
                for gt in &ground_terms {
                    let subs = vec![gt.clone(); var_count];
                    let fresh = rule::unit_clause(vec![lit.clone()]).unwrap();
                    if let Ok(grounded) = fresh.substitution(subs) {
                        let key = grounded.to_string();
                        all.insert(key.clone());
                        if !valued_true.contains(&key) {
                            valued_false.insert(key);
                        }
                    }
                }
            }
        }
    }
    (all, valued_true, valued_false)
}

/// Reads a `.pl` file and returns `(all_induced, valued_true, valued_false)`:
/// - `all_induced`: string form of all induced ground rules
/// - `valued_true`: ground literals (value = true)
/// - `valued_false`: substituted rules that are not ground facts (value = false)
pub fn induced_ground_rules_in_file(
    path: impl AsRef<std::path::Path>,
) -> Result<(HashSet<String>, HashSet<String>, HashSet<String>)> {
    Ok(induced_ground_rules_in_content(&read_file(path)?))
}

pub fn classify_line(line: &str) -> Option<String> {
    let s = normalize_line(line);
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
    fn induce_ground_terms_from_fact() {
        let terms = ground_terms_in_content("mild(tandoori)");
        assert!(terms.contains("tandoori"));
        assert_eq!(terms.len(), 1);
    }

    #[test]
    fn induce_no_ground_terms_from_non_ground_atom() {
        let terms = ground_terms_in_content("happy(A)");
        assert!(terms.is_empty());
    }

    #[test]
    fn induce_ground_terms_from_mixed_content() {
        let content = "mild(tandoori)\nhappy(A) :- lego_builder(A)\nloves(alice, bob)";
        let terms = ground_terms_in_content(content);
        assert!(terms.contains("tandoori"));
        assert!(terms.contains("alice"));
        assert!(terms.contains("bob"));
        assert_eq!(terms.len(), 3);
    }

    #[test]
    fn induce_ground_terms_skips_comments_and_blank_lines() {
        let content = "% this is a comment\n\nmild(tandoori)";
        let terms = ground_terms_in_content(content);
        assert!(terms.contains("tandoori"));
        assert_eq!(terms.len(), 1);
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
