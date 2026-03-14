use anyhow::Result;
use clap::{Parser, Subcommand};
use formalisms::individual_variable;

#[derive(Parser)]
#[command(name = "axiom")]
#[command(about = "A CLI tool", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Say hello
    Hello {
        /// Name to greet
        #[arg(short, long, default_value = "world")]
        name: String,
    },
}


fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hello { name } => {
            match individual_variable::new(name.clone()) {
                Ok(_) => println!("{name} is an individual variable"),
                Err(_) => println!("{name} is not an individual variable"),
            }
        }
    }

    Ok(())
}

// Test scripts:
// cargo run -- hello --name "A"       -> A is an individual variable
// cargo run -- hello --name "A'"      -> A' is an individual variable
// cargo run -- hello --name "A'''"    -> A''' is an individual variable
// cargo run -- hello --name "ABC"     -> ABC is not an individual variable
