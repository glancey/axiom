use axiom::helpers::normalize_formula;

fn main() {
    let input = "not(P => Q)";
    println!("normalized: {}", normalize_formula(input));
}
