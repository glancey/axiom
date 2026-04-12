use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process;

use scryer_prolog::{LeafAnswer, MachineBuilder, Term};
use serde_json::{json, Value};

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

fn main() {
    let path: PathBuf = match std::env::args_os().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => {
            eprintln!("usage: scryer-prolog-embed <file.pl>");
            process::exit(1);
        }
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", path.display(), e);
            process::exit(1);
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
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }

        let query = line.trim().to_string();
        if query.is_empty() {
            continue;
        }

        // Ensure the query ends with a period, as Prolog requires.
        let query = if query.ends_with('.') {
            query
        } else {
            format!("{query}.")
        };

        let answers: Vec<Value> = machine.run_query(query).map(answer_to_json).collect();

        println!("{}", serde_json::to_string_pretty(&answers).unwrap());
    }

    println!("\nGoodbye.");
}
