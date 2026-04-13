use anyhow::Result;
use std::collections::HashMap;

pub mod derivations;
pub mod proofs;
use proofs::{Proof, ProofTable};

/// A variable ranging over individuals in the domain.
/// Must begin with an uppercase ASCII letter (A–Z), optionally followed by any
/// combination of letters, digits, underscores, or apostrophes.
/// Examples: `A`, `B'`, `X'''`, `Day`, `Node1`
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct individual_variable {
    pub name: String,
}

impl individual_variable {
    pub fn new(s: &str) -> Result<Self> {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c.is_ascii_uppercase() => {}
            _ => anyhow::bail!("individual_variable must begin with an uppercase letter"),
        }
        for c in chars {
            if !c.is_alphanumeric() && c != '_' && c != '\'' {
                anyhow::bail!(
                    "individual_variable may only contain letters, digits, underscores, or apostrophes after the initial uppercase letter"
                );
            }
        }
        Ok(individual_variable { name: s.to_string() })
    }
}

/// One of the fixed logical connectives and punctuation symbols of the language:
/// `/\` (and), `\/` (or), `=>` (implies), `¬` (not), `<=>` (iff),
/// `∀` (for all), `Ǝ` (there exists), `==` (equals), `(`, `)`
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct logical_symbol(String);

impl logical_symbol {
    pub fn new(s: String) -> Result<Self> {
        const VALID: &[&str] = &[
            "\u{2227}", "\u{2228}", "=>", "\u{00AC}", "<=>",
            "\u{2200}", "\u{018E}", "==", "(", ")",
        ];
        if VALID.contains(&s.as_str()) {
            Ok(logical_symbol(s))
        } else {
            anyhow::bail!("not a valid logical symbol: {s}")
        }
    }

    pub fn symbol(&self) -> &str {
        &self.0
    }
}

/// A named symbol used to build terms and relations.
/// Must not be a `logical_symbol` or an `individual_variable`.
/// Carries a `rank` indicating the number of arguments the symbol takes.
/// Example: In mathematical terms, an operation, O, of rank 10, would be 
/// represented as O(a0, a1, a2,... a9).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct operation_symbol {
    pub symbol: String,
    pub rank: u32,
}

impl operation_symbol {
    pub fn new(s: String, rank: u32) -> Result<Self> {
        if logical_symbol::new(s.clone()).is_ok() {
            anyhow::bail!("operation_symbol cannot be a logical_symbol");
        }
        if individual_variable::new(&s).is_ok() {
            anyhow::bail!("operation_symbol cannot be an individual_variable");
        }
        Ok(operation_symbol { symbol: s, rank })
    }
}

/// A zero-arity `operation_symbol` (rank 0) naming a fixed individual in the domain.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct individual_constant(pub operation_symbol);

impl individual_constant {
    pub fn new(s: String) -> Result<Self> {
        Ok(individual_constant(operation_symbol::new(s, 0)?))
    }
}

/// An `operation_symbol` of rank 1–5 used to denote a relation between individuals.
/// Example: In mathematical terms, a Relation, R, of rank 4, would be
/// represented as R(a0, a1, a2, a3, a4).
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct relation_symbol(pub operation_symbol);

impl relation_symbol {
    pub fn new(s: String, rank: u32) -> Result<Self> {
        if !matches!(rank, 1..=5) {
            anyhow::bail!("relation_symbol rank must be 1, 2, 3, 4, or 5");
        }
        Ok(relation_symbol(operation_symbol::new(s, rank)?))
    }
}

/// An `operation_symbol` of rank m > 0 applied to exactly m terms.
/// `vars` must have the same length as `symbol.rank`.
/// Example: In logical terms, an operation of rank m is some process applied to all
/// the members of an array of size m.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct operation {
    pub symbol: operation_symbol,
    pub vars: Vec<term>,
}

impl operation {
    pub fn new(symbol: operation_symbol, vars: Vec<term>) -> Result<Self> {
        if symbol.rank == 0 {
            anyhow::bail!("operation rank must be > 0");
        }
        if vars.len() != symbol.rank as usize {
            anyhow::bail!("vars length must equal symbol rank");
        }
        Ok(operation { symbol, vars })
    }
}

/// Discriminates the three forms a `term` may take.
#[derive(Debug, Clone, PartialEq)]
pub enum TermType {
    /// An `individual_variable`.
    Variable(individual_variable),
    /// An `individual_constant` (operation_symbol of rank 0).
    Constant(individual_constant),
    /// An `operation_symbol` of rank m > 0 applied to m terms.
    Operation(operation),
}

/// A term in the language: either an individual variable, an individual constant,
/// or an operation symbol of rank m > 0 applied to m sub-terms.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct term {
    pub term_type: TermType,
}

impl term {
    pub fn new(s: String, rank: Option<u32>, vars: Vec<term>) -> Result<Self> {
        if let Ok(v) = individual_variable::new(&s) {
            return Ok(term { term_type: TermType::Variable(v) });
        }
        if rank == Some(0) {
            if let Ok(c) = individual_constant::new(s.clone()) {
                return Ok(term { term_type: TermType::Constant(c) });
            }
        }
        if let Some(m) = rank {
            if m > 0 {
                let sym = operation_symbol::new(s.clone(), m)?;
                let op = operation::new(sym, vars)?;
                return Ok(term { term_type: TermType::Operation(op) });
            }
        }
        anyhow::bail!("not a valid term: {s}")
    }
}

/// Discriminates the four forms a formula may take.
#[derive(Debug)]
pub enum FormulaType {
    /// An atomic formula consisting of a single term.
    Term(term),
    /// An atomic formula: a `relation_symbol` of rank m applied to m terms.
    Relation(relation_symbol, Vec<term>),
    /// A compound formula built by applying a `logical_symbol` connective
    /// (e.g. `/\`, `\/`, `=>`, `<=>`, `~`) to one or more sub-formulas.
    Combination(logical_symbol, Vec<Formula>),
    /// A quantified formula: a quantifier (`∀` or `Ǝ`), an `individual_variable`,
    /// and a sub-formula in whose scope the variable is bound.
    Quantifier(logical_symbol, individual_variable, Box<Formula>),
}

/// A well-formed formula (wff) of the language.
#[derive(Debug)]
pub struct Formula {
    pub formula_type: FormulaType,
    pub value: Option<bool>,
}

fn eval_connective(sym: &logical_symbol, results: &[bool]) -> bool {
    match sym.0.as_str() {
        "\u{2227}" => results.iter().all(|&v| v),
        "\u{2228}" => results.iter().any(|&v| v),
        "\u{00AC}" => results.first().map_or(false, |&v| !v),
        "=>" => results.len() != 2 || !results[0] || results[1],
        "<=>" => results.len() == 2 && results[0] == results[1],
        _ => false,
    }
}

pub fn all_assignments(vars: &[String]) -> Vec<HashMap<String, bool>> {
    let n = vars.len();
    (0u64..(1u64 << n))
        .map(|mask| {
            vars.iter()
                .enumerate()
                .map(|(i, name)| (name.clone(), (mask >> i) & 1 == 1))
                .collect()
        })
        .collect()
}

impl Formula {
    /// Evaluates whether this formula is true given a slice of contextual formulas.
    ///
    /// - Atomic formulas (`Term`, `Relation`): returns `self.value.unwrap_or(false)`.
    /// - `Combination`: evaluates structurally by the connective, recursing with `context`.
    /// - `Quantifier`: delegates to the body formula.
    /// Collect all unique variable names appearing in this formula.
    pub fn collect_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_variables_into(&mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    fn collect_variables_into(&self, vars: &mut Vec<String>) {
        match &self.formula_type {
            FormulaType::Term(t) => {
                if let TermType::Variable(v) = &t.term_type {
                    vars.push(v.name.clone());
                }
            }
            FormulaType::Relation(_, terms) => {
                for t in terms {
                    if let TermType::Variable(v) = &t.term_type {
                        vars.push(v.name.clone());
                    }
                }
            }
            FormulaType::Combination(_, formulas) => {
                for f in formulas {
                    f.collect_variables_into(vars);
                }
            }
            FormulaType::Quantifier(_, v, body) => {
                vars.push(v.name.clone());
                body.collect_variables_into(vars);
            }
        }
    }

    /// Evaluate the formula under a specific variable assignment.
    pub fn evaluate(&self, assignment: &HashMap<String, bool>) -> bool {
        match &self.formula_type {
            FormulaType::Term(t) => match &t.term_type {
                TermType::Variable(v) => *assignment.get(&v.name).unwrap_or(&false),
                _ => self.value.unwrap_or(false),
            },
            FormulaType::Relation(_, _) => self.value.unwrap_or(false),
            FormulaType::Combination(sym, formulas) => {
                let results: Vec<bool> = formulas.iter().map(|f| f.evaluate(assignment)).collect();
                eval_connective(sym, &results)
            }
            FormulaType::Quantifier(_, _, body) => body.evaluate(assignment),
        }
    }

    /// Renders the formula as a human-readable expression string.
    fn display_str(&self) -> String {
        fn fmt_term(t: &term) -> String {
            match &t.term_type {
                TermType::Variable(v) => v.name.clone(),
                TermType::Constant(c) => c.0.symbol.clone(),
                TermType::Operation(o) => {
                    let args: Vec<String> = o.vars.iter().map(fmt_term).collect();
                    format!("{}({})", o.symbol.symbol, args.join(", "))
                }
            }
        }
        match &self.formula_type {
            FormulaType::Term(t) => fmt_term(t),
            FormulaType::Relation(sym, terms) => {
                let args: Vec<String> = terms.iter().map(fmt_term).collect();
                format!("{}({})", sym.0.symbol, args.join(", "))
            }
            FormulaType::Combination(sym, formulas) => {
                if formulas.len() == 1 {
                    format!("{}({})", sym.0, formulas[0].display_str())
                } else {
                    let parts: Vec<String> = formulas.iter().map(|f| f.display_str()).collect();
                    format!("({})", parts.join(&format!(" {} ", sym.0)))
                }
            }
            FormulaType::Quantifier(sym, v, body) => {
                format!("{}{}.{}", sym.0, v.name, body.display_str())
            }
        }
    }

    /// Recursively evaluates and prints the truth value of each sub-formula
    /// (Terms, Relations, and Combinations) under the given assignment.
    pub fn evaluate_verbose(&self, assignment: &HashMap<String, bool>, proof: &mut Proof) -> bool {
        match &self.formula_type {
            FormulaType::Term(t) => {
                let val = match &t.term_type {
                    TermType::Variable(v) => *assignment.get(&v.name).unwrap_or(&false),
                    _ => self.value.unwrap_or(false),
                };
                //println!("  {} = {}", self.display_str(), val);
                proof.evals.push(HashMap::from([(self.display_str(), val)]));
                val
            }
            FormulaType::Relation(_, _) => {
                let val = self.value.unwrap_or(false);
                //println!("  {} = {}", self.display_str(), val);
                proof.evals.push(HashMap::from([(self.display_str(), val)]));
                val
            }
            FormulaType::Combination(sym, formulas) => {
                let sub_results: Vec<bool> = formulas.iter().map(|f| f.evaluate_verbose(assignment, proof)).collect();
                let val = eval_connective(sym, &sub_results);
                //println!("  {} = {}", self.display_str(), val);
                proof.evals.push(HashMap::from([(self.display_str(), val)]));
                val
            }
            FormulaType::Quantifier(_, _, body) => body.evaluate_verbose(assignment, proof),
        }
    }

    /// Return true if the formula holds under every possible truth assignment of its variables.
    /// Prints each assignment and its evaluation result, including sub-formula results.
    pub fn is_tautology(&self, proof_table: &mut ProofTable) -> bool {
        let vars = self.collect_variables();
        for assignment in all_assignments(&vars) {
            let mut sorted: Vec<(&String, &bool)> = assignment.iter().collect();
            sorted.sort_by_key(|(k, _)| *k);
            //let row: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
            //println!("assignment: [{}]", row.join(", "));
            let mut proof = Proof::new();
            proof.values.push(assignment.clone());
            proof_table.proofs.push(proof);
            let result = self.evaluate_verbose(&assignment, proof_table.proofs.last_mut().unwrap());
            //println!("result => {}", result);
            if !result {
                return false;
            }
        }
        true
    }

    /// Recursively sets `value` on all `Relation` and `Term` leaf nodes whose
    /// display string matches a key in `values`.
    pub fn set_relation_values(&mut self, values: &std::collections::HashMap<String, bool>) {
        match &mut self.formula_type {
            FormulaType::Relation(_, _) | FormulaType::Term(_) => {
                let key = self.display_str();
                if let Some(&v) = values.get(&key) {
                    self.value = Some(v);
                }
            }
            FormulaType::Combination(_, formulas) => {
                for f in formulas {
                    f.set_relation_values(values);
                }
            }
            FormulaType::Quantifier(_, _, body) => {
                body.set_relation_values(values);
            }
        }
    }

    pub fn is_true(&self, context: &[Formula]) -> bool {
        match &self.formula_type {
            FormulaType::Term(_) | FormulaType::Relation(_, _) => {
                self.value.unwrap_or(false)
            }
            FormulaType::Combination(sym, formulas) => {
                let results: Vec<bool> = formulas.iter().map(|f| f.is_true(context)).collect();
                eval_connective(sym, &results)
            }
            FormulaType::Quantifier(_, _, body) => body.is_true(context),
        }
    }
}

/// A rule in the form `h1, ..., hn :- b1, ..., bm`,
/// equivalent to the `Formula`: `(b1 ∧ ... ∧ bm) => (h1 ∧ ... ∧ hn)`.
///
/// - `head` — the conclusions (h1 … hn); must be non-empty.
/// - `body` — the premises   (b1 … bm); may be empty (fact / unconditional assertion).
///
/// Call [`rule::to_formula`] to obtain the corresponding `FormulaType::Combination`.
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct rule {
    pub head: Vec<Formula>,
    pub body: Vec<Formula>,
}

impl rule {
    pub fn new(head: Vec<Formula>, body: Vec<Formula>) -> Result<Self> {
        if head.is_empty() {
            anyhow::bail!("rule head must contain at least one formula");
        }
        Ok(rule { head, body })
    }

    /// Converts this rule into its logically equivalent `Formula`.
    ///
    /// - If the body is non-empty the result is
    ///   `FormulaType::Combination("=>", [body_conj, head_conj])`.
    /// - If the body is empty the rule is an unconditional fact and the result
    ///   is just `head_conj` (no implication wrapping needed).
    pub fn to_formula(self) -> Result<Formula> {
        // Build head conjunction: h1 ∧ ... ∧ hn
        let head_formula = if self.head.len() == 1 {
            self.head.into_iter().next().unwrap()
        } else {
            let and = logical_symbol::new("\u{2227}".to_string())?;
            Formula { formula_type: FormulaType::Combination(and, self.head), value: None }
        };

        // Empty body → unconditional assertion of the head
        if self.body.is_empty() {
            return Ok(head_formula);
        }

        // Build body conjunction: b1 ∧ ... ∧ bm
        let body_formula = if self.body.len() == 1 {
            self.body.into_iter().next().unwrap()
        } else {
            let and = logical_symbol::new("\u{2227}".to_string())?;
            Formula { formula_type: FormulaType::Combination(and, self.body), value: None }
        };

        let implies = logical_symbol::new("=>".to_string())?;
        Ok(Formula {
            formula_type: FormulaType::Combination(implies, vec![body_formula, head_formula]),
            value: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_symbol_valid() {
        assert!(logical_symbol::new("\u{2227}".to_string()).is_ok());
        assert!(logical_symbol::new("\u{2228}".to_string()).is_ok());
        assert!(logical_symbol::new("=>".to_string()).is_ok());
        assert!(logical_symbol::new("\u{00AC}".to_string()).is_ok());
        assert!(logical_symbol::new("<=>".to_string()).is_ok());
        assert!(logical_symbol::new("\u{2200}".to_string()).is_ok());
        assert!(logical_symbol::new("\u{018E}".to_string()).is_ok());
        assert!(logical_symbol::new("==".to_string()).is_ok());
        assert!(logical_symbol::new("(".to_string()).is_ok());
        assert!(logical_symbol::new(")".to_string()).is_ok());
    }

    #[test]
    fn logical_symbol_invalid() {
        assert!(logical_symbol::new("A".to_string()).is_err());
        assert!(logical_symbol::new("foo".to_string()).is_err());
        assert!(logical_symbol::new("".to_string()).is_err());
        assert!(logical_symbol::new("-|".to_string()).is_err());
    }

    #[test]
    fn term_variable() {
        assert!(term::new("A".to_string(), None, vec![]).is_ok());
        assert!(term::new("B'".to_string(), None, vec![]).is_ok());
        assert!(term::new("Z'''".to_string(), None, vec![]).is_ok());
    }

    #[test]
    fn term_constant() {
        assert!(term::new("foo".to_string(), Some(0), vec![]).is_ok());
        assert!(term::new("c1".to_string(), Some(0), vec![]).is_ok());
    }

    #[test]
    fn term_operation() {
        let var = term::new("X".to_string(), None, vec![]).unwrap();
        assert!(term::new("f".to_string(), Some(1), vec![var]).is_ok());

        let v1 = term::new("X".to_string(), None, vec![]).unwrap();
        let v2 = term::new("Y".to_string(), None, vec![]).unwrap();
        assert!(term::new("g".to_string(), Some(2), vec![v1, v2]).is_ok());
    }

    #[test]
    fn term_operation_wrong_arity() {
        let var = term::new("X".to_string(), None, vec![]).unwrap();
        assert!(term::new("f".to_string(), Some(2), vec![var]).is_err());
    }

    #[test]
    fn term_rejects_logical_symbol_as_constant() {
        assert!(term::new("\u{2227}".to_string(), Some(0), vec![]).is_err());
    }

    #[test]
    fn term_variable_supersedes_constant_rank() {
        // "A" is a valid individual_variable regardless of rank=Some(0)
        let t = term::new("A".to_string(), Some(0), vec![]).unwrap();
        assert!(matches!(t.term_type, TermType::Variable(_)));
    }

    #[test]
    fn formula_combination_conjunction() {
        // Build two atomic term formulas: Term(A) and Term(B)
        let t1 = term::new("A".to_string(), None, vec![]).unwrap();
        let t2 = term::new("B".to_string(), None, vec![]).unwrap();
        let f1 = Formula { formula_type: FormulaType::Term(t1), value: None };
        let f2 = Formula { formula_type: FormulaType::Term(t2), value: None };

        // Combine with /\ (conjunction)
        let conj = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let combo = Formula {
            formula_type: FormulaType::Combination(conj, vec![f1, f2]),
            value: None,
        };
        assert!(matches!(combo.formula_type, FormulaType::Combination(_, _)));
    }

    #[test]
    fn formula_combination_implication() {
        // P => Q where P = Term(X), Q = Term(Y)
        let p = Formula { formula_type: FormulaType::Term(term::new("X".to_string(), None, vec![]).unwrap()), value: None };
        let q = Formula { formula_type: FormulaType::Term(term::new("Y".to_string(), None, vec![]).unwrap()), value: None };

        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![p, q]),
            value: None,
        };
        assert!(matches!(formula.formula_type, FormulaType::Combination(_, _)));
    }

    #[test]
    fn formula_combination_nested() {
        // (A /\ B) \/ C
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let c = Formula { formula_type: FormulaType::Term(term::new("C".to_string(), None, vec![]).unwrap()), value: None };

        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let a_and_b = Formula { formula_type: FormulaType::Combination(and, vec![a, b]), value: None };

        let or = logical_symbol::new("\u{2228}".to_string()).unwrap();
        let result = Formula {
            formula_type: FormulaType::Combination(or, vec![a_and_b, c]),
            value: None,
        };
        assert!(matches!(result.formula_type, FormulaType::Combination(_, _)));
    }

    #[test]
    fn formula_quantifier_universal() {
        // ∀X . Term(X)
        let x = individual_variable::new("X").unwrap();
        let body = Formula { formula_type: FormulaType::Term(term::new("X".to_string(), None, vec![]).unwrap()), value: None };
        let forall = logical_symbol::new("\u{2200}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Quantifier(forall, x, Box::new(body)),
            value: None,
        };
        assert!(matches!(formula.formula_type, FormulaType::Quantifier(_, _, _)));
    }

    #[test]
    fn formula_quantifier_existential() {
        // ƎY . Term(Y)
        let y = individual_variable::new("Y").unwrap();
        let body = Formula { formula_type: FormulaType::Term(term::new("Y".to_string(), None, vec![]).unwrap()), value: None };
        let exists = logical_symbol::new("\u{018E}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Quantifier(exists, y, Box::new(body)),
            value: None,
        };
        assert!(matches!(formula.formula_type, FormulaType::Quantifier(_, _, _)));
    }

    #[test]
    fn is_true_nested_implication_f1_true_f2_true() {
        // f1 => (~f2 => f3) where f1 and f2 are true, f3 has no value (defaults to false)
        // ~f2 = false, so (~f2 => f3) is vacuously true, so f1 => true = true
        let f1 = Formula { formula_type: FormulaType::Term(term::new("P".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let f2 = Formula { formula_type: FormulaType::Term(term::new("Q".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let f3 = Formula { formula_type: FormulaType::Term(term::new("R".to_string(), Some(0), vec![]).unwrap()), value: None };

        let not = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_f2 = Formula {
            formula_type: FormulaType::Combination(not, vec![f2]),
            value: None,
        };
        let inner_implies = logical_symbol::new("=>".to_string()).unwrap();
        let inner = Formula {
            formula_type: FormulaType::Combination(inner_implies, vec![not_f2, f3]),
            value: None,
        };
        let outer_implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(outer_implies, vec![f1, inner]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_disjunction_true_or_not_false() {
        // A \/ ~B where A is true and B is false; ~B = true, so result should be true
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let not = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_b = Formula {
            formula_type: FormulaType::Combination(not, vec![b]),
            value: None,
        };
        let or = logical_symbol::new("\u{2228}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(or, vec![a, not_b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_iff_false_iff_false() {
        // A <=> B where A is false and B is false; result should be true
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let iff = logical_symbol::new("<=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(iff, vec![a, b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_iff_true_iff_false() {
        // A <=> B where A is true and B is false; result should be false
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let iff = logical_symbol::new("<=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(iff, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_true(&[]));
    }

    #[test]
    fn is_true_conjunction_false_and_false() {
        // A /\ B where A is false and B is false; result should be false
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(and, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_true(&[]));
    }

    #[test]
    fn is_true_disjunction_true_or_false() {
        // A \/ B where A is true and B is false; result should be true
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let or = logical_symbol::new("\u{2228}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(or, vec![a, b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_conjunction_true_and_true() {
        // A /\ B where A is true and B is true; result should be true
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(and, vec![a, b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_conjunction_true_and_false() {
        // A /\ B where A is true and B is false; result should be false
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(and, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_true(&[]));
    }

    #[test]
    fn is_true_implication_true_implies_true() {
        // A => B where A is true and B is true; result should be true
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![a, b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_implication_false_implies_any() {
        // A => B where A is false; result should be true regardless of B
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![a, b]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn is_true_implication_true_implies_no_value() {
        // A => B where A is true and B has no assigned value (defaults to false); result should be false
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_true(&[]));
    }

    #[test]
    fn is_true_implication_true_implies_false() {
        // A => B where A is true and B is false; result should be false
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: Some(false) };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_true(&[]));
    }

    #[test]
    fn is_true_implication_conjunction_f1_true_f3_any_implies_f2_true() {
        // (f1 /\ f3) => f2 where f1 and f2 are true and f3 is an arbitrary formula;
        // result should always be true:
        //   - if f3 is true:  (true /\ true) => true  = true => true  = true
        //   - if f3 is false/unset: (true /\ false) => true = false => true = true
        let f1 = Formula { formula_type: FormulaType::Term(term::new("P".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let f2 = Formula { formula_type: FormulaType::Term(term::new("Q".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let f3 = Formula { formula_type: FormulaType::Term(term::new("R".to_string(), Some(0), vec![]).unwrap()), value: None };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let antecedent = Formula {
            formula_type: FormulaType::Combination(and, vec![f1, f3]),
            value: None,
        };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![antecedent, f2]),
            value: None,
        };
        assert!(formula.is_true(&[]));
    }

    #[test]
    fn formula_quantifier_over_combination() {
        // ∀X . (Term(X) /\ Term(Y))
        let x = individual_variable::new("X").unwrap();
        let fx = Formula { formula_type: FormulaType::Term(term::new("X".to_string(), None, vec![]).unwrap()), value: None };
        let fy = Formula { formula_type: FormulaType::Term(term::new("Y".to_string(), None, vec![]).unwrap()), value: None };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let body = Formula { formula_type: FormulaType::Combination(and, vec![fx, fy]), value: None };
        let forall = logical_symbol::new("\u{2200}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Quantifier(forall, x, Box::new(body)),
            value: None,
        };
        assert!(matches!(formula.formula_type, FormulaType::Quantifier(_, _, _)));
    }

    #[test]
    fn is_tautology_excluded_middle() {
        // A \/ ¬A is a tautology
        let a1 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let a2 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let not = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_a = Formula { formula_type: FormulaType::Combination(not, vec![a2]), value: None };
        let or = logical_symbol::new("\u{2228}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(or, vec![a1, not_a]),
            value: None,
        };
        assert!(formula.is_tautology(&mut ProofTable::new()));
    }

    #[test]
    fn is_tautology_self_implication() {
        // A => A is a tautology
        let a1 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let a2 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(implies, vec![a1, a2]),
            value: None,
        };
        assert!(formula.is_tautology(&mut ProofTable::new()));
    }

    #[test]
    fn is_tautology_double_negation() {
        // ¬¬A <=> A is a tautology
        let a1 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let a2 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let not1 = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_a = Formula { formula_type: FormulaType::Combination(not1, vec![a1]), value: None };
        let not2 = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_not_a = Formula { formula_type: FormulaType::Combination(not2, vec![not_a]), value: None };
        let iff = logical_symbol::new("<=>".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(iff, vec![not_not_a, a2]),
            value: None,
        };
        assert!(formula.is_tautology(&mut ProofTable::new()));
    }

    #[test]
    fn is_tautology_conjunction_not_tautology() {
        // A /\ B is not a tautology (false when A=false or B=false)
        let a = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let and = logical_symbol::new("\u{2227}".to_string()).unwrap();
        let formula = Formula {
            formula_type: FormulaType::Combination(and, vec![a, b]),
            value: None,
        };
        assert!(!formula.is_tautology(&mut ProofTable::new()));
    }

    #[test]
    fn is_tautology_hypothetical_syllogism() {
        // (A => B) => ((B => C) => (A => C)) is a tautology
        let a1 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let a2 = Formula { formula_type: FormulaType::Term(term::new("A".to_string(), None, vec![]).unwrap()), value: None };
        let b1 = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let b2 = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let c1 = Formula { formula_type: FormulaType::Term(term::new("C".to_string(), None, vec![]).unwrap()), value: None };
        let c2 = Formula { formula_type: FormulaType::Term(term::new("C".to_string(), None, vec![]).unwrap()), value: None };

        // A => B
        let a_implies_b = Formula {
            formula_type: FormulaType::Combination(logical_symbol::new("=>".to_string()).unwrap(), vec![a1, b1]),
            value: None,
        };
        // B => C
        let b_implies_c = Formula {
            formula_type: FormulaType::Combination(logical_symbol::new("=>".to_string()).unwrap(), vec![b2, c1]),
            value: None,
        };
        // A => C
        let a_implies_c = Formula {
            formula_type: FormulaType::Combination(logical_symbol::new("=>".to_string()).unwrap(), vec![a2, c2]),
            value: None,
        };
        // (B => C) => (A => C)
        let inner = Formula {
            formula_type: FormulaType::Combination(logical_symbol::new("=>".to_string()).unwrap(), vec![b_implies_c, a_implies_c]),
            value: None,
        };
        // (A => B) => ((B => C) => (A => C))
        let formula = Formula {
            formula_type: FormulaType::Combination(logical_symbol::new("=>".to_string()).unwrap(), vec![a_implies_b, inner]),
            value: None,
        };
        assert!(formula.is_tautology(&mut ProofTable::new()));
    }

    // ── rule tests ─────────────────────────────────────────────────────────────

    #[test]
    fn rule_empty_head_rejected() {
        assert!(rule::new(vec![], vec![]).is_err());
    }

    #[test]
    fn rule_single_head_single_body_produces_implication() {
        // h :- b  ≡  b => h
        let h = Formula { formula_type: FormulaType::Term(term::new("H".to_string(), None, vec![]).unwrap()), value: None };
        let b = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let f = rule::new(vec![h], vec![b]).unwrap().to_formula().unwrap();
        assert!(matches!(f.formula_type, FormulaType::Combination(_, _)));
        if let FormulaType::Combination(sym, parts) = &f.formula_type {
            assert_eq!(sym.0, "=>");
            assert_eq!(parts.len(), 2);
        }
    }

    #[test]
    fn rule_multi_head_multi_body_produces_implication_of_conjunctions() {
        // h1, h2 :- b1, b2  ≡  (b1 ∧ b2) => (h1 ∧ h2)
        let h1 = Formula { formula_type: FormulaType::Term(term::new("H".to_string(), None, vec![]).unwrap()), value: None };
        let h2 = Formula { formula_type: FormulaType::Term(term::new("I".to_string(), None, vec![]).unwrap()), value: None };
        let b1 = Formula { formula_type: FormulaType::Term(term::new("B".to_string(), None, vec![]).unwrap()), value: None };
        let b2 = Formula { formula_type: FormulaType::Term(term::new("C".to_string(), None, vec![]).unwrap()), value: None };
        let f = rule::new(vec![h1, h2], vec![b1, b2]).unwrap().to_formula().unwrap();
        if let FormulaType::Combination(sym, parts) = &f.formula_type {
            assert_eq!(sym.0, "=>");
            // both sides are conjunctions
            assert!(matches!(parts[0].formula_type, FormulaType::Combination(_, _)));
            assert!(matches!(parts[1].formula_type, FormulaType::Combination(_, _)));
        } else {
            panic!("expected Combination");
        }
    }

    #[test]
    fn rule_empty_body_produces_unconditional_head() {
        // h :-   ≡  h  (no implication wrapper)
        let h = Formula { formula_type: FormulaType::Term(term::new("H".to_string(), None, vec![]).unwrap()), value: None };
        let f = rule::new(vec![h], vec![]).unwrap().to_formula().unwrap();
        // With a single head and no body, the result is just the head Term, not a Combination.
        assert!(matches!(f.formula_type, FormulaType::Term(_)));
    }

    #[test]
    fn rule_is_true_when_body_true_and_head_true() {
        // body = P (true), head = Q (true) → P => Q should be true
        let h = Formula { formula_type: FormulaType::Term(term::new("Q".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let b = Formula { formula_type: FormulaType::Term(term::new("P".to_string(), Some(0), vec![]).unwrap()), value: Some(true) };
        let f = rule::new(vec![h], vec![b]).unwrap().to_formula().unwrap();
        assert!(f.is_true(&[]));
    }

    #[test]
    fn rule_is_true_when_body_false() {
        // body = P (false), head = Q (false) → P => Q is vacuously true
        let h = Formula { formula_type: FormulaType::Term(term::new("Q".to_string(), Some(0), vec![]).unwrap()), value: Some(false) };
        let b = Formula { formula_type: FormulaType::Term(term::new("P".to_string(), Some(0), vec![]).unwrap()), value: Some(false) };
        let f = rule::new(vec![h], vec![b]).unwrap().to_formula().unwrap();
        assert!(f.is_true(&[]));
    }

}
