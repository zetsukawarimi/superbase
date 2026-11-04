use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use superbase::migration::{export_data, import_data, ExportOptions, MigrationFormat};
use superbase::{Database, DatabaseOptions};

#[derive(Parser, Debug)]
#[command(
    name = "superbase-migrate",
    about = "🔄 Superbase Universal Migration Engine - Migrate to & from MySQL, PostgreSQL, MongoDB, SQLite, CSV & JSON"
)]
struct Cli {
    /// Database data directory
    #[arg(short, long, default_value = "./superbase_data")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export collection data to another database format (MySQL, Postgres, SQLite, MongoDB, CSV, NDJSON, JSON)
    Export {
        /// Source collection name
        #[arg(short, long)]
        collection: String,

        /// Target format (mysql, postgres, sqlite, mongo, csv, json, ndjson)
        #[arg(short, long, default_value = "mysql")]
        format: String,

        /// Output file path (prints to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Flatten top-level JSON fields into native relational columns
        #[arg(long, default_value_t = false)]
        flatten: bool,
    },

    /// Import data from another database format into a Superbase collection
    Import {
        /// Target collection name
        #[arg(short, long)]
        collection: String,

        /// Source format (mysql, postgres, sqlite, mongo, csv, json, ndjson)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Input file path
        #[arg(short, long)]
        input: PathBuf,
    },

    /// Copy collection directly between two Superbase instances
    Copy {
        /// Source collection
        #[arg(long)]
        from_col: String,

        /// Destination collection
        #[arg(long)]
        to_col: String,

        /// Destination directory (defaults to same data-dir)
        #[arg(long)]
        to_dir: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let db = Database::open(&cli.data_dir, Some(DatabaseOptions::default()))?;

    match cli.command {
        Commands::Export {
            collection,
            format,
            output,
            flatten,
        } => {
            let fmt = MigrationFormat::from_str_loose(&format).ok_or_else(|| {
                format!(
                    "Unknown format '{}'. Supported: mysql, postgres, sqlite, mongo, csv, json, ndjson",
                    format
                )
            })?;

            let col = db.collection(&collection)?;
            let start = Instant::now();

            let opts = ExportOptions {
                flatten_columns: flatten,
            };

            let exported = export_data(&col, fmt, Some(opts))?;
            let elapsed = start.elapsed();

            if let Some(out_path) = output {
                fs::write(&out_path, exported)?;
                println!("{}", "✅ Export Completed Successfully!".green().bold());
                println!("  📁 Source Collection : {}", collection.bold());
                println!("  🎯 Target Format     : {:?}", fmt);
                println!("  💾 Output File       : {:?}", out_path);
                println!("  ⏱️  Duration          : {:.2?}", elapsed);
            } else {
                println!("{}", exported);
            }
        }

        Commands::Import {
            collection,
            format,
            input,
        } => {
            let fmt = MigrationFormat::from_str_loose(&format).ok_or_else(|| {
                format!(
                    "Unknown format '{}'. Supported: mysql, postgres, sqlite, mongo, csv, json, ndjson",
                    format
                )
            })?;

            if !input.exists() {
                return Err(format!("Input file '{:?}' does not exist", input).into());
            }

            println!("{}", "🚀 Starting Database Migration to Superbase...".cyan().bold());
            println!("  📁 Target Collection : {}", collection.bold());
            println!("  📦 Source Format     : {:?}", fmt);
            println!("  📄 Input File        : {:?}", input);

            let content = fs::read_to_string(&input)?;
            let col = db.collection(&collection)?;

            let report = import_data(&col, fmt, &content)?;

            println!("\n{}", "🎉 Migration Report:".bold().green());
            println!("  • Total Processed : {}", report.total_processed);
            println!("  • Imported Docs   : {}", report.imported.to_string().green().bold());
            println!("  • Failed Docs     : {}", report.failed);
            println!("  • Time Taken      : {:.2} ms", report.elapsed_ms);

            if !report.errors.is_empty() {
                println!("\n{}", "⚠️ Sample Errors:".yellow());
                for err in &report.errors {
                    println!("  - {}", err);
                }
            }
        }

        Commands::Copy {
            from_col,
            to_col,
            to_dir,
        } => {
            let target_db = if let Some(dir) = to_dir {
                Database::open(&dir, Some(DatabaseOptions::default()))?
            } else {
                db.clone()
            };

            let src = db.collection(&from_col)?;
            let dst = target_db.collection(&to_col)?;

            println!("{}", "🚀 Copying collection...".cyan());
            let exported_json = export_data(&src, MigrationFormat::Json, None)?;
            let report = import_data(&dst, MigrationFormat::Json, &exported_json)?;

            println!("{}", "✅ Collection Copy Complete!".green().bold());
            println!("  • Copied {} documents to '{}'", report.imported, to_col);
        }
    }

    Ok(())
}
