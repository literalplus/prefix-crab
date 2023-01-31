use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable debug mode, repeat to ✨ intensify ✨
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Performs a greeting
    Greet {
        /// Who to greet
        name: String,
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.debug {
        0 => println!("Zero debug for you"),
        1 => println!("Lowkey debugging"),
        2 => println!("No bugs left"),
        _ => println!("Bug count negative, police have been informed"),
    }

    match cli.command {
        Commands::Greet { name } => {
            println!("Henlo {}", name.as_str());
            if cli.debug > 1 {
                println!("HELLO {}!!", name.as_str());
            }
        }
    }
}
