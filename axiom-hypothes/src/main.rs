use axiom_hypothes::Vocabulary;
use serde_json::Value;
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "hypothes", about = "Stock data analysis CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Diagnose {
        #[arg(help = "Path to the JSON file")]
        file: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diagnose { file } => {
            let file = match file {
                Some(f) => f,
                None => {
                    print!("Enter file path: ");
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                    input.trim().to_string()
                }
            };

            let contents = fs::read_to_string(&file)
                .unwrap_or_else(|e| { eprintln!("Error reading {file}: {e}"); std::process::exit(1); });

            let value: Value = serde_json::from_str(&contents)
                .unwrap_or_else(|e| { eprintln!("Error parsing JSON: {e}"); std::process::exit(1); });

            let vocab = Vocabulary::new(&value);
            println!("constants:  {:?}\n", vocab.constants);
            println!("functions:  {:?}\n", vocab.functions);
            println!("predicates: {:?}\n", vocab.predicates);

        }
    }
}

