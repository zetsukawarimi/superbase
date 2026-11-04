use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use superbase::{run_server, Database, DatabaseOptions};

#[derive(Parser, Debug)]
#[command(
    name = "superbase-server",
    about = "🚀 Superbase - Ultra-fast JSON Document Database in 100% Rust"
)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Data directory path
    #[arg(short, long, default_value = "./superbase_data")]
    data_dir: PathBuf,

    /// Enable immediate synchronous disk fsync (slower, maximum durability)
    #[arg(long, default_value_t = false)]
    sync_immediate: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "superbase=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    println!("============================================================");
    println!("  ⚡ SUPERBASE: Ultra-fast JSON Database Engine (Full Rust)");
    println!("============================================================");
    println!("  📁 Storage Directory : {:?}", args.data_dir);
    println!("  🌐 Listening on     : http://{}", addr);
    println!("  ⚡ Fsync Mode        : {}", if args.sync_immediate { "Strict Sync" } else { "High-Throughput Async" });
    println!("============================================================");

    let options = DatabaseOptions {
        sync_immediate: args.sync_immediate,
    };
    let db = Database::open(&args.data_dir, Some(options))?;

    run_server(db, addr).await?;

    Ok(())
}
