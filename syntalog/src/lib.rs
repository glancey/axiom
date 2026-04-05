use formalisms::{individual_variable, operation_symbol, operation, term, TermType};
use std::collections::HashMap;
use std::fmt;
use anyhow::Result;

/// An `operation_symbol` whose name begins with a lowercase ASCII letter.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}

impl fmt::Display for atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.predicate.0.symbol)?;
        for (i, t) in self.terms.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", term_name(t))?;
        }
        write!(f, ")")
    }
}

fn term_name(t: &term) -> &str {
    match &t.term_type {
        TermType::Variable(v) => &v.name,
        TermType::Constant(c) => &c.0.symbol,
        TermType::Operation(op) => &op.symbol.symbol,
    }
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
                for (i, t) in terms.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", term_name(t))?;
                }
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
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct rule {
    pub head: Vec<literal>,
    pub body: Vec<literal>,
    pub rule_type: RuleType,
}

impl rule {
    /// General clause — no structural restrictions.
    pub fn new(head: Vec<literal>, body: Vec<literal>) -> Self {
        rule { head, body, rule_type: RuleType::General }
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
        Ok(())
    }
}

impl is_ground for rule {
    fn is_ground(&self) -> bool {
        self.head.iter().chain(self.body.iter()).all(|lit| lit.is_ground())
    }
}

fn substitute_term(t: term, subs: &HashMap<individual_variable, term>) -> term {
    match t.term_type {
        TermType::Variable(ref v) => {
            match subs.get(v) {
                Some(replacement) => replacement.clone(),
                None => t,
            }
        }
        TermType::Constant(_) => t,
        TermType::Operation(op) => {
            let new_vars = op.vars.into_iter().map(|v| substitute_term(v, subs)).collect();
            term { term_type: TermType::Operation(operation { symbol: op.symbol, vars: new_vars }) }
        }
    }
}

fn substitute_atom(a: atom, subs: &HashMap<individual_variable, term>) -> atom {
    atom {
        predicate: a.predicate,
        terms: a.terms.into_iter().map(|t| substitute_term(t, subs)).collect(),
    }
}

fn substitute_literal(lit: literal, subs: &HashMap<individual_variable, term>) -> literal {
    match lit {
        literal::positive_literal(a) => literal::positive_literal(substitute_atom(a, subs)),
        literal::negative_literal(p, terms) => literal::negative_literal(
            p,
            terms.into_iter().map(|t| substitute_term(t, subs)).collect(),
        ),
    }
}

impl rule {
    /// Returns a new rule with each `individual_variable` replaced by its
    /// corresponding term in `subs`, keyed by variable name.
    /// Variables not present in `subs` are left unchanged.
    pub fn substitution(self, subs: HashMap<individual_variable, term>) -> Self {
        let head = self.head.into_iter().map(|lit| substitute_literal(lit, &subs)).collect();
        let body = self.body.into_iter().map(|lit| substitute_literal(lit, &subs)).collect();
        rule { head, body, rule_type: self.rule_type }
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
    use std::collections::HashMap;
    use formalisms::individual_variable;

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
        let r = rule::new(head, body);
        assert!(matches!(r.rule_type, RuleType::General));
        assert_eq!(r.to_string(), "reachable(a) :- edge(a)");
    }

    #[test]
    fn unit_clause_is_ok() {
        let head = vec![literal::positive_literal(make_atom("loves", "alice"))];
        let r = rule::unit_clause(head).unwrap();
        assert!(matches!(r.rule_type, RuleType::UnitClause));
        assert_eq!(r.to_string(), "loves(alice)");
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
        assert_eq!(r.to_string(), ":- head(A, B), head(B, A)");
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
        assert_eq!(r.to_string(), "qsort(A, B) :- empty(A), empty(B)");
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
        assert_eq!(r.to_string(), "loves(andrew, laura)");
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
        assert!(rule::new(head, body).is_ground());
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
        assert!(!rule::new(head, body).is_ground());
    }

    #[test]
    fn substitution_replaces_variables() {
        // likes(X, Y) :- knows(X, Y)  →  likes(alice, bob) :- knows(alice, bob)
        let head = vec![literal::positive_literal(make_atom_2("likes", "X", "Y"))];
        let body = vec![literal::positive_literal(make_atom_2("knows", "X", "Y"))];
        let r = rule::new(head, body);

        let mut subs = HashMap::new();
        subs.insert(individual_variable::new("X").unwrap(), term::new("alice".to_string(), Some(0), vec![]).unwrap());
        subs.insert(individual_variable::new("Y").unwrap(), term::new("bob".to_string(), Some(0), vec![]).unwrap());

        let r2 = r.substitution(subs);
        assert_eq!(r2.to_string(), "likes(alice, bob) :- knows(alice, bob)");
        assert!(r2.is_ground());
    }

    #[test]
    fn substitution_leaves_unbound_variables() {
        // likes(X, Y) with only X bound → likes(alice, Y)
        let head = vec![literal::positive_literal(make_atom_2("likes", "X", "Y"))];
        let r = rule::unit_clause(head).unwrap();

        let mut subs = HashMap::new();
        subs.insert(individual_variable::new("X").unwrap(), term::new("alice".to_string(), Some(0), vec![]).unwrap());

        let r2 = r.substitution(subs);
        assert_eq!(r2.to_string(), "likes(alice, Y)");
        assert!(!r2.is_ground());
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
