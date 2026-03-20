use std::collections::HashMap;
use std::fmt;

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
}

impl fmt::Debug for ProofTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProofTable")
            .field("proofs", &self.proofs)
            .finish()
    }
}


