use std::collections::HashMap;
use std::fmt;
use tabled::{builder::Builder, settings::Style};

pub struct Proof {
    pub values: Vec<HashMap<String, bool>>,
    pub evals: Vec<HashMap<String, bool>>,
}

impl Proof {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            evals: Vec::new(),
        }
    }
}

impl fmt::Debug for Proof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proof")
            .field("values", &self.values)
            .field("evals", &self.evals)
            .finish()
    }
}

pub struct ProofTable {
    pub proofs: Vec<Proof>,
}

impl ProofTable {
    pub fn new() -> Self {
        Self {
            proofs: Vec::new(),
        }
    }

    pub fn build_table(self) {
        let all_keys: Vec<&String> = self.proofs[0].evals.iter()
            .flat_map(|eval| eval.keys())
            .collect();
        let mut builder = Builder::with_capacity(all_keys.len(), self.proofs.len());
        builder.push_record(all_keys);

        for proof in self.proofs.iter() {
            let all_values: Vec<String> = proof.evals.iter()
                .flat_map(|v| v.values())
                .map(|b| b.to_string())
                .collect();
            builder.push_record(all_values);
        }

        let mut table = builder.build();
        table.with(Style::modern());
        println!("{table}");
    }
}

impl fmt::Debug for ProofTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProofTable")
            .field("proofs", &self.proofs)
            .finish()
    }
}


