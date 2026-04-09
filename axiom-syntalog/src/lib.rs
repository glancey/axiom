use formalisms::{
    individual_variable, individual_constant, logical_symbol, relation_symbol,
    operation_symbol, operation, term, Formula, FormulaType, TermType,
};
use std::fmt;
use anyhow::Result;

pub mod parse;
pub use parse::parse_rule;
pub use parse::parse_formula_as_rule;
pub use parse::parse_term;

/// An `operation_symbol` whose name begins with a lowercase ASCII letter.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct predicate_symbol(pub operation_symbol);

impl predicate_symbol {
    pub fn new(s: String, rank: u32) -> Result<Self> {
        match s.chars().next() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => anyhow::bail!("predicate_symbol must begin with a lowercase letter"),
        }
        Ok(predicate_symbol(operation_symbol::new(s, rank)?))
    }
}

/// A formula of the form `p(t1, …, tn)` where `p` is a `predicate_symbol`
/// of rank `n` and each `ti` is a `term`.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub struct atom {
    pub predicate: predicate_symbol,
    pub terms: Vec<term>,
}

impl atom {
    pub fn new(predicate: predicate_symbol, terms: Vec<term>) -> Result<Self> {
        let rank = predicate.0.rank as usize;
        if terms.len() != rank {
            anyhow::bail!(
                "atom requires exactly {rank} term(s) for predicate '{}', got {}",
                predicate.0.symbol,
                terms.len()
            );
        }
        Ok(atom { predicate, terms })
    }

    /// Collects the distinct `individual_variable`s in appearance order.
    pub fn variables(&self) -> Vec<individual_variable> {
        let mut seen = Vec::new();
        for t in &self.terms {
            collect_variables_term(t, &mut seen);
        }
        seen
    }

    /// Substitutes `subs` into both `self` and `other` positionally (the same
    /// terms into both), then checks whether the results are equal. Requires:
    /// - `self` and `other` have the same number of distinct variables
    /// - `subs.len()` == that count
    /// - `self` and `other` share no variable names
    pub fn unifies(&self, subs: Vec<term>, other: &atom) -> Result<bool> {
        let self_vars = self.variables();
        let other_vars = other.variables();
        if self_vars.len() != other_vars.len() {
            anyhow::bail!(
                "unifies requires equal variable counts, self has {} and other has {}",
                self_vars.len(), other_vars.len()
            );
        }
        if subs.len() != self_vars.len() {
            anyhow::bail!(
                "unifies requires {} term(s), got {}",
                self_vars.len(), subs.len()
            );
        }
        if self_vars.iter().any(|v| other_vars.contains(v)) {
            anyhow::bail!("unifies requires self and other to have no shared variable names");
        }
        let substituted_self  = substitute_atom(self.clone(),  &self_vars,  &subs);
        let substituted_other = substitute_atom(other.clone(), &other_vars, &subs);
        Ok(substituted_self == substituted_other)
    }
}

impl fmt::Display for atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.predicate.0.symbol)?;
        fmt_terms(&self.terms, f)?;
        write!(f, ")")
    }
}

/// If `t` is a proper Prolog list (`'.'`-chain ending in `[]`), returns its
/// elements; otherwise returns `None`.
fn try_collect_list(t: &term) -> Option<Vec<&term>> {
    match &t.term_type {
        TermType::Constant(c) if c.0.symbol == "[]" => Some(vec![]),
        TermType::Operation(op) if op.symbol.symbol == "." && op.vars.len() == 2 => {
            let mut tail = try_collect_list(&op.vars[1])?;
            let mut elems = vec![&op.vars[0] as &term];
            elems.append(&mut tail);
            Some(elems)
        }
        _ => None,
    }
}

fn fmt_term(t: &term, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &t.term_type {
        TermType::Variable(v) => write!(f, "{}", v.name),
        TermType::Constant(c) => write!(f, "{}", c.0.symbol),
        TermType::Operation(op) => {
            if let Some(elems) = try_collect_list(t) {
                write!(f, "[")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    fmt_term(e, f)?;
                }
                write!(f, "]")
            } else {
                write!(f, "{}(", op.symbol.symbol)?;
                for (i, v) in op.vars.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    fmt_term(v, f)?;
                }
                write!(f, ")")
            }
        }
    }
}

fn fmt_terms(terms: &[term], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, t) in terms.iter().enumerate() {
        if i > 0 { write!(f, ", ")?; }
        fmt_term(t, f)?;
    }
    Ok(())
}

/// Returns the display string of a [`term`], using list bracket notation for
/// proper Prolog lists (`'.'`-chains ending in `[]`).
pub fn term_to_string(t: &term) -> String {
    struct D<'a>(&'a term);
    impl fmt::Display for D<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt_term(self.0, f) }
    }
    format!("{}", D(t))
}

/// An operation is *ground* if none of its vars — at any depth — are
/// `individual_variable`s (which always begin with an uppercase letter).
/// Example: `op(a, b, c)` is ground; `op(A, B, C)` is not.
#[allow(non_camel_case_types)]
pub trait is_ground {
    fn is_ground(&self) -> bool;
}

impl is_ground for term {
    fn is_ground(&self) -> bool {
        match &self.term_type {
            TermType::Variable(_) => false,
            TermType::Constant(_) => true,
            TermType::Operation(op) => op.is_ground(),
        }
    }
}

impl is_ground for operation {
    fn is_ground(&self) -> bool {
        self.vars.iter().all(|t| t.is_ground())
    }
}

impl is_ground for atom {
    fn is_ground(&self) -> bool {
        self.terms.iter().all(|t| t.is_ground())
    }
}

/// A `literal` is either a positive or negative occurrence of a predicate applied to terms.
/// A `positive_literal` is an `atom` — a predicate symbol applied to terms with no negation.
/// A `negative_literal` is a predicate symbol preceded by negation ("not", '-', or '¬');
/// it shares the structure of an atom but is not one.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub enum literal {
    positive_literal(atom),
    negative_literal(predicate_symbol, Vec<term>),
}

impl literal {
    pub fn negative(predicate: predicate_symbol, terms: Vec<term>) -> Result<Self> {
        let rank = predicate.0.rank as usize;
        if terms.len() != rank {
            anyhow::bail!(
                "negative_literal requires exactly {rank} term(s) for predicate '{}', got {}",
                predicate.0.symbol,
                terms.len()
            );
        }
        Ok(literal::negative_literal(predicate, terms))
    }
}

impl fmt::Display for literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            literal::positive_literal(a) => write!(f, "{a}"),
            literal::negative_literal(p, terms) => {
                write!(f, "¬{}(", p.0.symbol)?;
                fmt_terms(terms, f)?;
                write!(f, ")")
            }
        }
    }
}

impl is_ground for literal {
    fn is_ground(&self) -> bool {
        match self {
            literal::positive_literal(a) => a.is_ground(),
            literal::negative_literal(_, terms) => terms.iter().all(|t| t.is_ground()),
        }
    }
}

// ── literal → Formula conversion helpers ────────────────────────────────────

/// Converts a single [`literal`] into a [`Formula`].
///
/// - Positive literal with rank 0 predicate → `FormulaType::Term` (individual constant).
/// - Positive literal with rank 1–5 predicate → `FormulaType::Relation`.
/// - Negative literal → `FormulaType::Combination(¬, [inner])`.
///
/// Returns an error if the predicate rank is 0 < rank ≤ 5 is violated (i.e. rank > 5).
fn literal_to_formula(lit: &literal) -> Result<Formula> {
    match lit {
        literal::positive_literal(a) => predicate_to_formula(&a.predicate, &a.terms),
        literal::negative_literal(pred, terms) => {
            let inner = predicate_to_formula(pred, terms)?;
            let not = logical_symbol::new("\u{00AC}".to_string())?;
            Ok(Formula { formula_type: FormulaType::Combination(not, vec![inner]), value: None })
        }
    }
}

fn predicate_to_formula(pred: &predicate_symbol, terms: &[term]) -> Result<Formula> {
    let rank = pred.0.rank;
    if rank == 0 {
        let c = individual_constant::new(pred.0.symbol.clone())?;
        let t = term { term_type: TermType::Constant(c) };
        Ok(Formula { formula_type: FormulaType::Term(t), value: None })
    } else {
        let rel = relation_symbol::new(pred.0.symbol.clone(), rank)
            .map_err(|_| anyhow::anyhow!(
                "predicate '{}' has rank {rank} which exceeds the relation_symbol maximum of 5",
                pred.0.symbol
            ))?;
        Ok(Formula { formula_type: FormulaType::Relation(rel, terms.to_vec()), value: None })
    }
}

/// Folds a non-empty literal slice into a single conjunction `Formula`.
/// A single literal is returned as-is (no conjunction wrapper).
fn literals_to_conjunction(lits: &[literal]) -> Result<Formula> {
    if lits.is_empty() {
        anyhow::bail!("cannot build a conjunction formula from an empty literal list");
    }
    if lits.len() == 1 {
        return literal_to_formula(&lits[0]);
    }
    let formulas: Vec<Formula> = lits.iter().map(literal_to_formula).collect::<Result<_>>()?;
    let and = logical_symbol::new("\u{2227}".to_string())?;
    Ok(Formula { formula_type: FormulaType::Combination(and, formulas), value: None })
}

/// Builds the `FormulaType::Combination` representation of a rule:
///
/// | head | body | formula |
/// |------|------|---------|
/// | non-empty | non-empty | `(body₁ ∧ … ∧ bodyₘ) => (head₁ ∧ … ∧ headₙ)` |
/// | non-empty | empty     | `head₁ ∧ … ∧ headₙ`  (unconditional assertion) |
/// | empty     | non-empty | `body₁ ∧ … ∧ bodyₘ`  (goal / query to be proved) |
fn build_rule_formula(head: &[literal], body: &[literal]) -> Result<Formula> {
    match (head.is_empty(), body.is_empty()) {
        (false, false) => {
            let body_f = literals_to_conjunction(body)?;
            let head_f = literals_to_conjunction(head)?;
            let implies = logical_symbol::new("=>".to_string())?;
            Ok(Formula {
                formula_type: FormulaType::Combination(implies, vec![body_f, head_f]),
                value: None,
            })
        }
        (false, true) => literals_to_conjunction(head),
        (true, false) => literals_to_conjunction(body),
        (true, true) => anyhow::bail!("rule must have at least a head or a body"),
    }
}

/// Discriminates the structural form of a `rule`.
#[derive(Debug, Clone)]
pub enum RuleType {
    /// `h1, …, hn :- b1, …, bm` — general clause with head and body.
    General,
    /// `h1, …, hn` — non-empty head, no body; asserts head unconditionally.
    UnitClause,
    /// `:- b1, …, bm` — no head; a query or goal to be proved.
    Goal,
    /// `h :- b1, …, bm` — exactly one positive head literal.
    DefiniteClause,
    /// At most one positive literal across head and body.
    HornRule,
    /// Ground `UnitClause` — no variables anywhere in the head.
    Fact,
}

/// A clause of the form `h1, …, hn :- b1, …, bm` where each `hi` and `bj`
/// is a `literal`. The `rule_type` records which structural subclass this
/// instance belongs to.
///
/// Call [`rule::to_formula`] to obtain the corresponding `Formula`:
/// - General/Definite/Horn: `(b1 ∧ … ∧ bm) => (h1 ∧ … ∧ hn)`
/// - UnitClause/Fact (no body): `h1 ∧ … ∧ hn`
/// - Goal (no head): `b1 ∧ … ∧ bm`
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct rule {
    pub head: Vec<literal>,
    pub body: Vec<literal>,
    pub rule_type: RuleType,
}

impl rule {
    /// General clause — no structural restrictions.
    pub fn new(head: Vec<literal>, body: Vec<literal>) -> Result<Self> {
        Ok(rule { head, body, rule_type: RuleType::General })
    }

    /// Non-empty head, empty body.
    pub fn unit_clause(head: Vec<literal>) -> Result<Self> {
        if head.is_empty() {
            anyhow::bail!("unit_clause head must contain at least one literal");
        }
        Ok(rule { head, body: vec![], rule_type: RuleType::UnitClause })
    }

    /// Empty head, non-empty body.
    pub fn goal(body: Vec<literal>) -> Result<Self> {
        if body.is_empty() {
            anyhow::bail!("goal body must contain at least one literal");
        }
        Ok(rule { head: vec![], body, rule_type: RuleType::Goal })
    }

    /// Exactly one positive head literal, any body.
    pub fn definite_clause(head: Vec<literal>, body: Vec<literal>) -> Result<Self> {
        match head.as_slice() {
            [literal::positive_literal(_)] => {}
            [_] => anyhow::bail!("definite_clause head must be a positive literal"),
            _ => anyhow::bail!(
                "definite_clause must have exactly one head literal, found {}",
                head.len()
            ),
        }
        Ok(rule { head, body, rule_type: RuleType::DefiniteClause })
    }

    /// At most one positive literal across head and body combined.
    pub fn horn(head: Vec<literal>, body: Vec<literal>) -> Result<Self> {
        let positive_count = head.iter().chain(body.iter())
            .filter(|lit| matches!(lit, literal::positive_literal(_)))
            .count();
        if positive_count > 1 {
            anyhow::bail!(
                "horn rule may contain at most one positive literal, found {positive_count}"
            );
        }
        Ok(rule { head, body, rule_type: RuleType::HornRule })
    }

    /// Ground unit clause — non-empty head, empty body, no individual variables.
    pub fn fact(head: Vec<literal>) -> Result<Self> {
        if head.is_empty() {
            anyhow::bail!("fact head must contain at least one literal");
        }
        if head.iter().any(|lit| !lit.is_ground()) {
            anyhow::bail!("fact must be ground: head may not contain individual_variables");
        }
        Ok(rule { head, body: vec![], rule_type: RuleType::Fact })
    }

    /// Returns the `Formula` representation of this rule.
    /// For `Fact` rules the formula's `value` is set to `Some(true)`.
    pub fn to_formula(&self) -> Result<Formula> {
        let mut f = build_rule_formula(&self.head, &self.body)?;
        if matches!(self.rule_type, RuleType::Fact) {
            f.value = Some(true);
        }
        Ok(f)
    }
}

impl fmt::Display for rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, lit) in self.head.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{lit}")?;
        }
        if !self.body.is_empty() {
            if !self.head.is_empty() { write!(f, " ")?; }
            write!(f, ":- ")?;
            for (i, lit) in self.body.iter().enumerate() {
                if i > 0 { write!(f, ", ")?; }
                write!(f, "{lit}")?;
            }
        }
        write!(f, ".")
    }
}

impl is_ground for rule {
    fn is_ground(&self) -> bool {
        self.head.iter().chain(self.body.iter()).all(|lit| lit.is_ground())
    }
}

fn substitute_term(t: term, vars: &[individual_variable], subs: &[term]) -> term {
    match t.term_type {
        TermType::Variable(ref v) => {
            match vars.iter().position(|var| var == v) {
                Some(i) => subs[i].clone(),
                None => t,
            }
        }
        TermType::Constant(_) => t,
        TermType::Operation(op) => {
            let new_vars = op.vars.into_iter().map(|v| substitute_term(v, vars, subs)).collect();
            term { term_type: TermType::Operation(operation { symbol: op.symbol, vars: new_vars }) }
        }
    }
}

fn substitute_terms(terms: Vec<term>, vars: &[individual_variable], subs: &[term]) -> Vec<term> {
    terms.into_iter().map(|t| substitute_term(t, vars, subs)).collect()
}

fn substitute_atom(a: atom, vars: &[individual_variable], subs: &[term]) -> atom {
    atom {
        predicate: a.predicate,
        terms: substitute_terms(a.terms, vars, subs),
    }
}

fn substitute_literal(lit: literal, vars: &[individual_variable], subs: &[term]) -> literal {
    match lit {
        literal::positive_literal(a) => literal::positive_literal(substitute_atom(a, vars, subs)),
        literal::negative_literal(p, terms) => {
            literal::negative_literal(p, substitute_terms(terms, vars, subs))
        }
    }
}

fn collect_variables_term(t: &term, seen: &mut Vec<individual_variable>) {
    match &t.term_type {
        TermType::Variable(v) => {
            if !seen.contains(v) { seen.push(v.clone()); }
        }
        TermType::Constant(_) => {}
        TermType::Operation(op) => {
            for v in &op.vars { collect_variables_term(v, seen); }
        }
    }
}

fn collect_variables_literal(lit: &literal, seen: &mut Vec<individual_variable>) {
    let terms = match lit {
        literal::positive_literal(a) => &a.terms,
        literal::negative_literal(_, terms) => terms,
    };
    for t in terms { collect_variables_term(t, seen); }
}

impl rule {
    /// Collects the distinct `individual_variable`s in the rule in the order
    /// they first appear, scanning head then body left to right.
    pub fn variables(&self) -> Vec<individual_variable> {
        let mut seen = Vec::new();
        for lit in self.head.iter().chain(self.body.iter()) {
            collect_variables_literal(lit, &mut seen);
        }
        seen
    }

    /// Returns a new rule with each `individual_variable` replaced by the
    /// corresponding term in `subs`. `subs` must have exactly as many elements
    /// as there are distinct variables in the rule (in appearance order).
    pub fn substitution(self, subs: Vec<term>) -> Result<Self> {
        let vars = self.variables();
        if subs.len() != vars.len() {
            anyhow::bail!(
                "substitution requires {} term(s) for {} variable(s), got {}",
                vars.len(), vars.len(), subs.len()
            );
        }
        let head: Vec<literal> = self.head.into_iter().map(|lit| substitute_literal(lit, &vars, &subs)).collect();
        let body: Vec<literal> = self.body.into_iter().map(|lit| substitute_literal(lit, &vars, &subs)).collect();
        Ok(rule { head, body, rule_type: self.rule_type })
    }
}

fn json_term(t: &term) -> String {
    match &t.term_type {
        TermType::Variable(v) =>
            format!(r#"{{"type":"variable","name":"{}"}}"#, v.name),
        TermType::Constant(c) =>
            format!(r#"{{"type":"constant","name":"{}"}}"#, c.0.symbol),
        TermType::Operation(op) => {
            let args: Vec<String> = op.vars.iter().map(json_term).collect();
            format!(r#"{{"type":"operation","symbol":"{}","args":[{}]}}"#,
                op.symbol.symbol, args.join(","))
        }
    }
}

fn json_atom(a: &atom) -> String {
    let terms: Vec<String> = a.terms.iter().map(json_term).collect();
    format!(r#"{{"predicate":"{}","terms":[{}]}}"#,
        a.predicate.0.symbol, terms.join(","))
}

fn json_literal(lit: &literal) -> String {
    match lit {
        literal::positive_literal(a) =>
            format!(r#"{{"polarity":"positive","atom":{}}}"#, json_atom(a)),
        literal::negative_literal(p, terms) => {
            let ts: Vec<String> = terms.iter().map(json_term).collect();
            format!(r#"{{"polarity":"negative","predicate":"{}","terms":[{}]}}"#,
                p.0.symbol, ts.join(","))
        }
    }
}

fn json_formula(f: &Formula) -> String {
    let value = match f.value {
        Some(true)  => "true",
        Some(false) => "false",
        None        => "null",
    };
    let ft = json_formula_type(&f.formula_type);
    // ft is a JSON object starting with `{`; insert "value" as the first field
    format!(r#"{{"value":{},{}"#, value, &ft[1..])
}

fn json_formula_type(ft: &FormulaType) -> String {
    match ft {
        FormulaType::Term(t) =>
            format!(r#"{{"type":"term","term":{}}}"#, json_term(t)),
        FormulaType::Relation(rel, terms) => {
            let ts: Vec<String> = terms.iter().map(json_term).collect();
            format!(r#"{{"type":"relation","symbol":"{}","terms":[{}]}}"#,
                rel.0.symbol, ts.join(","))
        }
        FormulaType::Combination(sym, formulas) => {
            let fs: Vec<String> = formulas.iter().map(json_formula).collect();
            format!(r#"{{"type":"combination","connective":"{}","operands":[{}]}}"#,
                sym.symbol(), fs.join(","))
        }
        FormulaType::Quantifier(sym, var, body) =>
            format!(r#"{{"type":"quantifier","quantifier":"{}","variable":"{}","body":{}}}"#,
                sym.symbol(), var.name, json_formula(body)),
    }
}

fn json_pretty(s: &str) -> String {
    let mut out = String::new();
    let mut depth: usize = 0;
    let indent = "  ";
    let mut in_string = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' { i += 1; if i < chars.len() { out.push(chars[i]); } }
            else if c == '"' { in_string = false; }
        } else {
            match c {
                '"' => { in_string = true; out.push(c); }
                '{' | '[' => {
                    depth += 1;
                    out.push(c);
                    out.push('\n');
                    out.push_str(&indent.repeat(depth));
                }
                '}' | ']' => {
                    depth -= 1;
                    out.push('\n');
                    out.push_str(&indent.repeat(depth));
                    out.push(c);
                }
                ',' => {
                    out.push(c);
                    out.push('\n');
                    out.push_str(&indent.repeat(depth));
                }
                ':' => { out.push(c); out.push(' '); }
                c if c.is_whitespace() => {}
                _ => { out.push(c); }
            }
        }
        i += 1;
    }
    out
}

impl rule {
    pub fn to_json(&self) -> String {
        self.to_json_valued(None)
    }

    pub fn to_json_valued(&self, value: Option<bool>) -> String {
        let rule_type = match self.rule_type {
            RuleType::General        => "General",
            RuleType::UnitClause     => "UnitClause",
            RuleType::Goal           => "Goal",
            RuleType::DefiniteClause => "DefiniteClause",
            RuleType::HornRule       => "HornRule",
            RuleType::Fact           => "Fact",
        };
        let head: Vec<String> = self.head.iter().map(json_literal).collect();
        let body: Vec<String> = self.body.iter().map(json_literal).collect();
        let formula = self.to_formula()
            .map(|mut f| { f.value = value; json_formula(&f) })
            .unwrap_or_else(|_| "null".to_string());
        format!(r#"{{"rule_type":"{}","head":[{}],"body":[{}],"formula":{}}}"#,
            rule_type, head.join(","), body.join(","), formula)
    }

    pub fn to_json_pretty(&self) -> String {
        json_pretty(&self.to_json())
    }

    pub fn to_json_pretty_valued(&self, value: Option<bool>) -> String {
        json_pretty(&self.to_json_valued(value))
    }
}

/// A collection of `rule`s forming a logical theory.
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct clausal_theory {
    pub rules: Vec<rule>,
}

impl clausal_theory {
    pub fn new(rules: Vec<rule>) -> Self {
        clausal_theory { rules }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_atom(name: &str, arg: &str) -> atom {
        let p = predicate_symbol::new(name.to_string(), 1).unwrap();
        let terms = vec![term::new(arg.to_string(), Some(0), vec![]).unwrap()];
        atom::new(p, terms).unwrap()
    }

    fn make_atom_2(name: &str, arg1: &str, arg2: &str) -> atom {
        let p = predicate_symbol::new(name.to_string(), 2).unwrap();
        let terms = vec![
            term::new(arg1.to_string(), None, vec![]).unwrap(),
            term::new(arg2.to_string(), None, vec![]).unwrap(),
        ];
        atom::new(p, terms).unwrap()
    }

    #[test]
    fn lowercase_initial_is_predicate_symbol() {
        assert!(predicate_symbol::new("likes".to_string(), 2).is_ok());
    }

    #[test]
    fn uppercase_initial_is_not_predicate_symbol() {
        assert!(predicate_symbol::new("F1".to_string(), 2).is_err());
    }

    #[test]
    fn atom_with_constants_is_ground() {
        let p = predicate_symbol::new("likes".to_string(), 2).unwrap();
        let terms = vec![
            term::new("a".to_string(), Some(0), vec![]).unwrap(),
            term::new("b".to_string(), Some(0), vec![]).unwrap(),
        ];
        assert!(atom::new(p, terms).unwrap().is_ground());
    }

    #[test]
    fn atom_with_variables_is_not_ground() {
        let p = predicate_symbol::new("likes".to_string(), 2).unwrap();
        let terms = vec![
            term::new("A".to_string(), None, vec![]).unwrap(),
            term::new("B".to_string(), None, vec![]).unwrap(),
        ];
        assert!(!atom::new(p, terms).unwrap().is_ground());
    }

    #[test]
    fn operation_with_constants_is_ground() {
        let vars = vec![
            term::new("a".to_string(), Some(0), vec![]).unwrap(),
            term::new("b".to_string(), Some(0), vec![]).unwrap(),
        ];
        let op = operation::new(operation_symbol::new("op".to_string(), 2).unwrap(), vars).unwrap();
        assert!(op.is_ground());
    }

    #[test]
    fn atom_with_correct_arity_is_ok() {
        let p = predicate_symbol::new("likes".to_string(), 2).unwrap();
        let terms = vec![
            term::new("A".to_string(), None, vec![]).unwrap(),
            term::new("b".to_string(), Some(0), vec![]).unwrap(),
        ];
        assert!(atom::new(p, terms).is_ok());
    }

    #[test]
    fn atom_with_wrong_arity_is_err() {
        let p = predicate_symbol::new("likes".to_string(), 2).unwrap();
        let terms = vec![term::new("A".to_string(), None, vec![]).unwrap()];
        assert!(atom::new(p, terms).is_err());
    }

    #[test]
    fn atom_is_positive_literal() {
        let p = predicate_symbol::new("lego_builder".to_string(), 1).unwrap();
        let terms = vec![term::new("alice".to_string(), Some(0), vec![]).unwrap()];
        let a = atom::new(p, terms).unwrap();
        let lit = literal::positive_literal(a);
        assert!(matches!(lit, literal::positive_literal(_)));
        assert_eq!(lit.to_string(), "lego_builder(alice)");
    }

    #[test]
    fn negative_literal_is_not_an_atom() {
        let p = predicate_symbol::new("lego_builder".to_string(), 1).unwrap();
        let terms = vec![term::new("alice".to_string(), Some(0), vec![]).unwrap()];
        let lit = literal::negative(p, terms).unwrap();
        assert!(matches!(lit, literal::negative_literal(_, _)));
        assert!(!matches!(lit, literal::positive_literal(_)));
        assert_eq!(lit.to_string(), "¬lego_builder(alice)");
    }

    #[test]
    fn rule_general_with_head_and_body() {
        let head = vec![literal::positive_literal(make_atom("reachable", "a"))];
        let body = vec![literal::positive_literal(make_atom("edge", "a"))];
        let r = rule::new(head, body).unwrap();
        assert!(matches!(r.rule_type, RuleType::General));
        assert_eq!(r.to_string(), "reachable(a) :- edge(a).");
    }

    #[test]
    fn unit_clause_is_ok() {
        let head = vec![literal::positive_literal(make_atom("loves", "alice"))];
        let r = rule::unit_clause(head).unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.to_string(), "loves(alice).");
    }

    #[test]
    fn unit_clause_empty_head_is_err() {
        assert!(rule::unit_clause(vec![]).is_err());
    }

    #[test]
    fn goal_with_body_is_ok() {
        let body = vec![
            literal::positive_literal(make_atom_2("head", "A", "B")),
            literal::positive_literal(make_atom_2("head", "B", "A")),
        ];
        let r = rule::goal(body).unwrap();
        assert!(matches!(r.rule_type, RuleType::Goal));
        assert_eq!(r.to_string(), ":- head(A, B), head(B, A).");
    }

    #[test]
    fn goal_empty_body_is_err() {
        assert!(rule::goal(vec![]).is_err());
    }

    #[test]
    fn definite_clause_single_positive_head_is_ok() {
        let head = vec![literal::positive_literal(make_atom_2("qsort", "A", "B"))];
        let body = vec![
            literal::positive_literal(make_atom("empty", "A")),
            literal::positive_literal(make_atom("empty", "B")),
        ];
        let r = rule::definite_clause(head, body).unwrap();
        assert!(matches!(r.rule_type, RuleType::DefiniteClause));
        assert_eq!(r.to_string(), "qsort(A, B) :- empty(A), empty(B).");
    }

    #[test]
    fn definite_clause_multiple_heads_is_err() {
        let head = vec![
            literal::positive_literal(make_atom("reachable", "a")),
            literal::positive_literal(make_atom("edge", "a")),
        ];
        let body = vec![literal::positive_literal(make_atom("node", "a"))];
        assert!(rule::definite_clause(head, body).is_err());
    }

    #[test]
    fn definite_clause_negative_head_is_err() {
        let head = vec![literal::negative(
            predicate_symbol::new("reachable".to_string(), 1).unwrap(),
            vec![term::new("a".to_string(), Some(0), vec![]).unwrap()],
        ).unwrap()];
        let body = vec![literal::positive_literal(make_atom("edge", "a"))];
        assert!(rule::definite_clause(head, body).is_err());
    }

    #[test]
    fn horn_rule_one_positive_is_ok() {
        let head = vec![literal::positive_literal(make_atom("reachable", "a"))];
        let body = vec![literal::negative(
            predicate_symbol::new("edge".to_string(), 1).unwrap(),
            vec![term::new("a".to_string(), Some(0), vec![]).unwrap()],
        ).unwrap()];
        let r = rule::horn(head, body).unwrap();
        assert!(matches!(r.rule_type, RuleType::HornRule));
    }

    #[test]
    fn horn_rule_two_positives_is_err() {
        let head = vec![literal::positive_literal(make_atom("reachable", "a"))];
        let body = vec![literal::positive_literal(make_atom("edge", "a"))];
        assert!(rule::horn(head, body).is_err());
    }

    #[test]
    fn fact_with_constants_is_ok() {
        let p = predicate_symbol::new("loves".to_string(), 2).unwrap();
        let terms = vec![
            term::new("andrew".to_string(), Some(0), vec![]).unwrap(),
            term::new("laura".to_string(), Some(0), vec![]).unwrap(),
        ];
        let head = vec![literal::positive_literal(atom::new(p, terms).unwrap())];
        let r = rule::fact(head).unwrap();
        assert!(matches!(r.rule_type, RuleType::Fact));
        assert_eq!(r.to_string(), "loves(andrew, laura).");
    }

    #[test]
    fn fact_with_variable_is_err() {
        let p = predicate_symbol::new("loves".to_string(), 2).unwrap();
        let terms = vec![
            term::new("andrew".to_string(), Some(0), vec![]).unwrap(),
            term::new("X".to_string(), None, vec![]).unwrap(),
        ];
        let head = vec![literal::positive_literal(atom::new(p, terms).unwrap())];
        assert!(rule::fact(head).is_err());
    }

    #[test]
    fn rule_with_only_constants_is_ground() {
        let head = vec![literal::positive_literal(make_atom("reachable", "a"))];
        let body = vec![literal::positive_literal(make_atom("edge", "a"))];
        assert!(rule::new(head, body).unwrap().is_ground());
    }

    #[test]
    fn rule_with_variable_is_not_ground() {
        let var_a = term::new("A".to_string(), None, vec![]).unwrap();
        let var_a2 = term::new("A".to_string(), None, vec![]).unwrap();
        let head = vec![literal::positive_literal(
            atom::new(predicate_symbol::new("reachable".to_string(), 1).unwrap(), vec![var_a]).unwrap()
        )];
        let body = vec![literal::positive_literal(
            atom::new(predicate_symbol::new("edge".to_string(), 1).unwrap(), vec![var_a2]).unwrap()
        )];
        assert!(!rule::new(head, body).unwrap().is_ground());
    }

    #[test]
    fn unifies_matching_substitution_returns_true() {
        // likes(X, Y).unifies([alice, bob], likes(A, B)) with subs [alice, bob]
        // => likes(alice, bob) == likes(alice, bob) => true
        let self_atom = make_atom_2("likes", "X", "Y");
        let other    = make_atom_2("likes", "A", "B");
        let subs = vec![
            term::new("alice".to_string(), Some(0), vec![]).unwrap(),
            term::new("bob".to_string(),   Some(0), vec![]).unwrap(),
        ];
        assert!(self_atom.unifies(subs, &other).unwrap());
    }

    #[test]
    fn unifies_non_matching_substitution_returns_false() {
        // likes(X, Y) with [alice, bob], hates(A, B) with [alice, bob]
        // => likes(alice, bob) != hates(alice, bob) => false
        let self_atom = make_atom_2("likes", "X", "Y");
        let other    = make_atom_2("hates", "A", "B");
        let subs = vec![
            term::new("alice".to_string(), Some(0), vec![]).unwrap(),
            term::new("bob".to_string(),   Some(0), vec![]).unwrap(),
        ];
        assert!(!self_atom.unifies(subs, &other).unwrap());
    }

    #[test]
    fn unifies_shared_variable_names_is_err() {
        // likes(X, Y) and likes(X, Z) share variable X
        let self_atom = make_atom_2("likes", "X", "Y");
        let other = make_atom_2("likes", "X", "Z");
        let subs = vec![
            term::new("alice".to_string(), Some(0), vec![]).unwrap(),
            term::new("bob".to_string(),   Some(0), vec![]).unwrap(),
        ];
        assert!(self_atom.unifies(subs, &other).is_err());
    }

    #[test]
    fn unifies_wrong_subs_length_is_err() {
        let self_atom = make_atom_2("likes", "X", "Y");
        let other    = make_atom_2("likes", "A", "B");
        let subs = vec![term::new("alice".to_string(), Some(0), vec![]).unwrap()];
        assert!(self_atom.unifies(subs, &other).is_err());
    }

    #[test]
    fn rule_to_json() {
        let r = crate::parse::parse_rule("happy(A) :- lego_builder(A), enjoys_lego(A)").unwrap();
        let json = r.to_json();
        assert_eq!(json, r#"{"rule_type":"General","head":[{"polarity":"positive","atom":{"predicate":"happy","terms":[{"type":"variable","name":"A"}]}}],"body":[{"polarity":"positive","atom":{"predicate":"lego_builder","terms":[{"type":"variable","name":"A"}]}},{"polarity":"positive","atom":{"predicate":"enjoys_lego","terms":[{"type":"variable","name":"A"}]}}],"formula":{"value":null,"type":"combination","connective":"=>","operands":[{"value":null,"type":"combination","connective":"∧","operands":[{"value":null,"type":"relation","symbol":"lego_builder","terms":[{"type":"variable","name":"A"}]},{"value":null,"type":"relation","symbol":"enjoys_lego","terms":[{"type":"variable","name":"A"}]}]},{"value":null,"type":"relation","symbol":"happy","terms":[{"type":"variable","name":"A"}]}]}}"#);
    }

    #[test]
    fn substitution_replaces_variables() {
        // likes(X, Y) :- knows(X, Y)  →  likes(alice, bob) :- knows(alice, bob)
        // variables in appearance order: X, Y
        let head = vec![literal::positive_literal(make_atom_2("likes", "X", "Y"))];
        let body = vec![literal::positive_literal(make_atom_2("knows", "X", "Y"))];
        let r = rule::new(head, body).unwrap();

        let subs = vec![
            term::new("alice".to_string(), Some(0), vec![]).unwrap(),
            term::new("bob".to_string(), Some(0), vec![]).unwrap(),
        ];

        let r2 = r.substitution(subs).unwrap();
        assert_eq!(r2.to_string(), "likes(alice, bob) :- knows(alice, bob).");
        assert!(r2.is_ground());
    }

    #[test]
    fn substitution_wrong_length_is_err() {
        let head = vec![literal::positive_literal(make_atom_2("likes", "X", "Y"))];
        let r = rule::new(head, vec![]).unwrap();
        let subs = vec![term::new("alice".to_string(), Some(0), vec![]).unwrap()];
        assert!(r.substitution(subs).is_err());
    }

    #[test]
    fn operation_with_variables_is_not_ground() {
        let vars = vec![
            term::new("A".to_string(), None, vec![]).unwrap(),
            term::new("B".to_string(), None, vec![]).unwrap(),
        ];
        let op = operation::new(operation_symbol::new("op".to_string(), 2).unwrap(), vars).unwrap();
        assert!(!op.is_ground());
    }
}
