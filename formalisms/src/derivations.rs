use crate::Formula;
use crate::proofs::{Proof, ProofTable};
use crate::all_assignments;

pub struct Argument {
    pub premises: Vec<Formula>,
    pub conclusion: Formula,
}

impl Argument {
    pub fn build_premise_tables(&self) {
        for premise in self.premises.iter().filter(|p| p.value == Some(true)) {
            let vars = premise.collect_variables();
            let mut proof_table = ProofTable::new();

            for assignment in all_assignments(&vars) {
                let mut proof = Proof::new();
                proof.values.push(assignment.clone());
                let result = premise.evaluate_verbose(&assignment, &mut proof);
                if result {
                    proof_table.proofs.push(proof);
                }
            }

            proof_table.build_table();
        }
    }

    pub fn build_conclusion_table(&self) -> bool {
        // Collect all variables from premises and conclusion
        let mut vars: Vec<String> = self.premises.iter()
            .flat_map(|p| p.collect_variables())
            .chain(self.conclusion.collect_variables())
            .collect();
        vars.sort();
        vars.dedup();

        let mut proof_table = ProofTable::new();
        let mut valid = true;

        for assignment in all_assignments(&vars) {
            let all_premises_hold = self.premises.iter()
                .filter(|p| p.value == Some(true))
                .all(|p| p.evaluate(&assignment));

            if all_premises_hold {
                let mut proof = Proof::new();
                proof.values.push(assignment.clone());
                let result = self.conclusion.evaluate_verbose(&assignment, &mut proof);
                if !result {
                    valid = false;
                }
                proof_table.proofs.push(proof);
            }
        }

        proof_table.build_table();
        valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormulaType, term, logical_symbol};

    fn make_var(name: &str, value: Option<bool>) -> Formula {
        Formula {
            formula_type: FormulaType::Term(term::new(name.to_string(), None, vec![]).unwrap()),
            value,
        }
    }

    #[test]
    fn test_build_premise_tables_modus_tollens() {
        // ¬Q
        let not = logical_symbol::new("\u{00AC}".to_string()).unwrap();
        let not_q = Formula {
            formula_type: FormulaType::Combination(not, vec![make_var("Q", None)]),
            value: None,
        };

        // P => ¬Q  (value = Some(true))
        let implies = logical_symbol::new("=>".to_string()).unwrap();
        let p_implies_not_q = Formula {
            formula_type: FormulaType::Combination(implies, vec![make_var("P", None), not_q]),
            value: Some(true),
        };

        // P  (value = Some(true))
        let p = make_var("P", Some(true));

        let arg = Argument {
            premises: vec![p_implies_not_q, p],
            conclusion: make_var("Q", None),
        };

        arg.build_premise_tables();
    }
}
