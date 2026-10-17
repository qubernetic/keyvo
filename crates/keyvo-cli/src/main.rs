//! keyvo command-line interface.

use clap::Parser;

/// keyvo: context-aware control surface for the Logitech MX Creative Console.
#[derive(Parser)]
#[command(name = "keyvo", version = keyvo_core::VERSION)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
