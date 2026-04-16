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
        Commands::Compile => {
            let path = prompt("File path (.apl): ");
            axiom_prolog::compile(std::path::PathBuf::from(path));
        }
    }
}
