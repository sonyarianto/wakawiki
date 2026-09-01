use clap::Parser;

mod agent;
mod config;
mod index;
mod mcp;
mod output;
mod prompts;
mod provider;
mod scan;
mod scanner;
mod vector;

#[derive(Parser)]
#[command(
    name = "wakawiki",
    version = env!("CARGO_PKG_VERSION"),
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

    /// Scan-only mode: generate documentation using heuristics (no LLM)
    #[arg(long)]
    scan: bool,

    /// Build structured JSON index of all symbols and files
    #[arg(long)]
    index: bool,

    /// Generate embeddings for semantic search
    #[arg(long)]
    embed: bool,

    /// Query the index for symbols or files matching a pattern
    #[arg(short = 'q', long = "query")]
    query: Option<String>,

    /// Semantic search query (requires embeddings)
    #[arg(short = 's', long = "semantic")]
    semantic: Option<String>,

    /// Start MCP server (use with --mcp for MCP protocol over stdio)
    #[arg(long)]
    serve: bool,

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

    let project_dir = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("Error getting current directory: {e}");
        std::process::exit(1);
    });

    if cli.scan {
        if let Err(e) = scan::run(&project_dir) {
            eprintln!("Scan failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    if cli.index {
        match index::build_index(&project_dir) {
            Ok(code_index) => match index::save_index(&project_dir, &code_index) {
                Ok(path) => {
                    println!("Index saved to {path}");
                    println!("  Files: {}", code_index.files.len());
                    println!("  Symbols: {}", code_index.symbols.len());
                }
                Err(e) => {
                    eprintln!("Error saving index: {e}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("Index build failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if cli.embed {
        let code_index = match index::load_index(&project_dir) {
            Ok(idx) => idx,
            Err(_) => {
                eprintln!("No index found. Run 'wakawiki --index' first.");
                std::process::exit(1);
            }
        };

        let mut store = vector::VectorStore::new();
        store.build_from_index(&code_index);

        match store.save(&project_dir) {
            Ok(path) => {
                println!("Embeddings saved to {path}");
                println!("  Vectors: {}", store.embeddings.len());
            }
            Err(e) => {
                eprintln!("Error saving embeddings: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(pattern) = cli.query {
        let code_index = match index::load_index(&project_dir) {
            Ok(idx) => idx,
            Err(_) => {
                eprintln!("No index found. Run 'wakawiki --index' first.");
                std::process::exit(1);
            }
        };
        let result = index::query_index(&code_index, &pattern);
        print!("{}", index::format_query_result(&result, &pattern));
        return;
    }

    if let Some(query) = cli.semantic {
        let store = match vector::VectorStore::load(&project_dir) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("No embeddings found. Run 'wakawiki --embed' first.");
                std::process::exit(1);
            }
        };

        let results = store.search(&query, 10);
        print!("{}", vector::format_semantic_results(&results));
        return;
    }

    if cli.serve {
        if let Err(e) = mcp::run_server(&project_dir) {
            eprintln!("MCP server error: {e}");
            std::process::exit(1);
        }
        return;
    }

    let cfg = config::load_config().unwrap_or_else(|e| {
        eprintln!("Error loading config: {e}");
        eprintln!("Run 'wakawiki --init' first to configure.");
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
