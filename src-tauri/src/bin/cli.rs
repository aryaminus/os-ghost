//! OS-Ghost CLI
//!
//! Command-line interface for interacting with the OS-Ghost server
//! or running tasks directly. Provides similar capabilities to UI-TARS CLI.

use std::process;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:7842";

#[derive(Parser)]
#[command(name = "os-ghost-cli")]
#[command(about = "OS-Ghost Command Line Interface")]
#[command(version)]
struct Cli {
    /// Server URL
    #[arg(short, long, default_value = DEFAULT_SERVER_URL)]
    server: String,

    /// API key for authentication
    #[arg(short, long, env = "OSGHOST_API_KEY")]
    api_key: Option<String>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "text")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Debug)]
enum OutputFormat {
    Text,
    Json,
    Table,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a task
    Execute {
        /// Task description
        task: String,

        /// Autonomy level
        #[arg(short, long, value_enum, default_value = "supervised")]
        autonomy: AutonomyLevel,

        /// Wait for completion
        #[arg(short, long)]
        wait: bool,
    },

    /// Check server status
    Status,

    /// List workflows
    Workflows,

    /// Execute a workflow
    RunWorkflow {
        /// Workflow ID or name
        workflow: String,
    },

    /// Start recording a workflow
    Record {
        /// Workflow name
        name: String,

        /// Workflow description
        #[arg(short, long)]
        description: Option<String>,

        /// Starting URL
        #[arg(short, long)]
        url: String,
    },

    /// Stop recording
    StopRecord,

    /// List active agents
    Agents,

    /// Show pending actions
    Pending,

    /// Approve a pending action
    Approve {
        /// Action ID
        id: String,
    },

    /// Deny a pending action
    Deny {
        /// Action ID
        id: String,
    },

    /// Show memory statistics
    Memory,

    /// Interactive mode
    Interactive,

    /// Watch events
    Watch,
}

#[derive(ValueEnum, Clone, Debug, Serialize, Deserialize)]
enum AutonomyLevel {
    Observer,
    Suggester,
    Supervised,
    Autonomous,
}

impl From<AutonomyLevel> for String {
    fn from(level: AutonomyLevel) -> Self {
        match level {
            AutonomyLevel::Observer => "observer".to_string(),
            AutonomyLevel::Suggester => "suggester".to_string(),
            AutonomyLevel::Supervised => "supervised".to_string(),
            AutonomyLevel::Autonomous => "autonomous".to_string(),
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("os_ghost_cli=debug")
            .init();
    }

    match run_command(cli).await {
        Ok(_) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

async fn run_command(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let base_url = cli.server.trim_end_matches('/');

    match cli.command {
        Commands::Status => {
            let resp = client
                .get(format!("{}/api/v1/status", base_url))
                .send()
                .await?;

            let status: serde_json::Value = resp.json().await?;
            print_status(&status, cli.format)?;
        }

        Commands::Execute {
            task,
            autonomy,
            wait,
        } => {
            let req = serde_json::json!({
                "task": task,
                "autonomy_level": String::from(autonomy),
                "wait": wait,
            });

            let resp = client
                .post(format!("{}/api/v1/execute", base_url))
                .json(&req)
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;

            if wait {
                println!("Task queued. Use 'os-ghost-cli watch' to monitor progress.");
            }
        }

        Commands::Workflows => {
            let resp = client
                .get(format!("{}/api/v1/workflows", base_url))
                .send()
                .await?;

            let workflows: serde_json::Value = resp.json().await?;
            print_workflows(&workflows, cli.format)?;
        }

        Commands::RunWorkflow { workflow } => {
            let resp = client
                .post(format!(
                    "{}/api/v1/workflows/{}/execute",
                    base_url, workflow
                ))
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;
        }

        Commands::Record {
            name,
            description,
            url,
        } => {
            let req = serde_json::json!({
                "name": name,
                "description": description.unwrap_or_default(),
                "start_url": url,
            });

            let resp = client
                .post(format!("{}/api/v1/record/start", base_url))
                .json(&req)
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;
        }

        Commands::StopRecord => {
            let resp = client
                .post(format!("{}/api/v1/record/stop", base_url))
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;
        }

        Commands::Agents => {
            let resp = client
                .get(format!("{}/api/v1/agents", base_url))
                .send()
                .await?;

            let agents: serde_json::Value = resp.json().await?;
            print_agents(&agents, cli.format)?;
        }

        Commands::Pending => {
            let resp = client
                .get(format!("{}/api/v1/pending-actions", base_url))
                .send()
                .await?;

            let actions: serde_json::Value = resp.json().await?;
            print_pending_actions(&actions, cli.format)?;
        }

        Commands::Approve { id } => {
            let resp = client
                .post(format!("{}/api/v1/actions/{}/approve", base_url, id))
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;
        }

        Commands::Deny { id } => {
            let resp = client
                .post(format!("{}/api/v1/actions/{}/deny", base_url, id))
                .send()
                .await?;

            let result: serde_json::Value = resp.json().await?;
            print_result(&result, cli.format)?;
        }

        Commands::Memory => {
            let resp = client
                .get(format!("{}/api/v1/memory", base_url))
                .send()
                .await?;

            let memory: serde_json::Value = resp.json().await?;
            print_memory(&memory, cli.format)?;
        }

        Commands::Interactive => {
            run_interactive_mode(&client, base_url).await?;
        }

        Commands::Watch => {
            watch_events(base_url).await?;
        }
    }

    Ok(())
}

fn print_status(
    status: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(status)?);
        }
        OutputFormat::Text => {
            if let Some(data) = status.get("data") {
                println!("OS-Ghost Server Status");
                println!("======================");
                println!(
                    "Connected: {}",
                    data["connected"].as_bool().unwrap_or(false)
                );
                println!(
                    "Uptime: {} seconds",
                    data["uptime_secs"].as_i64().unwrap_or(0)
                );
                println!(
                    "Active Agents: {}",
                    data["active_agents_count"].as_u64().unwrap_or(0)
                );
                println!(
                    "Pending Actions: {}",
                    data["pending_actions_count"].as_u64().unwrap_or(0)
                );
                println!(
                    "Workflows: {}",
                    data["workflows_count"].as_u64().unwrap_or(0)
                );
                println!(
                    "Memory Entries: {}",
                    data["memory_entries"].as_u64().unwrap_or(0)
                );
            }
        }
        OutputFormat::Table => {
            // Simple table format
            println!("{:<20} Value", "Property");
            println!("{}", "-".repeat(40));
            if let Some(data) = status.get("data") {
                println!(
                    "{:<20} {}",
                    "Connected:",
                    data["connected"].as_bool().unwrap_or(false)
                );
                println!(
                    "{:<20} {}",
                    "Uptime (s):",
                    data["uptime_secs"].as_i64().unwrap_or(0)
                );
                println!(
                    "{:<20} {}",
                    "Active Agents:",
                    data["active_agents_count"].as_u64().unwrap_or(0)
                );
            }
        }
    }
    Ok(())
}

fn print_result(
    result: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(result)?);
        }
        OutputFormat::Text | OutputFormat::Table => {
            if let Some(data) = result.get("data") {
                println!("Success: {}", result["success"].as_bool().unwrap_or(false));
                if let Some(msg) = data.get("message") {
                    println!("{}", msg.as_str().unwrap_or(""));
                }
            } else if let Some(error) = result.get("error") {
                eprintln!("Error: {}", error.as_str().unwrap_or("Unknown error"));
            }
        }
    }
    Ok(())
}

fn print_workflows(
    workflows: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(workflows)?);
        }
        _ => {
            if let Some(data) = workflows.get("data").and_then(|d| d.as_array()) {
                println!("{:<30} {:<10} {:<10}", "Name", "Steps", "Success Rate");
                println!("{}", "-".repeat(60));
                for workflow in data {
                    let name = workflow["name"].as_str().unwrap_or("Unknown");
                    let steps = workflow["step_count"].as_u64().unwrap_or(0);
                    let rate = workflow["success_rate"].as_f64().unwrap_or(0.0);
                    println!("{:<30} {:<10} {:.1}%", name, steps, rate * 100.0);
                }
            }
        }
    }
    Ok(())
}

fn print_agents(
    agents: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(agents)?);
        }
        _ => {
            if let Some(data) = agents.get("data").and_then(|d| d.as_array()) {
                println!("{:<30} {:<15} {:<15}", "Name", "Type", "Status");
                println!("{}", "-".repeat(70));
                for agent in data {
                    let name = agent["name"].as_str().unwrap_or("Unknown");
                    let agent_type = agent["type"].as_str().unwrap_or("unknown");
                    let status = agent["status"].as_str().unwrap_or("unknown");
                    println!("{:<30} {:<15} {:<15}", name, agent_type, status);
                }
            }
        }
    }
    Ok(())
}

fn print_pending_actions(
    actions: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(actions)?);
        }
        _ => {
            if let Some(data) = actions.get("data").and_then(|d| d.as_array()) {
                if data.is_empty() {
                    println!("No pending actions");
                } else {
                    println!("{:<36} {:<15} {:<30}", "ID", "Type", "Description");
                    println!("{}", "-".repeat(90));
                    for action in data {
                        let id = action["id"].as_str().unwrap_or("unknown");
                        let action_type = action["type"].as_str().unwrap_or("unknown");
                        let desc = action["description"].as_str().unwrap_or("");
                        println!("{:<36} {:<15} {}", id, action_type, desc);
                    }
                }
            }
        }
    }
    Ok(())
}

fn print_memory(
    memory: &serde_json::Value,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(memory)?);
        }
        _ => {
            if let Some(data) = memory.get("data") {
                println!("Memory Statistics");
                println!("=================");
                println!(
                    "Total Entries: {}",
                    data["total_entries"].as_u64().unwrap_or(0)
                );
            }
        }
    }
    Ok(())
}

async fn run_interactive_mode(
    _client: &reqwest::Client,
    _base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("OS-Ghost Interactive Mode");
    println!("Type 'help' for available commands, 'exit' to quit.");
    println!();

    // Simple interactive loop
    loop {
        print!("ghost> ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let trimmed = input.trim();

        match trimmed {
            "exit" | "quit" => {
                println!("Goodbye!");
                break;
            }
            "help" => {
                println!("Available commands:");
                println!("  status     - Show server status");
                println!("  agents     - List active agents");
                println!("  workflows  - List workflows");
                println!("  pending    - Show pending actions");
                println!("  execute <task> - Execute a task");
                println!("  exit       - Quit interactive mode");
            }
            _ => {
                println!("Unknown command: {}", trimmed);
            }
        }
    }

    Ok(())
}

async fn watch_events(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let ws_url = base_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let url = format!("{}/ws", ws_url);

    println!("Connecting to WebSocket: {}", url);

    let (ws_stream, _) = connect_async(&url).await?;
    println!("Connected! Watching events... (Press Ctrl+C to exit)");

    let (mut write, mut read) = ws_stream.split();

    // Send ping every 30 seconds
    let ping_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            if write.send(Message::Ping(vec![])).await.is_err() {
                break;
            }
        }
    });

    // Read messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                    println!(
                        "[{}] {}",
                        event["type"].as_str().unwrap_or("unknown"),
                        serde_json::to_string_pretty(&event["data"]).unwrap_or_default()
                    );
                }
            }
            Ok(Message::Close(_)) => {
                println!("Connection closed");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    ping_task.abort();
    Ok(())
}
