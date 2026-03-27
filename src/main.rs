use formalisms::proofs::ProofTable;
use formalisms::derivations::Argument;
use anyhow::Result;
use clap::{Parser, Subcommand};
use formalisms::{
    individual_variable, logical_symbol, operation_symbol, individual_constant,
    relation_symbol, term, Formula,
};
use axiom_parser::parse_formula;


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
    /// Greet a user by name
    Hello {
        /// Name to greet (defaults to "world")
        #[arg(short, long, default_value = "world")]
        name: String,
    },
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
    /// Validate a logical argument: build proof tables for premises and conclusion
    ValidateArgument {
        /// Premises separated by ' . ' and conclusion after ' :: ', e.g. "(P => notQ) . P :: notQ"
        #[arg(trailing_var_arg = true)]
        formulas: Vec<String>,
    },
}

/// Normalizes a natural-language logical symbol name into its symbol form.
/// Returns the mapped symbol string, or the original input if no mapping matches.
fn normalize_logical_symbol(s: &str) -> &str {
    match s.trim() {
        "and"          => "\u{2227}",
        "or"           => "\u{2228}",
        "implies"      => "=>",
        "not"          => "\u{00AC}",
        "iff"          => "<=>",
        "for all"      => "\u{2200}",
        "there exists" => "\u{018E}",
        "equals"       => "==",
        other          => other,
    }
}

/// Normalizes natural-language negation into the `¬` symbol before parsing.
/// - `not(expr)` → `¬(expr)`
/// - `notX` where X is an uppercase ASCII letter → `¬X`
/// All other input is passed through unchanged.
fn normalize_formula(s: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with("not(") {
            result.push('\u{00AC}');
            result.push('(');
            i += 4;
        } else if s[i..].starts_with("not") {
            let after = s[i + 3..].chars().next();
            if after.map_or(false, |c| c.is_ascii_uppercase()) {
                result.push('\u{00AC}');
                i += 3;
            } else {
                let c = s[i..].chars().next().unwrap();
                result.push(c);
                i += c.len_utf8();
            }
        } else {
            let c = s[i..].chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    result
}

/// Parses a string of the form `name(arg1, arg2, ...)` into an `operation_symbol`
/// and its argument list. Returns an error if the parentheses or arguments are missing.
fn parse_operation_symbol(s: &str) -> Result<(operation_symbol, Vec<String>)> {
    let s = s.trim();
    let paren = s.find('(').ok_or_else(|| anyhow::anyhow!("expected '(' in operation symbol"))?;
    let name = s[..paren].trim().to_string();
    let rest = s[paren + 1..].trim();
    let rest = rest.strip_suffix(')').ok_or_else(|| anyhow::anyhow!("expected ')' at end of operation symbol"))?;
    let args: Vec<String> = rest.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    if args.is_empty() {
        anyhow::bail!("operation_symbol requires at least one argument");
    }
    let rank = args.len() as u32;
    let sym = operation_symbol::new(name, rank)?;
    Ok((sym, args))
}

/// Parses a string of the form `re(a1, a2, ..., an)` into a [`relation_symbol`] of rank n
/// and the corresponding `Vec<String>` of argument names.
///
/// Returns an error if the string is not well-formed, the argument list is empty,
/// n is not in the range 1–5, or the symbol name is invalid.
fn parse_relation_symbol(s: &str) -> Result<(relation_symbol, Vec<String>)> {
    let s = s.trim();
    let paren = s.find('(').ok_or_else(|| anyhow::anyhow!("expected '(' in relation symbol"))?;
    let name = s[..paren].trim().to_string();
    let rest = s[paren + 1..].trim();
    let rest = rest.strip_suffix(')').ok_or_else(|| anyhow::anyhow!("expected ')' at end of relation symbol"))?;
    let args: Vec<String> = rest.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    if args.is_empty() {
        anyhow::bail!("relation_symbol requires at least one argument");
    }
    let rank = args.len() as u32;
    if rank > 5 {
        anyhow::bail!("relation_symbol rank must be 1–5, got {rank}");
    }
    let sym = relation_symbol::new(name, rank)?;
    Ok((sym, args))
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
        Commands::TautologicalProof { value } => {
            let mut proof_table = ProofTable::new();
            let normalized = normalize_formula(&value);
            let parse_str = if !normalized.starts_with('(') { format!("({normalized})") } else { normalized.clone() };
            println!("Formula to prove: {}", parse_str);
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
        Commands::ValidateArgument { formulas } => {
            let input = formulas.join(" ");
            let (premises_str, conclusion_str) = match input.split_once(" :: ") {
                Some((p, c)) => (p, c),
                None => {
                    println!("Expected format: <premise> . <premise> :: <conclusion>");
                    return Ok(());
                }
            };

            let normalized_premises: Vec<String> = premises_str.split(" . ")
                .map(|p| normalize_formula(p.trim()).to_string())
                .collect();
            let normalized_conclusion = normalize_formula(conclusion_str.trim()).to_string();
            println!("Validate: [{}] => {}", normalized_premises.join(" . "), normalized_conclusion);

            let mut premises = Vec::new();
            for part in premises_str.split(" . ") {
                let normalized = normalize_formula(part.trim());
                let parse_str = if !normalized.starts_with('(') {
                    format!("({normalized})")
                } else {
                    normalized.clone()
                };
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
            let normalized = normalize_formula(conclusion_str.trim());
            let parse_str = if !normalized.starts_with('(') {
                format!("({normalized})")
            } else {
                normalized.clone()
            };
            let conclusion = match parse_formula(&parse_str) {
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
            println!("  ∧ (and), ∨ (or), => (implies), ¬ (not), <=> (iff),");
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
            println!("  Example: (P => notQ) . P :: notQ");
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_logical_symbol_and() {
        assert_eq!(normalize_logical_symbol("and"), "\u{2227}");
    }

    #[test]
    fn normalize_logical_symbol_or() {
        assert_eq!(normalize_logical_symbol("or"), "\u{2228}");
    }

    #[test]
    fn normalize_logical_symbol_implies() {
        assert_eq!(normalize_logical_symbol("implies"), "=>");
    }

    #[test]
    fn normalize_logical_symbol_not() {
        assert_eq!(normalize_logical_symbol("not"), "\u{00AC}");
    }

    #[test]
    fn normalize_logical_symbol_iff() {
        assert_eq!(normalize_logical_symbol("iff"), "<=>");
    }

    #[test]
    fn normalize_logical_symbol_for_all() {
        assert_eq!(normalize_logical_symbol("for all"), "\u{2200}");
    }

    #[test]
    fn normalize_logical_symbol_there_exists() {
        assert_eq!(normalize_logical_symbol("there exists"), "\u{018E}");
    }

    #[test]
    fn normalize_logical_symbol_equals() {
        assert_eq!(normalize_logical_symbol("equals"), "==");
    }

    #[test]
    fn normalize_logical_symbol_passthrough() {
        assert_eq!(normalize_logical_symbol("=>"), "=>");
        assert_eq!(normalize_logical_symbol("\u{2227}"), "\u{2227}");
        assert_eq!(normalize_logical_symbol("unknown"), "unknown");
    }

    fn validate(value: &str) -> String {
        match individual_variable::new(value) {
            Ok(_) => format!("individual_variable({value})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    fn validate_operation_symbol(value: &str, args: &[&str]) -> String {
        if args.is_empty() {
            return "Error: operation_symbol requires at least one argument".to_string();
        }
        let rank = args.len() as u32;
        match operation_symbol::new(value.to_string(), rank) {
            Ok(_) => format!("operation_symbol({value}, rank={rank})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    fn validate_operation_symbol_from_str(s: &str) -> String {
        match parse_operation_symbol(s) {
            Ok((sym, _)) => format!("operation_symbol({}, rank={})", sym.symbol, sym.rank),
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
    fn validate_operation_symbol_rank_from_args() {
        assert_eq!(validate_operation_symbol("f", &["x"]), "operation_symbol(f, rank=1)");
        assert_eq!(validate_operation_symbol("f", &["x", "y", "z"]), "operation_symbol(f, rank=3)");
    }

    #[test]
    fn validate_operation_symbol_empty_args_is_error() {
        assert!(validate_operation_symbol("f", &[]).starts_with("Error:"));
    }

    #[test]
    fn validate_operation_symbol_zero_rank() {
        assert_eq!(validate_operation_symbol("f", &[]), "Error: operation_symbol requires at least one argument");
    }

    #[test]
    fn validate_operation_symbol_from_str_rank_1() {
        assert_eq!(validate_operation_symbol_from_str("f(a)"), "operation_symbol(f, rank=1)");
    }

    #[test]
    fn validate_operation_symbol_from_str_rank_3() {
        assert_eq!(validate_operation_symbol_from_str("op(a1, a2, a3)"), "operation_symbol(op, rank=3)");
    }

    #[test]
    fn validate_operation_symbol_from_str_empty_args_is_error() {
        assert!(validate_operation_symbol_from_str("f()").starts_with("Error:"));
    }

    #[test]
    fn validate_operation_symbol_from_str_missing_paren_is_error() {
        assert!(validate_operation_symbol_from_str("f").starts_with("Error:"));
    }

    fn validate_relation_symbol_from_str(s: &str) -> String {
        match parse_relation_symbol(s) {
            Ok((sym, _)) => format!("relation_symbol({}, rank={})", sym.0.symbol, sym.0.rank),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[test]
    fn validate_relation_symbol_from_str_rank_1() {
        assert_eq!(validate_relation_symbol_from_str("rel(a)"), "relation_symbol(rel, rank=1)");
    }

    #[test]
    fn validate_relation_symbol_from_str_rank_3() {
        assert_eq!(validate_relation_symbol_from_str("rel(a1, a2, a3)"), "relation_symbol(rel, rank=3)");
    }

    #[test]
    fn validate_relation_symbol_from_str_rank_5() {
        assert_eq!(validate_relation_symbol_from_str("rel(a1, a2, a3, a4, a5)"), "relation_symbol(rel, rank=5)");
    }

    #[test]
    fn validate_relation_symbol_from_str_empty_args_is_error() {
        assert!(validate_relation_symbol_from_str("rel()").starts_with("Error:"));
    }

    #[test]
    fn validate_relation_symbol_from_str_rank_6_is_error() {
        assert!(validate_relation_symbol_from_str("rel(a1, a2, a3, a4, a5, a6)").starts_with("Error:"));
    }

    #[test]
    fn validate_relation_symbol_from_str_missing_paren_is_error() {
        assert!(validate_relation_symbol_from_str("rel").starts_with("Error:"));
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

    fn check_formula_symbol(value: &str) -> String {
        let normalized = normalize_formula(value);
        let is_symbol_application = normalized
            .chars().next().map_or(false, |c| c.is_ascii_lowercase())
            && normalized.contains('(');
        if is_symbol_application {
            if let Ok((sym, args)) = parse_relation_symbol(&normalized) {
                return format!("relation_symbol({}, rank={}), args: {:?}", sym.0.symbol, sym.0.rank, args);
            }
            if let Ok((sym, args)) = parse_operation_symbol(&normalized) {
                return format!("operation_symbol({}, rank={}), args: {:?}", sym.symbol, sym.rank, args);
            }
        }
        let parse_str = if !normalized.starts_with('(') { format!("({normalized})") } else { normalized.clone() };
        match parse_formula(&parse_str) {
            Ok(_) => format!("Valid formula: {normalized}"),
            Err(e) => format!("Invalid formula: {e}"),
        }
    }

    #[test]
    fn check_formula_p_implies_q() {
        assert_eq!(check_formula("P=>Q"), "Valid formula: P=>Q");
    }

    #[test]
    fn check_formula_relation_symbol_rank_1() {
        assert_eq!(
            check_formula_symbol("rel(a)"),
            "relation_symbol(rel, rank=1), args: [\"a\"]"
        );
    }

    #[test]
    fn check_formula_relation_symbol_rank_3() {
        assert_eq!(
            check_formula_symbol("rel(a1, a2, a3)"),
            "relation_symbol(rel, rank=3), args: [\"a1\", \"a2\", \"a3\"]"
        );
    }

    #[test]
    fn check_formula_operation_symbol_rank_6() {
        assert_eq!(
            check_formula_symbol("op(a1, a2, a3, a4, a5, a6)"),
            "operation_symbol(op, rank=6), args: [\"a1\", \"a2\", \"a3\", \"a4\", \"a5\", \"a6\"]"
        );
    }

    #[test]
    fn check_formula_operation_symbol_rank_1() {
        assert_eq!(
            check_formula_symbol("op(a)"),
            "relation_symbol(op, rank=1), args: [\"a\"]"
        );
    }

    #[test]
    fn check_formula_no_parens_falls_back_to_formula() {
        assert_eq!(check_formula_symbol("P=>Q"), "Valid formula: P=>Q");
    }

    #[test]
    fn check_formula_not_variable() {
        assert_eq!(check_formula_symbol("not(A)"), "Valid formula: \u{00AC}(A)");
    }

    #[test]
    fn check_formula_not_implication() {
        assert_eq!(check_formula_symbol("not(A => B)"), "Valid formula: \u{00AC}(A => B)");
    }

    #[test]
    fn check_formula_not_conjunction() {
        assert_eq!(check_formula_symbol("not(A ∧ B)"), "Valid formula: \u{00AC}(A ∧ B)");
    }

    fn tautological_proof(value: &str) -> String {
        match parse_formula(value) {
            Err(e) => format!("Invalid formula: {e}"),
            Ok(ft) => {
                let formula = Formula { formula_type: ft, value: None };
                let mut proofs = ProofTable::new();
                if formula.is_tautology(&mut proofs) {
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
