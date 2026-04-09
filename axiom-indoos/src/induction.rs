use anyhow::Result;
use axiom_syntalog::{literal, rule};
use formalisms::{individual_variable, term, TermType};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::{ground_terms_in_file, positive_facts_in_file, negative_example_atoms_in_file};

fn terms_equal(a: &term, b: &term) -> bool {
    match (&a.term_type, &b.term_type) {
        (TermType::Variable(v1), TermType::Variable(v2)) => v1.name == v2.name,
        (TermType::Constant(c1), TermType::Constant(c2)) => c1.0.symbol == c2.0.symbol,
        (TermType::Operation(o1), TermType::Operation(o2)) =>
            o1.symbol.symbol == o2.symbol.symbol
                && o1.vars.len() == o2.vars.len()
                && o1.vars.iter().zip(o2.vars.iter()).all(|(a, b)| terms_equal(a, b)),
        _ => false,
    }
}

/// One-sided unification: bind variables in `pattern` to ground sub-terms.
/// Returns false if any variable is already bound to a different term.
fn match_term(pattern: &term, ground: &term, subs: &mut HashMap<String, term>) -> bool {
    match &pattern.term_type {
        TermType::Variable(v) => {
            if let Some(existing) = subs.get(&v.name) {
                terms_equal(existing, ground)
            } else {
                subs.insert(v.name.clone(), ground.clone());
                true
            }
        }
        TermType::Constant(c) =>
            matches!(&ground.term_type, TermType::Constant(gc) if gc.0.symbol == c.0.symbol),
        TermType::Operation(op) => {
            if let TermType::Operation(gop) = &ground.term_type {
                op.symbol.symbol == gop.symbol.symbol
                    && op.vars.len() == gop.vars.len()
                    && op.vars.iter().zip(gop.vars.iter()).all(|(p, g)| match_term(p, g, subs))
            } else {
                false
            }
        }
    }
}

/// Recursively finds all complete variable substitutions that make every literal
/// in `body` match a fact in `interpretation`.  Each call fixes one body literal
/// and recurses on the rest, accumulating consistent bindings.
fn extend_subs(
    body: &[literal],
    interpretation: &HashSet<String>,
    current: &HashMap<String, term>,
    results: &mut Vec<HashMap<String, term>>,
) {
    if body.is_empty() {
        results.push(current.clone());
        return;
    }
    let (lit, rest) = (&body[0], &body[1..]);
    for fact in interpretation {
        let gr_lit = match axiom_syntalog::parse_rule(fact)
            .ok()
            .and_then(|r| r.head.into_iter().next())
        {
            Some(l) => l,
            None => continue,
        };
        let mut subs = current.clone();
        if match_literal(lit, &gr_lit, &mut subs) {
            extend_subs(rest, interpretation, &subs, results);
        }
    }
}

fn match_literal(pattern: &literal, ground: &literal, subs: &mut HashMap<String, term>) -> bool {
    match (pattern, ground) {
        (literal::positive_literal(pa), literal::positive_literal(ga)) =>
            pa.predicate.0.symbol == ga.predicate.0.symbol
                && pa.terms.len() == ga.terms.len()
                && pa.terms.iter().zip(ga.terms.iter()).all(|(p, g)| match_term(p, g, subs)),
        (literal::negative_literal(pp, pt), literal::negative_literal(gp, gt)) =>
            pp.0.symbol == gp.0.symbol
                && pt.len() == gt.len()
                && pt.iter().zip(gt.iter()).all(|(p, g)| match_term(p, g, subs)),
        _ => false,
    }
}

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
        ex_minus: Option<&Path>,
    ) -> Result<Self> {
        fn check_pl(p: &Path) -> Result<()> {
            if p.extension().and_then(|e| e.to_str()) != Some("pl") {
                anyhow::bail!("expected a .pl file, got: {}", p.display());
            }
            Ok(())
        }
        let (background, ex_plus) = (background.as_ref(), ex_plus.as_ref());
        check_pl(background)?;
        check_pl(ex_plus)?;
        if let Some(p) = ex_minus { check_pl(p)?; }

        let terms = ground_terms_in_file(background)?;
        // interpretation = atoms known to be true (background + positive examples)
        let mut interpretation = positive_facts_in_file(background)?;
        interpretation.extend(positive_facts_in_file(ex_plus)?);
        // base = interpretation + atoms known to be false (negative examples, stripped of `not`)
        let mut base = interpretation.clone();
        if let Some(p) = ex_minus {
            base.extend(negative_example_atoms_in_file(p)?);
        }
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
        fn remap_term(
            t: &term,
            mapping: &mut HashMap<String, term>,
            counter: &mut usize,
            var_names: &[String],
        ) -> term {
            match &t.term_type {
                TermType::Variable(v) => {
                    if let Some(mapped) = mapping.get(&v.name) {
                        return mapped.clone();
                    }
                    let new_name = &var_names[*counter];
                    *counter += 1;
                    let new_var = individual_variable::new(new_name).unwrap();
                    let new_term = term { term_type: TermType::Variable(new_var) };
                    mapping.insert(v.name.clone(), new_term.clone());
                    new_term
                }
                TermType::Constant(_) => t.clone(),
                TermType::Operation(op) => {
                    let new_vars = op.vars.iter()
                        .map(|v| remap_term(v, mapping, counter, var_names))
                        .collect();
                    term { term_type: TermType::Operation(formalisms::operation { symbol: op.symbol.clone(), vars: new_vars }) }
                }
            }
        }

        let var_names: Vec<String> = ('A'..='Z').map(|c| c.to_string()).collect();
        let mut mapping: HashMap<String, term> = HashMap::new();
        let mut counter = 0;

        let new_head = r.head.iter()
            .map(|l| map_literal_terms(l, &mut |t| remap_term(t, &mut mapping, &mut counter, &var_names)))
            .collect();
        let new_body = r.body.iter()
            .map(|l| map_literal_terms(l, &mut |t| remap_term(t, &mut mapping, &mut counter, &var_names)))
            .collect();
        rule::new(new_head, new_body)
    }

    /// Replaces each distinct compound argument (list / operation) in a rule with a
    /// fresh variable, then re-normalizes.  Two occurrences of structurally identical
    /// compound terms receive the same variable.  Scalar variables are left untouched.
    ///
    /// Example:
    ///   `last([A,B,C,D,E,F,G], G) :- head([A,B,C,D,E,F,G], A), tail([A,B,C,D,E,F,G], [B,C,D,E,F,G]).`
    ///   →  `last(A, B) :- head(A, B), tail(A, C).`
    fn structural_generalize(r: rule) -> Result<rule> {
        use axiom_syntalog::term_to_string;

        // Use apostrophe-suffixed names (A', B', …) for compound replacements so
        // they are distinct from the existing scalar variables (A, B, …) in the rule.
        // normalize_vars then renumbers all variables in appearance order.
        let compound_var_names: Vec<String> = ('A'..='Z').map(|c| format!("{c}'")).collect();
        let mut compound_map: HashMap<String, term> = HashMap::new();

        fn lift_term(
            t: &term,
            map: &mut HashMap<String, term>,
            var_names: &[String],
        ) -> term {
            match &t.term_type {
                TermType::Operation(_) => {
                    let key = term_to_string(t);
                    if let Some(v) = map.get(&key) {
                        return v.clone();
                    }
                    let idx = map.len();
                    let new_var = individual_variable::new(&var_names[idx]).unwrap();
                    let new_term = term { term_type: TermType::Variable(new_var) };
                    map.insert(key, new_term.clone());
                    new_term
                }
                // Scalar variables and constants pass through unchanged.
                _ => t.clone(),
            }
        }

        let new_head = r.head.iter()
            .map(|l| map_literal_terms(l, &mut |t| lift_term(t, &mut compound_map, &compound_var_names)))
            .collect();
        let new_body = r.body.iter()
            .map(|l| map_literal_terms(l, &mut |t| lift_term(t, &mut compound_map, &compound_var_names)))
            .collect();
        let lifted = rule::new(new_head, new_body)?;
        Self::normalize_vars(lifted)
    }

    /// For each ground atom in `base` as head, builds a rule `h :- b1, ..., bn`
    /// where `bi`s are the remaining atoms of `base` sharing the same ground term as `h`.
    /// Generalizes each candidate and adds it to the result only if `is_model` returns true.
    /// Returns all such rules, deduplicated by string.
    pub fn build_model(&self) -> Vec<rule> {
        use axiom_syntalog::{parse_rule, term_to_string};

        // Returns the display string of the first top-level argument of the head literal.
        // Works for any term type: scalar constant, list, or compound operation.
        fn ground_term_of(s: &str) -> Option<String> {
            let r = parse_rule(s).ok()?;
            let lit = r.head.into_iter().next()?;
            let terms = match lit {
                literal::positive_literal(a) => a.terms,
                literal::negative_literal(_, terms) => terms,
            };
            terms.into_iter().next().map(|t| term_to_string(&t))
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
                // Collect variables recursively so variables inside list
                // operations in the head are not missed.
                fn vars_in_term(t: &term, out: &mut HashSet<String>) {
                    match &t.term_type {
                        TermType::Variable(v) => { out.insert(v.name.clone()); }
                        TermType::Operation(op) => { for v in &op.vars { vars_in_term(v, out); } }
                        TermType::Constant(_) => {}
                    }
                }
                let mut head_vars: HashSet<String> = HashSet::new();
                for l in &g.head {
                    for t in literal_terms(l) { vars_in_term(t, &mut head_vars); }
                }
                let safe = g.body.iter().all(|l| {
                    let mut bvars = HashSet::new();
                    for t in literal_terms(l) { vars_in_term(t, &mut bvars); }
                    bvars.iter().all(|v| head_vars.contains(v))
                });
                if safe && self.is_model(&g) {
                    if let Ok(normalized) = Self::normalize_vars(g) {
                        if let Ok(structural) = Self::structural_generalize(normalized) {
                            let key = structural.to_string();
                            if seen.insert(key) {
                                rules.push(structural);
                            }
                        }
                    }
                }
            }
        }
        rules
    }

    /// Returns true if the rule is consistent with `self.interpretation`.
    ///
    /// For ground rules, checks all literals directly.  For rules with variables,
    /// enumerates every grounding of the body literals that is satisfied by the
    /// interpretation (via pattern matching), then checks whether the head holds
    /// for each such grounding.  Returns false if any body-satisfying grounding
    /// leaves the head unsatisfied; returns true if at least one grounding
    /// satisfies both body and head.
    pub fn is_model(&self, r: &rule) -> bool {
        let vars = r.variables();
        if vars.is_empty() {
            return r.head.iter().chain(r.body.iter())
                .all(|l| self.interpretation.contains(&l.to_string()));
        }

        // Find all complete substitutions that satisfy the entire body.
        let mut all_subs: Vec<HashMap<String, term>> = Vec::new();
        extend_subs(&r.body, &self.interpretation, &HashMap::new(), &mut all_subs);

        if all_subs.is_empty() {
            return false; // body never satisfied → no evidence the rule fires
        }

        let mut any_satisfied = false;
        for subs in all_subs {
            let sub_terms: Option<Vec<term>> =
                vars.iter().map(|v| subs.get(&v.name).cloned()).collect();
            let sub_terms = match sub_terms {
                Some(s) => s,
                None => continue,
            };
            let grounded = match r.clone().substitution(sub_terms) {
                Ok(g) => g,
                Err(_) => continue,
            };
            let head_satisfied = grounded.head.iter()
                .all(|l| self.interpretation.contains(&l.to_string()));
            if !head_satisfied {
                return false;
            }
            any_satisfied = true;
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

    // background: empty([]).  head([m,a,c,h,i,n,e], m).  tail([m,a,c,h,i,n,e], [a,c,h,i,n,e]).
    // ex_plus:    last([m,a,c,h,i,n,e], e).  last([l,e,a,r,n,i,n,g], g).  last([a,l,g,o,r,i,t,m], m).
    //
    // terms: recursive extraction from background lists ([] excluded) → {a,c,e,h,i,m,n}
    //
    // base: 4 predicates × 7 terms, all with uniform scalar args, e.g. head(m,m)
    //   → no base atom matches any list-bearing literal in the files
    //
    // interpretation: base ∩ known = {} (empty)
    //   → build_model returns no rules
    #[test]
    fn build_model_with_list_predicates() {
        use std::path::Path;

        let dir = std::env::temp_dir();
        let bg_path  = dir.join("axiom_test_bg_list.pl");
        let ex_path  = dir.join("axiom_test_ex_list.pl");

        std::fs::write(&bg_path,
            "empty([]).\nhead([m,a,c,h,i,n,e], m).\ntail([m,a,c,h,i,n,e], [a,c,h,i,n,e]).\n"
        ).unwrap();
        std::fs::write(&ex_path,
            "last([m,a,c,h,i,n,e], e).\nlast([l,e,a,r,n,i,n,g], g).\nlast([a,l,g,o,r,i,t,m], m).\n"
        ).unwrap();

        let theory = Theory::new(Path::new(&bg_path), Path::new(&ex_path), None).unwrap();

        // Scalar terms extracted from background ([] excluded).
        let expected_terms: HashSet<String> =
            ["a", "c", "e", "h", "i", "m", "n"].iter().map(|s| s.to_string()).collect();
        assert_eq!(theory.terms, expected_terms);

        // base = interpretation = all known facts (3 background + 3 ex_plus).
        assert_eq!(theory.interpretation.len(), 6);
        assert!(theory.interpretation.contains("head([m, a, c, h, i, n, e], m)"));
        assert!(theory.interpretation.contains("last([m, a, c, h, i, n, e], e)"));

        // build_model: for [m,a,c,h,i,n,e], head/tail/last share the same first arg.
        // After generalize, normalize_vars, then structural_generalize (list args → single vars):
        //   list arg → A, scalar element → B or C/D, tail list → separate var
        let model = theory.build_model();
        let rule_strs: HashSet<String> = model.iter().map(|r| r.to_string()).collect();
        assert!(rule_strs.contains(
            "last(A, B) :- head(A, C), tail(A, D)."
        ), "rules: {rule_strs:#?}");
    }

    // head([m,a,c,h,i,n,e], m):-
    //   Parse: head literal with a dot-list term and constant `m`; empty body.
    //   Terms sorted: a→A, c→B, e→C, h→D, i→E, m→F, n→G
    //   After generalize: second arg `m` becomes variable `F`; list renders in bracket notation.
    #[test]
    fn generalize_list_head_rule() {
        let terms: HashSet<String> = ["m", "a", "c", "h", "i", "n", "e"]
            .iter().map(|s| s.to_string()).collect();
        let theory = Theory {
            terms,
            base: HashSet::new(),
            interpretation: HashSet::new(),
        };
        let r = parse_rule("head([m,a,c,h,i,n,e], m):-").unwrap();
        let generalized = theory.generalize(&r).unwrap();
        assert_eq!(generalized.head.len(), 1);
        assert_eq!(generalized.body.len(), 0);
        assert_eq!(generalized.to_string(), "head([F, A, B, D, E, G, C], F).");
    }
}
