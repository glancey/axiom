use anyhow::Result;
use axiom_syntalog::{literal, rule};
use formalisms::{individual_variable, term, TermType};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::{ground_atoms_for_predicates, ground_terms_in_file, literals_in_file, predicate_symbols_in_file};

pub struct Theory {
    pub terms: HashSet<String>,
    pub base: HashSet<String>,
    pub interpretation: HashSet<String>,
}

fn map_literal_terms(lit: &literal, f: &mut impl FnMut(&term) -> term) -> literal {
    match lit {
        literal::positive_literal(a) => {
            let new_terms = a.terms.iter().map(|t| f(t)).collect();
            literal::positive_literal(axiom_syntalog::atom::new(a.predicate.clone(), new_terms).unwrap())
        }
        literal::negative_literal(pred, terms) => {
            let new_terms = terms.iter().map(|t| f(t)).collect();
            literal::negative_literal(pred.clone(), new_terms)
        }
    }
}

fn literal_terms(lit: &literal) -> &[term] {
    match lit {
        literal::positive_literal(a) => &a.terms,
        literal::negative_literal(_, terms) => terms,
    }
}

impl Theory {
    pub fn new(
        background: impl AsRef<Path>,
        ex_plus: impl AsRef<Path>,
        ex_minus: impl AsRef<Path>,
    ) -> Result<Self> {
        fn check_pl(p: &Path) -> Result<()> {
            if p.extension().and_then(|e| e.to_str()) != Some("pl") {
                anyhow::bail!("expected a .pl file, got: {}", p.display());
            }
            Ok(())
        }
        let (background, ex_plus, ex_minus) = (background.as_ref(), ex_plus.as_ref(), ex_minus.as_ref());
        check_pl(background)?;
        check_pl(ex_plus)?;
        check_pl(ex_minus)?;

        let terms = ground_terms_in_file(background)?;
        let mut predicates = predicate_symbols_in_file(background)?;
        predicates.extend(predicate_symbols_in_file(ex_plus)?);
        predicates.extend(predicate_symbols_in_file(ex_minus)?);
        let base = ground_atoms_for_predicates(&predicates, &terms);
        let mut known = literals_in_file(background)?;
        known.extend(literals_in_file(ex_plus)?);
        let interpretation = base.iter().filter(|s| known.contains(*s)).cloned().collect();
        Ok(Theory { terms, base, interpretation })
    }

    /// Replaces each ground term from `self.terms` in the rule with a unique
    /// `individual_variable` (A, B, C, …) and returns the generalized rule.
    pub fn generalize(&self, r: &rule) -> Result<rule> {
        // Build a stable mapping: term name → variable name.
        let mut sorted_terms: Vec<&String> = self.terms.iter().collect();
        sorted_terms.sort();
        let var_names: Vec<String> = ('A'..='Z')
            .map(|c| c.to_string())
            .collect();
        let mapping: HashMap<String, term> = sorted_terms.iter().enumerate()
            .filter_map(|(i, t)| {
                let var_name = var_names.get(i)?;
                let var = individual_variable::new(var_name).ok()?;
                Some(((*t).clone(), term { term_type: TermType::Variable(var) }))
            })
            .collect();

        fn replace_term(t: &term, mapping: &HashMap<String, term>) -> term {
            match &t.term_type {
                TermType::Constant(c) => mapping.get(&c.0.symbol).cloned().unwrap_or_else(|| t.clone()),
                TermType::Variable(_) => t.clone(),
                TermType::Operation(op) => {
                    let new_vars = op.vars.iter().map(|v| replace_term(v, mapping)).collect();
                    term { term_type: TermType::Operation(formalisms::operation { symbol: op.symbol.clone(), vars: new_vars }) }
                }
            }
        }

        let new_head = r.head.iter().map(|lit| map_literal_terms(lit, &mut |t| replace_term(t, &mapping))).collect();
        let new_body = r.body.iter().map(|lit| map_literal_terms(lit, &mut |t| replace_term(t, &mapping))).collect();
        rule::new(new_head, new_body)
    }

    /// Rewrites a rule's variables to A, B, C, … in order of first appearance across head then body.
    fn normalize_vars(r: rule) -> Result<rule> {
        let var_names: Vec<String> = ('A'..='Z').map(|c| c.to_string()).collect();
        let mut mapping: HashMap<String, term> = HashMap::new();
        let mut counter = 0;

        let mut remap = |t: &term| -> term {
            match &t.term_type {
                TermType::Variable(v) => {
                    if let Some(mapped) = mapping.get(&v.name) {
                        return mapped.clone();
                    }
                    let new_name = &var_names[counter];
                    counter += 1;
                    let new_var = individual_variable::new(new_name).unwrap();
                    let new_term = term { term_type: TermType::Variable(new_var) };
                    mapping.insert(v.name.clone(), new_term.clone());
                    new_term
                }
                TermType::Constant(_) => t.clone(),
                TermType::Operation(op) => {
                    let new_vars = op.vars.iter().map(|v| match &v.term_type {
                        TermType::Variable(v2) => mapping.get(&v2.name).cloned().unwrap_or_else(|| v.clone()),
                        _ => v.clone(),
                    }).collect();
                    term { term_type: TermType::Operation(formalisms::operation { symbol: op.symbol.clone(), vars: new_vars }) }
                }
            }
        };

        let new_head = r.head.iter().map(|l| map_literal_terms(l, &mut remap)).collect();
        let new_body = r.body.iter().map(|l| map_literal_terms(l, &mut remap)).collect();
        rule::new(new_head, new_body)
    }

    /// For each ground atom in `base` as head, builds a rule `h :- b1, ..., bn`
    /// where `bi`s are the remaining atoms of `base` sharing the same ground term as `h`.
    /// Generalizes each candidate and adds it to the result only if `is_model` returns true.
    /// Returns all such rules, deduplicated by string.
    pub fn build_model(&self) -> Vec<rule> {
        use axiom_syntalog::parse_rule;

        fn ground_term_of(s: &str) -> Option<String> {
            let r = parse_rule(s).ok()?;
            let lit = r.head.into_iter().next()?;
            let terms = match &lit {
                literal::positive_literal(a) => a.terms.clone(),
                literal::negative_literal(_, terms) => terms.clone(),
            };
            terms.into_iter().find_map(|t| match t.term_type {
                TermType::Constant(c) => Some(c.0.symbol.clone()),
                _ => None,
            })
        }

        let mut sorted_base: Vec<&String> = self.base.iter().collect();
        sorted_base.sort();

        let mut seen = std::collections::HashSet::new();
        let mut rules = Vec::new();

        for head_s in &sorted_base {
            let head_term = match ground_term_of(head_s) {
                Some(t) => t,
                None => continue,
            };
            let head_rule = match parse_rule(head_s) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let head_lit = match head_rule.head.into_iter().next() {
                Some(l) => l,
                None => continue,
            };

            let body: Vec<literal> = sorted_base.iter()
                .filter(|s| **s != *head_s)
                .filter(|s| ground_term_of(s).as_deref() == Some(&head_term))
                .filter(|s| self.interpretation.contains(**s))
                .filter_map(|s| {
                    parse_rule(s).ok()?.head.into_iter().next()
                })
                .collect();

            if body.is_empty() {
                continue;
            }

            let candidate = match rule::new(vec![head_lit], body) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let Ok(g) = self.generalize(&candidate) {
                // Only keep rules where every body variable also appears in the head.
                let head_vars: std::collections::HashSet<String> = g.head.iter()
                    .flat_map(|l| literal_terms(l).iter().filter_map(|t| match &t.term_type {
                        TermType::Variable(v) => Some(v.name.clone()),
                        _ => None,
                    }))
                    .collect();
                let safe = g.body.iter().all(|l| {
                    literal_terms(l).iter().all(|t| match &t.term_type {
                        TermType::Variable(v) => head_vars.contains(&v.name),
                        _ => true,
                    })
                });
                if safe && self.is_model(&g) {
                    if let Ok(normalized) = Self::normalize_vars(g) {
                        let key = normalized.to_string();
                        if seen.insert(key) {
                            rules.push(normalized);
                        }
                    }
                }
            }
        }
        rules
    }

    /// Returns true if, for every term in `self.terms` substituted into `r`,
    /// all head and body literals of the resulting ground rule are in `self.interpretation`.
    pub fn is_model(&self, r: &rule) -> bool {
        let var_count = r.variables().len();
        if var_count == 0 {
            return r.head.iter().chain(r.body.iter())
                .all(|l| self.interpretation.contains(&l.to_string()));
        }
        let ground_terms: Vec<term> = {
            let mut sorted: Vec<&String> = self.terms.iter().collect();
            sorted.sort();
            sorted.iter()
                .filter_map(|name| term::new((*name).clone(), Some(0), vec![]).ok())
                .collect()
        };
        let mut any_satisfied = false;
        for gt in &ground_terms {
            let subs = vec![gt.clone(); var_count];
            let grounded = match r.clone().substitution(subs) {
                Err(_) => return false,
                Ok(g) => g,
            };
            let body_satisfied = grounded.body.iter()
                .all(|l| self.interpretation.contains(&l.to_string()));
            let head_satisfied = grounded.head.iter()
                .all(|l| self.interpretation.contains(&l.to_string()));
            if body_satisfied && !head_satisfied {
                return false;
            }
            if body_satisfied && head_satisfied {
                any_satisfied = true;
            }
        }
        any_satisfied
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use axiom_syntalog::parse_rule;

    fn test_theory() -> Theory {
        let terms: HashSet<String> = ["alice", "bob", "claire", "dave"]
            .iter().map(|s| s.to_string()).collect();
        let base: HashSet<String> = [
            "enjoys_lego(alice)", "enjoys_lego(bob)", "enjoys_lego(claire)", "enjoys_lego(dave)",
            "estate_agent(alice)", "estate_agent(bob)", "estate_agent(claire)", "estate_agent(dave)",
            "happy(alice)", "happy(bob)", "happy(claire)", "happy(dave)",
            "lego_builder(alice)", "lego_builder(bob)", "lego_builder(claire)", "lego_builder(dave)",
        ].iter().map(|s| s.to_string()).collect();
        let interpretation: HashSet<String> = [
            "enjoys_lego(alice)", "enjoys_lego(claire)",
            "estate_agent(claire)", "estate_agent(dave)",
            "happy(alice)", "lego_builder(alice)", "lego_builder(bob)",
        ].iter().map(|s| s.to_string()).collect();
        Theory { terms, base, interpretation }
    }

    // Interpretation: enjoys_lego(alice), enjoys_lego(claire),
    //   estate_agent(claire), estate_agent(dave),
    //   happy(alice), lego_builder(alice), lego_builder(bob)
    //
    #[test]
    fn build_model_is_not_empty() {
        let theory = test_theory();
        assert!(!theory.build_model().is_empty());
    }

    // happy(X) :- lego_builder(X), enjoys_lego(X)
    //   alice: lego_builder(alice)✓, enjoys_lego(alice)✓, happy(alice)✓ → true
    #[test]
    fn is_model_true_when_at_least_one_term_satisfies_all_literals() {
        let theory = test_theory();
        let r = parse_rule("happy(X) :- lego_builder(X), enjoys_lego(X)").unwrap();
        assert!(theory.is_model(&r));
    }

    // estate_agent(X) :- happy(X), lego_builder(X)
    //   alice: body filtered = happy(alice)✓, lego_builder(alice)✓ (non-empty)
    //          head estate_agent(alice)✗ → false
    //   bob:   body filtered = lego_builder(bob)✓ (non-empty), head estate_agent(bob)✗ → false
    //   claire/dave: happy not in interpretation → no term qualifies
    #[test]
    fn is_model_false_when_head_not_in_interpretation() {
        let theory = test_theory();
        let r = parse_rule("estate_agent(X) :- happy(X), lego_builder(X)").unwrap();
        assert!(!theory.is_model(&r));
    }

    // enjoys_lego(A) :- estate_agent(A), happy(A), lego_builder(A)
    //   alice: estate_agent(alice)✗ → skip; no term satisfies all body literals
    #[test]
    fn is_model_false_when_body_never_fully_satisfied() {
        let theory = test_theory();
        let r = parse_rule("enjoys_lego(A) :- estate_agent(A), happy(A), lego_builder(A)").unwrap();
        assert!(!theory.is_model(&r));
    }

    // enjoys_lego(A) :- estate_agent(A)
    //   claire: estate_agent(claire)✓, enjoys_lego(claire)✓ → satisfied
    //   dave:   estate_agent(dave)✓,   enjoys_lego(dave)✗   → body satisfied, head not → false
    #[test]
    fn is_model_false_when_dave_violates_rule() {
        let theory = test_theory();
        let r = parse_rule("enjoys_lego(A) :- estate_agent(A)").unwrap();
        assert!(!theory.is_model(&r));
    }
}
