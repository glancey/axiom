use anyhow::Result;
use clap::{Parser, Subcommand};
use formalisms::{
    individual_variable, logical_symbol, operation_symbol, individual_constant,
    relation_symbol, operation, term, Formula,
};
use axiom_parser::parse_formula;

enum LanguageConstruct {
    IndividualVariable(individual_variable),
    LogicalSymbol(logical_symbol),
    OperationSymbol(operation_symbol),
    IndividualConstant(individual_constant),
    RelationSymbol(relation_symbol),
    Operation(operation),
    Term(term),
    Formula(Formula),
}

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
    /// Validate a string
    Validate {
        /// String to validate
        value: String,
    },
    /// Print descriptions of all language constructs
    Glossary,
    /// Check if a term is a valid formula 
    CheckFormula {
        /// Term to build the formula from
        value: String,
    },
    /// Parse a formula and return Ok if it is a tautology (is_true with empty context)
    TautologicalProof {
        /// Formula to evaluate
        value: String,
    },
}


fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hello { name } => {
            match individual_variable::new(&name) {
                Ok(_) => println!("Hello, {name}!"),
                Err(_) => println!("Name cannot be empty."),
            }
        }
        Commands::Validate { value } => {
            println!("Select type to validate against:");
            println!("  1. individual_variable");
            println!("  2. logical_symbol");
            println!("  3. operation_symbol");
            println!("  4. individual_constant");
            println!("  5. relation_symbol");
            println!("  6. term");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            let result: Result<LanguageConstruct> = match input.trim() {
                "1" => individual_variable::new(&value).map(LanguageConstruct::IndividualVariable),
                "2" => logical_symbol::new(value.clone()).map(LanguageConstruct::LogicalSymbol),
                "3" => {
                    print!("Enter rank: ");
                    let mut rank_input = String::new();
                    std::io::stdin().read_line(&mut rank_input)?;
                    let rank: u32 = rank_input.trim().parse()?;
                    operation_symbol::new(value.clone(), rank).map(LanguageConstruct::OperationSymbol)
                }
                "4" => individual_constant::new(value.clone()).map(LanguageConstruct::IndividualConstant),
                "5" => {
                    print!("Enter rank (1–5): ");
                    let mut rank_input = String::new();
                    std::io::stdin().read_line(&mut rank_input)?;
                    let rank: u32 = rank_input.trim().parse()?;
                    relation_symbol::new(value.clone(), rank).map(LanguageConstruct::RelationSymbol)
                }
                "6" => term::new(value.clone(), None, vec![]).map(LanguageConstruct::Term),
                _ => anyhow::bail!("invalid selection"),
            };

            match result {
                Ok(construct) => {
                    let name = match &construct {
                        LanguageConstruct::IndividualVariable(_) => "individual_variable",
                        LanguageConstruct::LogicalSymbol(_) => "logical_symbol",
                        LanguageConstruct::OperationSymbol(_) => "operation_symbol",
                        LanguageConstruct::IndividualConstant(_) => "individual_constant",
                        LanguageConstruct::RelationSymbol(_) => "relation_symbol",
                        LanguageConstruct::Term(_) => "term",
                        _ => unreachable!(),
                    };
                    println!("{name}({value})");
                }
                Err(e) => println!("Error: {e}"),
            }
        }
        Commands::CheckFormula { value } => {
            match parse_formula(&value) {
                Ok(ft) => println!("Valid formula: {value}\n{ft:#?}"),
                Err(e) => println!("Invalid formula: {e}"),
            }
        }
        Commands::TautologicalProof { value } => {
            match parse_formula(&value) {
                Err(e) => println!("Invalid formula: {e}"),
                Ok(ft) => {
                    let formula = Formula { formula_type: ft, value: None };
                    if formula.is_tautology() {
                        println!("Tautology: {value}");
                    } else {
                        println!("Not a tautology: {value}");
                    }
                }
            }
        }
        Commands::Glossary => {
            println!("individual_variable");
            println!("  A variable ranging over individuals in the domain.");
            println!("  Must be a single uppercase letter (A–Z), optionally followed by one or more apostrophes.");
            println!("  Examples: A, B', X'''");
            println!();
            println!("logical_symbol");
            println!("  One of the fixed logical connectives and punctuation symbols of the language:");
            println!("  ∧ (and), ∨ (or), => (implies), ~ (not), <=> (iff),");
            println!("  ∀ (for all), Ǝ (there exists), == (equals), (, )");
            println!();
            println!("operation_symbol");
            println!("  A named symbol used to build terms and relations.");
            println!("  Must not be a logical_symbol or an individual_variable.");
            println!("  Carries a rank indicating the number of arguments the symbol takes.");
            println!();
            println!("individual_constant");
            println!("  A zero-arity operation_symbol (rank 0) naming a fixed individual in the domain.");
            println!();
            println!("relation_symbol");
            println!("  An operation_symbol of rank 1–5 used to denote a relation between individuals.");
            println!();
            println!("operation");
            println!("  An operation_symbol of rank m > 0 applied to exactly m terms.");
            println!("  vars must have the same length as symbol.rank.");
            println!();
            println!("term");
            println!("  A term in the language: either an individual variable, an individual constant,");
            println!("  or an operation symbol of rank m > 0 applied to m sub-terms.");
            println!();
            println!("Formula");
            println!("  A well-formed formula (wff) of the language.");
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn validate(value: &str) -> String {
        match individual_variable::new(value) {
            Ok(_) => format!("individual_variable({value})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[test]
    fn validate_single_uppercase_letter() {
        assert_eq!(validate("A"), "individual_variable(A)");
    }

    #[test]
    fn validate_with_apostrophes() {
        assert_eq!(validate("A'"), "individual_variable(A')");
        assert_eq!(validate("A'''"), "individual_variable(A''')");
    }

    #[test]
    fn validate_multiple_uppercase_letters_is_error() {
        assert!(validate("ABC").starts_with("Error:"));
    }

    #[test]
    fn validate_lowercase_is_error() {
        assert!(validate("abc").starts_with("Error:"));
    }

    #[test]
    fn validate_empty_is_error() {
        assert!(validate("").starts_with("Error:"));
    }

    #[test]
    fn check_formula_implication() {
        assert!(parse_formula("P=>Q").is_ok());
        assert!(parse_formula("(P=>Q)").is_ok());
    }

    fn check_formula(value: &str) -> String {
        match parse_formula(value) {
            Ok(_) => format!("Valid formula: {value}"),
            Err(e) => format!("Invalid formula: {e}"),
        }
    }

    #[test]
    fn check_formula_p_implies_q() {
        assert_eq!(check_formula("P=>Q"), "Valid formula: P=>Q");
    }

    fn tautological_proof(value: &str) -> String {
        match parse_formula(value) {
            Err(e) => format!("Invalid formula: {e}"),
            Ok(ft) => {
                let formula = Formula { formula_type: ft, value: None };
                if formula.is_tautology() {
                    format!("Tautology: {value}")
                } else {
                    format!("Not a tautology: {value}")
                }
            }
        }
    }

    #[test]
    fn tautological_proof_invalid_formula() {
        assert!(tautological_proof("123").starts_with("Invalid formula:"));
    }

    #[test]
    fn tautological_proof_p_implies_q_is_not_tautology() {
        // P=>Q is false when P=true, Q=false
        assert_eq!(tautological_proof("P=>Q"), "Not a tautology: P=>Q");
    }

    #[test]
    fn tautological_proof_p_implies_p_is_tautology() {
        // P=>P is true under every assignment
        assert_eq!(tautological_proof("P=>P"), "Tautology: P=>P");
    }
}
