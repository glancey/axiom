use anyhow::Result;
use formalisms::{
    operation_symbol, relation_symbol,
};
use axiom_syntalog::{parse_rule, parse_formula_as_rule};

/// Normalizes a natural-language logical symbol name into its symbol form.
/// Returns the mapped symbol string, or the original input if no mapping matches.
pub fn normalize_logical_symbol(s: &str) -> &str {
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
///
/// All other input is passed through unchanged.
pub fn normalize_formula(s: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with("not(") {
            result.push('\u{00AC}');
            result.push('(');
            i += 4;
        } else if s[i..].starts_with("not") {
            let after = s[i + 3..].chars().next();
            if after.is_some_and(|c| c.is_ascii_uppercase()) {
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

/// Parses a string of the form `name(arg1, arg2, ...)` into a name and argument list.
/// Returns an error if the parentheses or arguments are missing.
pub fn parse_symbol_args(s: &str, kind: &str) -> Result<(String, Vec<String>)> {
    let s = s.trim();
    let paren = s.find('(').ok_or_else(|| anyhow::anyhow!("expected '(' in {kind} symbol"))?;
    let name = s[..paren].trim().to_string();
    let rest = s[paren + 1..].trim();
    let rest = rest.strip_suffix(')').ok_or_else(|| anyhow::anyhow!("expected ')' at end of {kind} symbol"))?;
    let args: Vec<String> = rest.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    if args.is_empty() {
        anyhow::bail!("{kind}_symbol requires at least one argument");
    }
    Ok((name, args))
}

/// Normalizes and wraps a formula string in parentheses for parsing.
pub fn normalize_for_parse(s: &str) -> String {
    let n = normalize_formula(s);
    if !n.starts_with('(') { format!("({n})") } else { n }
}

/// Parses a rule string in either Prolog style or formula (`=>`) style.
pub fn parse_rule_input(s: &str) -> Result<axiom_syntalog::rule> {
    if s.contains("=>") { parse_formula_as_rule(s) } else { parse_rule(s) }
}

pub fn parse_operation_symbol(s: &str) -> Result<(operation_symbol, Vec<String>)> {
    let (name, args) = parse_symbol_args(s, "operation")?;
    let rank = args.len() as u32;
    let sym = operation_symbol::new(name, rank)?;
    Ok((sym, args))
}

/// Parses a string of the form `re(a1, a2, ..., an)` into a [`relation_symbol`] of rank n
/// and the corresponding `Vec<String>` of argument names.
///
/// Returns an error if the string is not well-formed, the argument list is empty,
/// n is not in the range 1–5, or the symbol name is invalid.
pub fn parse_relation_symbol(s: &str) -> Result<(relation_symbol, Vec<String>)> {
    let (name, args) = parse_symbol_args(s, "relation")?;
    let rank = args.len() as u32;
    if rank > 5 {
        anyhow::bail!("relation_symbol rank must be 1–5, got {rank}");
    }
    let sym = relation_symbol::new(name, rank)?;
    Ok((sym, args))
}


#[cfg(test)]
mod tests {
    use super::*;
    use formalisms::{
        individual_variable, operation_symbol, Formula,
    };
    use axiom_parser::parse_formula;
    use formalisms::proofs::ProofTable;

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
