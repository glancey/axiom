use formalisms::proofs::ProofTable;
use formalisms::derivations::Argument;
use anyhow::Result;
use clap::{Parser, Subcommand};
use formalisms::{
    individual_variable, logical_symbol, operation_symbol,
    individual_constant, relation_symbol, term, Formula,
};
use axiom_parser::parse_formula;
use axiom::helpers::{
    normalize_logical_symbol, normalize_formula, normalize_for_parse,
    parse_rule_input, parse_operation_symbol, parse_relation_symbol,
};


#[derive(Parser)]
#[command(name = "axiom")]
#[command(about = "A formal logic CLI for validating, parsing, and proving logical formulas", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a language construct (individual variable, constant, logical/operation/relation symbol, or term)
    Validate {
        /// The string representation of the construct to validate
        value: String,
        /// Additional arguments for operation_symbol validation; the number of args sets the rank
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print definitions and examples for all supported language constructs
    Glossary,
    /// Parse a string as a formula and report whether it is well-formed
    CheckFormula {
        /// The string to parse as a formula
        value: String,
    },
    /// Parse a formula and build a proof table showing whether it is a tautology
    TautologicalProof {
        /// The formula to evaluate across all truth assignments
        value: String,
    },
    /// Parse a rule and substitute the given terms for its variables (in appearance order), then pretty-print the result.
    /// Accepts either `h1, …, hn :- b1, …, bm` or the formula form `(b1 and … and bm) => (h1 and … and hn)`
    Substitution {
        /// Comma-separated terms to substitute, e.g. "alice,bob"
        terms: String,
        /// The rule string — Prolog style or formula style
        #[arg(trailing_var_arg = true)]
        rule: Vec<String>,
    },
    /// Parse a rule and print its JSON serialization.
    /// Accepts either `h1, …, hn :- b1, …, bm` or the formula form `(b1 and … and bm) -> (h1 and … and hn)`
    SerializeRule {
        /// The rule string — Prolog style or formula style
        #[arg(trailing_var_arg = true)]
        tokens: Vec<String>,
    },
    /// Parse a rule and validate it as a logical argument: body literals as premises, head literals
    /// as conclusion. Prints the argument form, checks tautology, then builds the proof table.
    /// Accepts either `h1, …, hn :- b1, …, bm` or the formula form `(b1 and … and bm) => (h1 and … and hn)`
    ValidateRule {
        /// The rule string — Prolog style or formula style
        #[arg(trailing_var_arg = true)]
        tokens: Vec<String>,
    },
    /// Validate a logical argument: build proof tables for premises and conclusion
    ValidateArgument {
        /// Premises separated by ' . ' and conclusion after ' :: ', e.g. "(P => notQ) . P :: notQ"
        #[arg(trailing_var_arg = true)]
        formulas: Vec<String>,
    },
}

/// Finds the byte offset of the first `->` that is not nested inside parentheses.
fn find_top_level_arrow(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => { if depth > 0 { depth -= 1; } }
            b'-' if depth == 0 && bytes[i + 1] == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { value, args } => {
            println!("Select type to validate against:");
            println!("  1. individual_variable");
            println!("  2. logical_symbol");
            println!("  3. operation_symbol");
            println!("  4. individual_constant");
            println!("  5. relation_symbol");
            println!("  6. term");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            let result: Result<String> = match input.trim() {
                "1" => individual_variable::new(&value).map(|_| format!("individual_variable({value})")),
                "2" => {
                    let sym = normalize_logical_symbol(&value);
                    logical_symbol::new(sym.to_string()).map(|_| format!("logical_symbol({sym})"))
                }
                "3" => {
                    if value.contains('(') {
                        parse_operation_symbol(&value).map(|(sym, _)| format!("operation_symbol({}, rank={})", sym.symbol, sym.rank))
                    } else if args.is_empty() {
                        anyhow::bail!("operation_symbol requires at least one argument");
                    } else {
                        let rank = args.len() as u32;
                        operation_symbol::new(value.clone(), rank).map(|_| format!("operation_symbol({value}, rank={rank})"))
                    }
                }
                "4" => individual_constant::new(value.clone()).map(|_| format!("individual_constant({value})")),
                "5" => {
                    if value.contains('(') {
                        parse_relation_symbol(&value).map(|(sym, _)| format!("relation_symbol({}, rank={})", sym.0.symbol, sym.0.rank))
                    } else if args.is_empty() {
                        anyhow::bail!("relation_symbol requires at least one argument");
                    } else {
                        let rank = args.len() as u32;
                        relation_symbol::new(value.clone(), rank).map(|_| format!("relation_symbol({value}, rank={rank})"))
                    }
                }
                "6" => term::new(value.clone(), None, vec![]).map(|_| format!("term({value})")),
                _ => anyhow::bail!("invalid selection"),
            };

            match result {
                Ok(output) => println!("{output}"),
                Err(e) => println!("Error: {e}"),
            }
        }
        Commands::CheckFormula { value } => {
            let normalized = normalize_formula(&value);
            if normalized.contains(":-") {
                match parse_rule_input(&normalized) {
                    Ok(r) => { println!("Valid rule: {normalized}\n{r:#?}"); }
                    Err(e) => println!("Invalid rule: {e}"),
                }
            } else {
                let is_symbol_application = normalized
                    .chars().next().map_or(false, |c| c.is_ascii_lowercase())
                    && normalized.contains('(');
                let formula_str = if is_symbol_application {
                    if let Ok((sym, args)) = parse_relation_symbol(&normalized) {
                        format!("{}({})", sym.0.symbol, args.join(", "))
                    } else if let Ok((sym, args)) = parse_operation_symbol(&normalized) {
                        format!("{}({})", sym.symbol, args.join(", "))
                    } else {
                        normalized
                    }
                } else {
                    normalized
                };
                let parse_str = if !formula_str.starts_with('(') { format!("({formula_str})") } else { formula_str.clone() };
                match parse_formula(&parse_str) {
                    Ok(ft) => println!("Valid formula: {formula_str}\n{ft:#?}"),
                    Err(e) => println!("Invalid formula: {e}"),
                }
            }
        }
        Commands::TautologicalProof { value } => {
            let mut proof_table = ProofTable::new();
            let parse_str = normalize_for_parse(&value);
            println!("Formula to prove: {parse_str}");
            match parse_formula(&parse_str) {
                Err(e) => println!("Invalid formula: {e}"),
                Ok(ft) => {
                    let formula = Formula { formula_type: ft, value: None };
                    if formula.is_tautology(&mut proof_table) {
                        println!("Tautology: {value}");
                    } else {
                        println!("Not a tautology: {value}");
                    }
                }
            }

            proof_table.build_table();

        }
        Commands::Substitution { terms, rule } => {
            let rule_str = normalize_formula(&rule.join(" "));
            match parse_rule_input(&rule_str) {
                Err(e) => println!("Error parsing rule: {e}"),
                Ok(r) => {
                    let subs: Result<Vec<formalisms::term>> = terms
                        .split(',')
                        .map(|t| formalisms::term::new(t.trim().to_string(), Some(0), vec![]))
                        .collect();
                    match subs {
                        Err(e) => println!("Error parsing terms: {e}"),
                        Ok(subs) => match r.substitution(subs) {
                            Err(e) => println!("Error: {e}"),
                            Ok(r2) => println!("{r2}\n{}", r2.to_json_pretty()),
                        },
                    }
                }
            }
        }
        Commands::SerializeRule { tokens } => {
            let input = normalize_formula(&tokens.join(" "));
            match parse_rule_input(&input) {
                Ok(r) => println!("{}", r.to_json_pretty()),
                Err(e) => println!("Error: {e}"),
            }
        }
        Commands::ValidateRule { tokens } => {
            let input = normalize_formula(&tokens.join(" "));
            match parse_rule_input(&input) {
                Err(e) => println!("Error parsing rule: {e}"),
                Ok(r) => {
                    let body_str = r.body.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(" . ");
                    let head_str = r.head.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(" . ");
                    println!("Validate: {body_str} :: {head_str}");
                    match r.to_formula() {
                        Err(e) => println!("Error building formula: {e}"),
                        Ok(formula) => {
                            let mut proof_table = ProofTable::new();
                            if formula.is_tautology(&mut proof_table) {
                                println!("Tautology: {input}");
                            } else {
                                println!("Not a tautology: {input}");
                            }
                            proof_table.build_table();
                        }
                    }
                }
            }
        }
        Commands::ValidateArgument { formulas } => {
            let input = normalize_formula(&formulas.join(" "));
            let (premises_str, conclusion_str) = if let Some((p, c)) = input.split_once(" :: ") {
                (p.to_string(), c.to_string())
            } else if let Some(arrow) = find_top_level_arrow(&input) {
                (input[..arrow].trim().to_string(), input[arrow + 2..].trim().to_string())
            } else {
                println!("Expected format: <premise> . <premise> :: <conclusion>");
                return Ok(());
            };

            let normalized_premises: Vec<String> = premises_str.split(" . ")
                .map(|p| normalize_formula(p.trim()).to_string())
                .collect();
            let normalized_conclusion = normalize_formula(conclusion_str.trim()).to_string();
            println!("Validate: [{}] -> {}", normalized_premises.join(" . "), normalized_conclusion);

            let mut premises = Vec::new();
            for part in premises_str.split(" . ") {
                let parse_str = normalize_for_parse(part.trim());
                match parse_formula(&parse_str) {
                    Err(e) => {
                        println!("Invalid premise '{}': {e}", part.trim());
                        return Ok(());
                    }
                    Ok(ft) => premises.push(Formula { formula_type: ft, value: Some(true) }),
                }
            }
            if premises.is_empty() {
                println!("No premises provided.");
                return Ok(());
            }
            let conclusion = match parse_formula(&normalize_for_parse(conclusion_str.trim())) {
                Ok(ft) => Formula { formula_type: ft, value: None },
                Err(e) => {
                    println!("Invalid conclusion '{}': {e}", conclusion_str.trim());
                    return Ok(());
                }
            };
            let arg = Argument { premises, conclusion };
            println!("Premises ---------------------------------");
            arg.build_premise_tables();

            println!("Conclusion =================================");
            let valid = arg.build_conclusion_table();
            if valid {
                println!("Argument is valid.");
            } else {
                println!("Argument is not valid: the conclusion is false for some assignment where all premises are true.");
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
            println!("  ∧ (and), ∨ (or), -> (implies), ¬ (not), <-> (iff),");
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
            println!();
            println!("ValidateArgument");
            println!("  Validates a logical argument by building proof tables for each premise");
            println!("  and a truth table for the conclusion under assignments where all premises hold.");
            println!("  Format: <premise> . <premise> ... :: <conclusion>");
            println!("  Example: (P -> notQ) . P :: notQ");
        }
    }

    Ok(())
}
