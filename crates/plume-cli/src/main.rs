//! Outil de test sans interface. Sous-commandes ajoutées par module.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        _ => {
            eprintln!("usage : plume-cli <commande> …");
            std::process::exit(2);
        }
    }
}
