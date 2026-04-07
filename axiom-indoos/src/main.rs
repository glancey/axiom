use anyhow::{bail, Result};
use axiom_indoos::{classify_line, induce_rule, read_file};
use axiom_syntalog::RuleType;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "axiom-indoos")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load and print the contents of a Prolog (.pl) file
    Load {
        /// Path to the .pl file
        path: PathBuf,
    },
    /// Classify an atom or literal and build a unit clause rule from it
    InduceRule {
        /// The atom or literal string (must not be a rule)
        #[arg(trailing_var_arg = true)]
        input: Vec<String>,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Load { path } => {
            if path.extension().and_then(|e| e.to_str()) != Some("pl") {
                bail!("expected a .pl file, got: {}", path.display());
            }
            let contents = read_file(&path)?;
            for line in contents.lines() {
                if let Some(result) = classify_line(line) {
                    println!("{line}: {result}");
                }
            }
        }
        Command::InduceRule { input } => {
            let s = input.join(" ");
            match classify_line(&s) {
                None => bail!("input is empty or a comment"),
                Some(classification) => {
                    println!("{s}: {classification}");
                    match induce_rule(&s) {
                        Err(e) => bail!("{e}"),
                        Ok(r) => {
                            println!("rule: {r}");
                            let value = matches!(r.rule_type, RuleType::Fact).then_some(true);
                            println!("{}", r.to_json_pretty_valued(value));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
