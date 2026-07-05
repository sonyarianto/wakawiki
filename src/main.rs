use clap::Parser;

mod agent;
mod config;
mod output;
mod prompts;
mod provider;
mod scanner;

#[derive(Parser)]
#[command(
    name = "wakawiki",
    about = "A CLI that writes and maintains agent documentation for your codebase"
)]
struct Cli {
    /// Initialize wakawiki: configure provider, API key, and model
    #[arg(long)]
    init: bool,

    /// Update existing documentation
    #[arg(long)]
    update: bool,

    /// Non-interactive mode: run a one-shot prompt and print the result
    #[arg(short = 'p', long = "print")]
    print_mode: bool,

    /// Initial prompt to start with (otherwise enters interactive mode)
    prompt: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.init {
        if let Err(e) = config::init_config() {
            eprintln!("Error during initialization: {e}");
            std::process::exit(1);
        }
        println!("Configuration saved. Run 'wakawiki' to generate documentation.");
        return;
    }

    let cfg = config::load_config().unwrap_or_else(|e| {
        eprintln!("Error loading config: {e}");
        eprintln!("Run 'wakawiki --init' first to configure.");
        std::process::exit(1);
    });

    let project_dir = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("Error getting current directory: {e}");
        std::process::exit(1);
    });
    let wakawiki_dir = project_dir.join("wakawiki");

    if cli.update && wakawiki_dir.exists() {
        let mut wiki_meta = output::load_wiki_meta(&wakawiki_dir);
        let provider = provider::create(&cfg);
        let result =
            agent::update_docs(&project_dir, &wakawiki_dir, &mut wiki_meta, &provider, &cfg).await;
        match result {
            Ok(()) => println!("Documentation updated."),
            Err(e) => eprintln!("Update failed: {e}"),
        }
        return;
    }

    let init_prompt = cli.prompt.unwrap_or_else(|| {
        "Please generate comprehensive documentation for this codebase. Start by exploring the directory structure and key files, then create documentation covering architecture, modules, and APIs.".into()
    });

    let provider = provider::create(&cfg);

    if cli.print_mode {
        match agent::run_oneshot(&project_dir, &provider, &cfg, &init_prompt).await {
            Ok(output) => println!("{output}"),
            Err(e) => eprintln!("Error: {e}"),
        }
    } else {
        match agent::run_interactive(&project_dir, &provider, &cfg, Some(&init_prompt)).await {
            Ok(()) => {}
            Err(e) => eprintln!("Error: {e}"),
        }
    }
}
