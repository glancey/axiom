use serde_json::Value;
use std::collections::HashSet;

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn collect_keys<'a>(value: &'a Value, keys: &mut HashSet<&'a str>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                keys.insert(k.as_str());
                collect_keys(v, keys);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_keys(v, keys);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Default)]
pub struct Vocabulary {
    pub constants: HashSet<String>,
    pub functions: Vec<(String, String)>,
    pub predicates: HashSet<String>,
}

impl Vocabulary {
    pub fn new(value: &Value) -> Self {
        let mut constants = HashSet::new();
        let mut functions: Vec<(String, String)> = Vec::new();

        if let Some(symbol) = value.get("symbol").and_then(|v| v.as_str()) {
            constants.insert(symbol.to_string());
        }

        if let Some(data) = value.get("data").and_then(|v| v.as_array()) {
            for item in data {
                if let Some(map) = item.as_object() {
                    for (k, v) in map {
                        if let Some(s) = value_to_string(v) {
                            constants.insert(s.clone());
                            functions.push((k.clone(), s));
                        }
                    }
                }
            }
        }

        let mut predicate_refs: HashSet<&str> = HashSet::new();
        collect_keys(value, &mut predicate_refs);
        let predicates = predicate_refs.into_iter().map(|s| s.to_string()).collect();

        Self { constants, functions, predicates }
    }
}

