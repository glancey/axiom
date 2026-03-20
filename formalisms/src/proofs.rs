use std::collections::HashMap;

pub struct Proofs {
    pub values: Vec<HashMap<String, bool>>,
    pub evals: Vec<HashMap<String, bool>>,
}

impl Proofs {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            evals: Vec::new(),
        }
    }
}
