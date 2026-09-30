#[path = "../js_repl.rs"]
mod js_repl;

/// Run the dedicated stdio transport without initializing the Michelle GUI.
fn main() {
    if let Err(error) = js_repl::serve_stdio() {
        eprintln!("Michelle JavaScript REPL: {error:#}");
        std::process::exit(1);
    }
}
