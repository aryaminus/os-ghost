//! OS-Ghost Headless Server
//!
//! Runs OS-Ghost as a headless HTTP server with REST API and WebSocket support.
//! This enables programmatic access to OS-Ghost capabilities without the GUI.

use std::process;

use clap::Parser;
use tracing::{error, info};

use os_ghost_lib::server::auth::generate_api_key;
use os_ghost_lib::server::{GhostServer, ServerConfig};

const DEFAULT_PORT: u16 = 7842;

#[derive(Parser)]
#[command(name = "os-ghost-server")]
#[command(about = "OS-Ghost Headless Server")]
#[command(version)]
struct Args {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// API key for authentication
    #[arg(short, long, env = "OSGHOST_API_KEY")]
    api_key: Option<String>,

    /// Generate a new API key and exit
    #[arg(long)]
    generate_key: bool,

    /// Run in headless mode (no GUI)
    #[arg(long)]
    headless: bool,

    /// Disable WebSocket support
    #[arg(long)]
    no_websocket: bool,

    /// Disable CORS
    #[arg(long)]
    no_cors: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    let filter = if args.verbose {
        "os_ghost_lib=debug,server=debug,axum=debug"
    } else {
        "os_ghost_lib=info,server=info"
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Generate API key if requested
    if args.generate_key {
        let key = generate_api_key();
        println!("Generated API key: {}", key);
        println!(
            "Set this as environment variable: export OSGHOST_API_KEY={}",
            key
        );
        process::exit(0);
    }

    // Print startup banner
    print_banner();

    // Build server config
    let config = ServerConfig {
        host: args.host,
        port: args.port,
        api_key: args.api_key,
        enable_websocket: !args.no_websocket,
        enable_cors: !args.no_cors,
        headless: args.headless,
        data_dir: dirs::data_dir().unwrap_or_default().join("os-ghost"),
        log_level: if args.verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        },
        max_request_size: 10 * 1024 * 1024, // 10MB
        request_timeout_secs: 60,
    };

    info!("Starting OS-Ghost server with config: {:?}", config);

    // Create and start server
    let server = GhostServer::new(config, None);

    if let Err(e) = server.start().await {
        error!("Server failed: {}", e);
        process::exit(1);
    }
}

fn print_banner() {
    println!(
        r#"
   ____  _____       ____  _   _            _   
  / __ \|  __ \     / __ \| | | |          | |  
 | |  | | |__) |___| |  | | |_| | __ _  ___| |_ 
 | |  | |  _  // _ \ |  | |  _  |/ _` |/ _ \ __|
 | |__| | | \ \  __/ |__| | | | | (_| |  __/ |_ 
  \____/|_|  \_\___|\____/|_| |_|\__, |\___|\__|
                                  __/ |         
                                 |___/          
    "#
    );

    println!("  The OS-Ghost Headless Server");
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));
    println!();
}
