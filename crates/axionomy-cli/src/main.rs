use clap::Parser;

fn main() {
    let cli = axionomy_cli::Cli::parse();
    match axionomy_cli::run(axionomy_problems::registry(), cli) {
        Ok(Some(text)) => println!("{text}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}
