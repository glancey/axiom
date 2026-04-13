use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "axiom-prolog")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Load a Prolog file and start an interactive query REPL
    Query,
    /// Normalize a string and print it as a Prolog-ready query
    Normalize,
    /// Compile a .apl file to a .pl file by converting each line with to_prolog_string
    Compile,
}

fn prompt(msg: &str) -> String {
    use std::io::{self, Write};
    print!("{msg}");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Query => {
            let path = prompt("Prolog file path: ");
            axiom_prolog::query(std::path::PathBuf::from(path));
        }
        Commands::Normalize => {
            let input = prompt("Input: ");
            println!("{}", axiom_prolog::to_prolog_string(&input));
        }
        Commands::Compile => {
            let path = prompt("File path (.apl): ");
            axiom_prolog::compile(std::path::PathBuf::from(path));
        }
    }
}
