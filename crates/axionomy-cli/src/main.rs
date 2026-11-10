use axionomy_cli::{execute, write_output};
use clap::Parser;

fn main() {
    let cli = axionomy_cli::Cli::parse();
    match execute(&axionomy_problems::registry(), cli).and_then(write_output) {
        Ok(Some(text)) => println!("{text}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}
