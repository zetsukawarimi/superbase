use clap::Parser;
use colored::Colorize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use superbase::{run_server, Database, DatabaseOptions};

#[derive(Parser, Debug)]
#[command(
    name = "superbase-studio",
    about = "⚡ Superbase Studio - Lightweight, Full-Rust Management & Control GUI"
)]
struct Args {
    /// Port for the Studio UI
    #[arg(short, long, default_value_t = 9090)]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Database data directory path
    #[arg(short, long, default_value = "./superbase_data")]
    data_dir: PathBuf,

    /// Automatically open in default browser
    #[arg(long, default_value_t = true)]
    open: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "superbase=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let url = format!("http://{}", addr);

    println!("{}", "==================================================================".cyan());
    println!("{}", "   ⚡ SUPERBASE STUDIO - Rust Database Management & Control GUI".bold().yellow());
    println!("{}", "==================================================================".cyan());
    println!("  📁 Storage Directory : {:?}", args.data_dir);
    println!("  🌐 Studio Dashboard  : {}", url.bold().green());
    println!("  ⚡ Mode              : Full Rust GUI (Native Web & Desktop Control)");
    println!("{}", "==================================================================".cyan());

    let db = Database::open(&args.data_dir, Some(DatabaseOptions::default()))?;

    // Automatically open in default desktop browser if requested
    if args.open {
        let url_clone = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("xdg-open").arg(&url_clone).spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("open").arg(&url_clone).spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("cmd").args(["/C", "start", &url_clone]).spawn();
            }
        });
    }

    println!("\n{}", "✨ Superbase Studio is ready!".green().bold());
    println!("Press {} to stop the management server.\n", "Ctrl+C".bold());

    run_server(db, addr).await?;

    Ok(())
}
