use serde_json::Value;
use std::collections::HashSet;
use anyhow::Result;
use formalisms::{individual_constant, operation_symbol, operation, term, TermType};

// ── Vocabulary ───────────────────────────────────────────────────────────────

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
            for v in arr { collect_keys(v, keys); }
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

// ── Term construction helpers ─────────────────────────────────────────────────

fn op_term(sym: impl Into<String>, vars: Vec<term>) -> Result<term> {
    let s = operation_symbol::new(sym.into(), vars.len() as u32)?;
    let op = operation::new(s, vars)?;
    Ok(term { term_type: TermType::Operation(op) })
}

fn const_term(s: impl Into<String>) -> Result<term> {
    let c = individual_constant::new(s.into())?;
    Ok(term { term_type: TermType::Constant(c) })
}

// ── Symbol sanitisation ───────────────────────────────────────────────────────

/// Lowercase and replace non-alphanumeric chars with underscores.
/// Prepend "n" if the result starts with a digit.
fn sanitize_symbol(s: &str) -> String {
    let cleaned: String = s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let cleaned = if cleaned.is_empty() { "empty".to_string() } else { cleaned };
    if cleaned.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("n{cleaned}")
    } else {
        cleaned
    }
}

fn is_date(s: &str) -> bool {
    let p: Vec<&str> = s.splitn(3, '-').collect();
    p.len() == 3
        && p[0].len() == 4 && p[0].chars().all(|c| c.is_ascii_digit())
        && p[1].len() == 2 && p[1].chars().all(|c| c.is_ascii_digit())
        && p[2].len() == 2 && p[2].chars().all(|c| c.is_ascii_digit())
}

// ── JSON → term conversion ────────────────────────────────────────────────────

fn prolog_list(items: Vec<term>) -> Result<term> {
    let mut list = const_term("[]")?;
    for item in items.into_iter().rev() {
        list = op_term(".", vec![item, list])?;
    }
    Ok(list)
}

fn json_scalar_to_const(v: &Value) -> Result<term> {
    let s = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if is_date(s)              { s.clone() }
            else if s.parse::<f64>().is_ok() { s.clone() }
            else if s.chars().all(|c| c.is_alphanumeric()) { s.to_ascii_lowercase() }
            else                       { sanitize_symbol(s) }
        }
        Value::Bool(b)   => b.to_string(),
        Value::Null      => "null".to_string(),
        Value::Array(_)  => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    };
    const_term(s)
}

fn json_key_value_to_term(k: &str, v: &Value) -> Result<term> {
    let key = sanitize_symbol(k);
    match v {
        Value::Array(arr) if arr.is_empty() =>
            op_term(&key, vec![const_term("[]")?]),
        Value::Array(arr) => {
            let items: Result<Vec<term>> = arr.iter()
                .map(|item| json_to_term_labeled(item, &key))
                .collect();
            op_term(&key, vec![prolog_list(items?)?])
        }
        Value::Object(_) =>
            op_term(&key, vec![json_to_term_labeled(v, &key)?]),
        _ =>
            op_term(&key, vec![json_scalar_to_const(v)?]),
    }
}

/// Parse a nested JSON object into `sym(key1(val1), ...)`.
/// Uses the object's `"symbol"` field as the symbol if present, otherwise `label`.
fn json_to_term_labeled(value: &Value, label: &str) -> Result<term> {
    match value {
        Value::Object(map) => {
            if map.is_empty() { return const_term("[]"); }
            let sym = map.get("symbol")
                .and_then(|v| v.as_str())
                .map(sanitize_symbol)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "row".to_string());
            let vars: Result<Vec<term>> = map.iter()
                .map(|(k, v)| json_key_value_to_term(k, v))
                .collect();
            op_term(sym, vars?)
        }
        Value::Array(arr) => {
            let items: Result<Vec<term>> = arr.iter()
                .map(|item| json_to_term_labeled(item, label))
                .collect();
            prolog_list(items?)
        }
        _ => json_scalar_to_const(value),
    }
}

/// Parse a top-level JSON object into `sym([scalar_fields...], complex_fields...)`.
/// Scalar fields are collected into a Prolog list; arrays and objects are separate siblings.
/// Uses the object's `"symbol"` field as the functor if present, otherwise `label`.
pub fn json_to_term(value: &Value, label: String) -> Result<term> {
    match value {
        Value::Object(map) => {
            if map.is_empty() { return const_term("empty_object"); }
            let mut scalars: Vec<term> = Vec::new();
            let mut complex: Vec<term> = Vec::new();
            for (k, v) in map {
                let t = json_key_value_to_term(k, v)?;
                if matches!(v, Value::Array(_) | Value::Object(_)) {
                    complex.push(t);
                } else {
                    scalars.push(t);
                }
            }
            let mut top: Vec<term> = vec![prolog_list(scalars)?];
            top.extend(complex);
            op_term(label, top)
        }
        Value::Array(arr) => {
            let items: Result<Vec<term>> = arr.iter()
                .map(|item| json_to_term_labeled(item, &label))
                .collect();
            prolog_list(items?)
        }
        _ => json_scalar_to_const(value),
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

fn collect_list_items(t: &term, items: &mut Vec<String>) -> bool {
    match &t.term_type {
        TermType::Constant(c) if c.0.symbol == "[]" => true,
        TermType::Operation(op) if op.symbol.symbol == "." && op.vars.len() == 2 => {
            items.push(term_display(&op.vars[0]));
            collect_list_items(&op.vars[1], items)
        }
        _ => false,
    }
}

pub fn term_display(t: &term) -> String {
    match &t.term_type {
        TermType::Variable(v)  => v.name.clone(),
        TermType::Constant(c)  => c.0.symbol.clone(),
        TermType::Operation(op) if op.symbol.symbol == "." && op.vars.len() == 2 => {
            let mut items = Vec::new();
            if collect_list_items(t, &mut items) {
                format!("[{}]", items.join(", "))
            } else {
                let args: Vec<String> = op.vars.iter().map(term_display).collect();
                format!("{}({})", op.symbol.symbol, args.join(", "))
            }
        }
        TermType::Operation(op) => {
            let args: Vec<String> = op.vars.iter().map(term_display).collect();
            format!("{}({})", op.symbol.symbol, args.join(", "))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_json_to_term_ibm_week() {
        let json_str = r#"{
  "symbol": "IBM",
  "week": "2026-04-07 to 2026-04-10",
  "note": "April 11 (Good Friday) was a market holiday",
  "data": [
    {"date": "2026-04-07", "open": 245.32, "high": 245.76, "low": 241.10, "close": 245.07, "volume": 2356072},
    {"date": "2026-04-08", "open": 248.63, "high": 250.00, "low": 240.80, "close": 241.74, "volume": 4771476},
    {"date": "2026-04-09", "open": 240.88, "high": 241.74, "low": 233.76, "close": 237.18, "volume": 5078595},
    {"date": "2026-04-10", "open": 239.19, "high": 239.27, "low": 230.51, "close": 230.76, "volume": 5869361}
  ]
}"#;
        let value: Value = serde_json::from_str(json_str).unwrap();
        let t = json_to_term(&value, "stock".to_string()).unwrap();
        assert_eq!(
            term_display(&t),
            "stock([note(april_11__good_friday__was_a_market_holiday), symbol(ibm), week(n2026_04_07_to_2026_04_10)], data([row(close(245.07), date(2026-04-07), high(245.76), low(241.1), open(245.32), volume(2356072)), row(close(241.74), date(2026-04-08), high(250.0), low(240.8), open(248.63), volume(4771476)), row(close(237.18), date(2026-04-09), high(241.74), low(233.76), open(240.88), volume(5078595)), row(close(230.76), date(2026-04-10), high(239.27), low(230.51), open(239.19), volume(5869361))]))"
        );
    }
}
