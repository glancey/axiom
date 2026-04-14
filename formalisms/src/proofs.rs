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

impl Default for Proof {
    fn default() -> Self {
        Self::new()
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
}

impl Default for ProofTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofTable {

    pub fn merge(&mut self, other: ProofTable) {
        self.proofs.extend(other.proofs);
    }

    /// Injects fixed key-value pairs into every row of every proof in the table.
    pub fn inject_keys(&mut self, keys: &HashMap<String, bool>) {
        for proof in &mut self.proofs {
            for eval in &mut proof.evals {
                for (k, v) in keys {
                    eval.insert(k.clone(), *v);
                }
            }
        }
    }

    pub fn build_table(self) {
        if self.proofs.is_empty() {
            return;
        }
        // Union of all keys across all proofs, in first-seen order.
        let mut keys: Vec<String> = Vec::new();
        for proof in &self.proofs {
            for eval in &proof.evals {
                for k in eval.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
        }

        let mut builder = Builder::new();
        builder.push_record(keys.clone());

        for proof in &self.proofs {
            // Flatten all evals for this proof into one combined map.
            let mut combined: HashMap<String, bool> = HashMap::new();
            for eval in &proof.evals {
                combined.extend(eval.clone());
            }
            let row: Vec<String> = keys.iter()
                .map(|k| combined.get(k).map(|b| b.to_string()).unwrap_or_else(|| "-".to_string()))
                .collect();
            builder.push_record(row);
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


