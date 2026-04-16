use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use axiom::helpers::normalize_for_parse;
use axiom_syntalog::{parse_rule, parse_formula_as_rule};
use scryer_prolog::{LeafAnswer, MachineBuilder, Term};
use serde_json::{json, Value};

/// Parses `input` as either a Prolog-style rule (`head :- body`) or an axiom
/// formula rule (`(body) => (head)`), then returns the canonical Prolog string
/// terminated with a period. Falls back to `normalize_for_parse` if neither parse succeeds.
pub fn to_prolog_string(input: &str) -> String {
    let rule_str = if input.contains("->") {
        parse_formula_as_rule(input).map(|r| r.to_string())
    } else {
        parse_rule(input).map(|r| r.to_string())
    }
    .unwrap_or_else(|_| normalize_for_parse(input));

    let rule_str = rule_str
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .map(|s| s.to_string())
        .unwrap_or(rule_str);

    if rule_str.ends_with('.') {
        rule_str
    } else {
        format!("{rule_str}.")
    }
}

/// Reads an `.apl` file line by line, converts each line with [`to_prolog_string`],
/// and writes the result to a `.pl` file at the same path.
pub fn compile(path: PathBuf) {
    if path.extension().and_then(|e| e.to_str()) != Some("apl") {
        eprintln!("error: expected a .apl file, got '{}'", path.display());
        return;
    }

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading {}: {e}", path.display()); return; }
    };

    let output: Vec<String> = source
        .lines()
        .map(|line| to_prolog_string(line))
        .collect();

    let out_path = path.with_extension("pl");
    if let Err(e) = std::fs::write(&out_path, output.join("\n")) {
        eprintln!("error writing {}: {e}", out_path.display());
        return;
    }

    println!("Compiled '{}' → '{}'", path.display(), out_path.display());
}

fn term_to_json(term: &Term) -> Value {
    match term {
        Term::Integer(n) => json!(n.to_string()),
        Term::Rational(r) => json!(r.to_string()),
        Term::Float(f) => json!(f),
        Term::Atom(s) => json!(s),
        Term::String(s) => json!(s),
        Term::Var(v) => json!(format!("_{v}")),
        Term::List(items) => Value::Array(items.iter().map(term_to_json).collect()),
        Term::Compound(functor, args) => json!({
            "functor": functor,
            "args": args.iter().map(term_to_json).collect::<Value>(),
        }),
        _ => json!(format!("{term:?}")),
    }
}

fn answer_to_json(answer: Result<LeafAnswer, Term>) -> Value {
    match answer {
        Ok(LeafAnswer::True) => json!({ "result": true }),
        Ok(LeafAnswer::False) => json!({ "result": false }),
        Ok(LeafAnswer::Exception(term)) => json!({ "error": term_to_json(&term) }),
        Ok(LeafAnswer::LeafAnswer { bindings, .. }) => {
            let bindings: serde_json::Map<String, Value> = bindings
                .iter()
                .map(|(k, v)| (k.clone(), term_to_json(v)))
                .collect();
            json!({ "bindings": bindings })
        }
        Err(term) => json!({ "error": term_to_json(&term) }),
    }
}

pub fn query(path: PathBuf) {
    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", path.display(), e);
            return;
        }
    };

    let module_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("user");

    let mut machine = MachineBuilder::default().build();
    machine.consult_module_string(module_name, source);

    println!("Loaded '{}' as module '{module_name}'.", path.display());
    println!("Enter a Prolog query (e.g. member(X, [1,2,3]).) or Ctrl-D to quit.\n");

    let stdin = io::stdin();
    loop {
        print!("?- ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }

        let query = line.trim();
        if query.is_empty() {
            continue;
        }

        let query = to_prolog_string(query);
        println!("Query: {query}");

        let mut answers: Vec<Value> = machine.run_query(query).map(answer_to_json).collect();
        // scryer-prolog appends a final False to signal "no more solutions"; drop it
        // when preceding answers exist so False only appears for genuinely failed queries.
        if answers.len() > 1 {
            if answers.last() == Some(&json!({ "result": false })) {
                answers.pop();
            }
        }

        println!("{}", serde_json::to_string_pretty(&answers).unwrap());
    }

    println!("\nGoodbye.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_prolog_string_unit_clause() {
        assert_eq!(to_prolog_string("is_weekday(monday)"), "is_weekday(monday).");
    }

    #[test]
    fn to_prolog_string_weekend_disjunction() {
        assert_eq!(
            to_prolog_string("(Day = saturday or  Day = sunday) -> is_weekend(Day)"),
            "is_weekend(Day) :- (Day = saturday ; Day = sunday).",
        );
    }
}
