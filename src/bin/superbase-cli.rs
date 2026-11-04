use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Row, Table};
use reqwest::Client;
use serde_json::{json, Value};
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(
    name = "superbase-cli",
    about = "⚡ Superbase CLI - Interactive management tool for Superbase"
)]
struct Cli {
    /// URL of the Superbase server
    #[arg(short, long, default_value = "http://127.0.0.1:8080")]
    url: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show server health and database statistics
    Stats,

    /// List all collections
    List,

    /// Create a new collection
    Create {
        collection: String,
    },

    /// Drop an existing collection
    Drop {
        collection: String,
    },

    /// Insert a document (JSON string)
    Insert {
        collection: String,
        #[arg(short, long)]
        data: String,
    },

    /// Get a document by ID
    Get {
        collection: String,
        id: String,
    },

    /// Update a document by ID
    Update {
        collection: String,
        id: String,
        #[arg(short, long)]
        data: String,
    },

    /// Delete a document by ID
    Delete {
        collection: String,
        id: String,
    },

    /// Query documents with optional filter and sort JSON
    Query {
        collection: String,
        #[arg(short, long)]
        filter: Option<String>,
        #[arg(short, long)]
        sort: Option<String>,
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Create secondary index on a JSON field/path
    Index {
        collection: String,
        path: String,
    },

    /// Compact a collection to reclaim deleted space
    Compact {
        collection: String,
    },

    /// Launch interactive REPL mode
    Repl,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Some(Commands::Stats) => print_stats(&client, &cli.url).await?,
        Some(Commands::List) => list_collections(&client, &cli.url).await?,
        Some(Commands::Create { collection }) => create_col(&client, &cli.url, &collection).await?,
        Some(Commands::Drop { collection }) => drop_col(&client, &cli.url, &collection).await?,
        Some(Commands::Insert { collection, data }) => insert_doc(&client, &cli.url, &collection, &data).await?,
        Some(Commands::Get { collection, id }) => get_doc(&client, &cli.url, &collection, &id).await?,
        Some(Commands::Update { collection, id, data }) => update_doc(&client, &cli.url, &collection, &id, &data).await?,
        Some(Commands::Delete { collection, id }) => delete_doc(&client, &cli.url, &collection, &id).await?,
        Some(Commands::Query { collection, filter, sort, limit }) => {
            query_docs(&client, &cli.url, &collection, filter, sort, limit).await?
        }
        Some(Commands::Index { collection, path }) => create_idx(&client, &cli.url, &collection, &path).await?,
        Some(Commands::Compact { collection }) => compact_col(&client, &cli.url, &collection).await?,
        Some(Commands::Repl) | None => run_repl(&client, &cli.url).await?,
    }

    Ok(())
}

async fn print_stats(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.get(format!("{}/api/v1/stats", base_url)).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn list_collections(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.get(format!("{}/api/v1/collections", base_url)).send().await?.json().await?;
    if let Some(list) = resp.get("data").and_then(|v| v.as_array()) {
        println!("{}", "📁 Collections in Superbase:".bold().cyan());
        for item in list {
            println!("  • {}", item.as_str().unwrap_or(""));
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    }
    Ok(())
}

async fn create_col(client: &Client, base_url: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({ "name": name });
    let resp: Value = client.post(format!("{}/api/v1/collections", base_url)).json(&payload).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn drop_col(client: &Client, base_url: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.delete(format!("{}/api/v1/collections/{}", base_url, name)).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn insert_doc(client: &Client, base_url: &str, col: &str, data: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parsed: Value = serde_json::from_str(data)?;
    let resp: Value = client.post(format!("{}/api/v1/collections/{}/documents", base_url, col)).json(&parsed).send().await?.json().await?;
    println!("{}", "✅ Document Inserted:".green().bold());
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn get_doc(client: &Client, base_url: &str, col: &str, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.get(format!("{}/api/v1/collections/{}/documents/{}", base_url, col, id)).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn update_doc(client: &Client, base_url: &str, col: &str, id: &str, data: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parsed: Value = serde_json::from_str(data)?;
    let resp: Value = client.put(format!("{}/api/v1/collections/{}/documents/{}", base_url, col, id)).json(&parsed).send().await?.json().await?;
    println!("{}", "✅ Document Updated:".green().bold());
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn delete_doc(client: &Client, base_url: &str, col: &str, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.delete(format!("{}/api/v1/collections/{}/documents/{}", base_url, col, id)).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn query_docs(
    client: &Client,
    base_url: &str,
    col: &str,
    filter: Option<String>,
    sort: Option<String>,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = json!({});
    if let Some(f) = filter {
        payload["filter"] = serde_json::from_str(&f)?;
    }
    if let Some(s) = sort {
        payload["sort"] = serde_json::from_str(&s)?;
    }
    if let Some(l) = limit {
        payload["limit"] = json!(l);
    }

    let resp: Value = client
        .post(format!("{}/api/v1/collections/{}/query", base_url, col))
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    if let Some(data) = resp.get("data") {
        if let Some(docs) = data.get("documents").and_then(|v| v.as_array()) {
            println!("{}", format!("🔍 Query Results ({} matching):", docs.len()).bold().cyan());
            if docs.is_empty() {
                println!("  (No documents found)");
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(UTF8_FULL).apply_modifier(UTF8_ROUND_CORNERS);
            table.set_header(vec![
                Cell::new("_id").fg(Color::Yellow),
                Cell::new("_version").fg(Color::DarkGrey),
                Cell::new("Document Content").fg(Color::Green),
            ]);

            for doc in docs {
                let id = doc.get("_id").and_then(|v| v.as_str()).unwrap_or("-");
                let ver = doc.get("_version").map(|v| v.to_string()).unwrap_or_else(|| "-".into());
                table.add_row(Row::from(vec![
                    Cell::new(id),
                    Cell::new(ver),
                    Cell::new(serde_json::to_string(doc)?),
                ]));
            }
            println!("{table}");
            return Ok(());
        }
    }

    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn create_idx(client: &Client, base_url: &str, col: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({ "path": path });
    let resp: Value = client.post(format!("{}/api/v1/collections/{}/indexes", base_url, col)).json(&payload).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn compact_col(client: &Client, base_url: &str, col: &str) -> Result<(), Box<dyn std::error::Error>> {
    let resp: Value = client.post(format!("{}/api/v1/collections/{}/compact", base_url, col)).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

async fn run_repl(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "==================================================".cyan());
    println!("{}", "  ⚡ SUPERBASE REPL - Interactive Terminal Shell".bold().yellow());
    println!("{}", format!("  Connecting to: {}", base_url).green());
    println!("{}", "  Type 'help' for available commands or 'exit' to quit.".dimmed());
    println!("{}", "==================================================".cyan());

    loop {
        print!("{} ", "superbase>".bold().blue());
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "exit" || trimmed == "quit" {
            println!("Goodbye!");
            break;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        match parts[0] {
            "help" => {
                println!("{}", "Available commands:".bold());
                println!("  stats                                      Show DB stats");
                println!("  list                                       List collections");
                println!("  create <col>                               Create collection");
                println!("  drop <col>                                 Drop collection");
                println!("  insert <col> <json>                        Insert document");
                println!("  get <col> <id>                             Get document");
                println!("  delete <col> <id>                          Delete document");
                println!("  find <col> [filter_json]                   Find documents");
                println!("  index <col> <json_path>                    Create index");
                println!("  compact <col>                              Trigger compaction");
                println!("  exit                                       Quit REPL");
            }
            "stats" => {
                let _ = print_stats(client, base_url).await;
            }
            "list" => {
                let _ = list_collections(client, base_url).await;
            }
            "create" if parts.len() > 1 => {
                let _ = create_col(client, base_url, parts[1]).await;
            }
            "drop" if parts.len() > 1 => {
                let _ = drop_col(client, base_url, parts[1]).await;
            }
            "insert" if parts.len() > 2 => {
                let col = parts[1];
                let json_str = &trimmed[parts[0].len() + parts[1].len() + 2..];
                let _ = insert_doc(client, base_url, col, json_str).await;
            }
            "get" if parts.len() > 2 => {
                let _ = get_doc(client, base_url, parts[1], parts[2]).await;
            }
            "delete" if parts.len() > 2 => {
                let _ = delete_doc(client, base_url, parts[1], parts[2]).await;
            }
            "find" if parts.len() > 1 => {
                let col = parts[1];
                let filter = if parts.len() > 2 {
                    Some(trimmed[parts[0].len() + parts[1].len() + 2..].to_string())
                } else {
                    None
                };
                let _ = query_docs(client, base_url, col, filter, None, Some(20)).await;
            }
            "index" if parts.len() > 2 => {
                let _ = create_idx(client, base_url, parts[1], parts[2]).await;
            }
            "compact" if parts.len() > 1 => {
                let _ = compact_col(client, base_url, parts[1]).await;
            }
            unknown => {
                println!("{}", format!("Unknown command: '{}'. Type 'help'.", unknown).red());
            }
        }
    }

    Ok(())
}
