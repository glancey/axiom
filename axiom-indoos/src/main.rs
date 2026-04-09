use anyhow::{bail, Result};
use axiom_indoos::{classify_line, induce_rule, proof_table_for_rule_valued, read_file};
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
    /// Induce hypotheses from background knowledge and examples
    Induce {
        /// Path to the background knowledge .pl file
        background: PathBuf,
        /// Path to the positive examples .pl file
        ex_plus: PathBuf,
        /// Path to the negative examples .pl file
        ex_minus: PathBuf,
    },
    /// Parse a rule and build a proof table for it
    ProveInduced {
        /// The rule string
        #[arg(trailing_var_arg = true)]
        input: Vec<String>,
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
        Command::Induce { background, ex_plus, ex_minus } => {
            let theory = axiom_indoos::induction::Theory::new(&background, &ex_plus, &ex_minus)?;
            println!("Terms: {:?}\n", theory.terms);
            let mut base: Vec<&String> = theory.base.iter().collect();
            base.sort();
            println!("Base: {base:?}\n");
            let mut interpretation: Vec<&String> = theory.interpretation.iter().collect();
            interpretation.sort();
            println!("Interpretation: {interpretation:?}\n");
            println!("Model:");
            let mut model: Vec<String> = theory.build_model().iter().map(|r| r.to_string()).collect();
            model.sort();
            for r in &model {
                println!("  {r}");
            }
        }
        Command::ProveInduced { input } => {
            let s = input.join(" ");
            match axiom_syntalog::parse_rule(&s) {
                Err(e) => bail!("error parsing rule: {e}"),
                Ok(r) => {
                    println!("rule: {r}");
                    match proof_table_for_rule_valued(&r, None) {
                        Err(e) => bail!("error building proof table: {e}"),
                        Ok(table) => table.build_table(),
                    }
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
