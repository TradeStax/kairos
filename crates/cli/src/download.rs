//! Data download command

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;

use kairos_data::adapter::databento::{DatabentoConfig, fetcher::DatabentoAdapter};
use kairos_data::cache::store::{CacheStore, CacheSchema};
use kairos_data::cache::CacheProvider;

/// Download historical market data from Databento
#[derive(Args)]
pub struct DownloadArgs {
    /// Continuous contract symbol (e.g., ES.c.0, NQ.c.0)
    #[arg(short, long)]
    pub symbol: String,

    /// Start date (YYYY-MM-DD, inclusive)
    #[arg(long)]
    pub start: String,

    /// End date (YYYY-MM-DD, inclusive)
    #[arg(long)]
    pub end: String,

    /// Data schema to download
    #[arg(long, default_value = "trades", value_parser = ["trades", "depth", "ohlcv"])]
    pub schema: String,

    /// Databento API key
    #[arg(short, long)]
    pub api_key: Option<String>,

    /// Cache directory
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// List available schemas and exit
    #[arg(long)]
    pub list_schemas: bool,
}

pub async fn run(args: DownloadArgs) -> Result<()> {
    if args.list_schemas {
        println!("Available schemas:");
        println!("  trades  - Individual trade ticks (default)");
        println!("  depth   - Market by price (MBP-10) order book");
        println!("  ohlcv  - 1-minute OHLCV bars");
        return Ok(());
    }

    let start = chrono::NaiveDate::parse_from_str(&args.start, "%Y-%m-%d")
        .with_context(|| format!("Invalid start date: {}", args.start))?;
    let end = chrono::NaiveDate::parse_from_str(&args.end, "%Y-%m-%d")
        .with_context(|| format!("Invalid end date: {}", args.end))?;

    if end < start {
        anyhow::bail!("--end date must be >= --start date");
    }

    let total_days = (end - start).num_days() + 1;

    let api_key = args.api_key
        .or_else(|| std::env::var("DATABENTO_API_KEY").ok())
        .unwrap_or_else(|| {
            eprintln!("Error: No API key provided. Use --api-key or set DATABENTO_API_KEY");
            std::process::exit(1);
        });

    let cache_dir = args.cache_dir.unwrap_or_else(|| {
        dirs_next::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("kairos")
    });

    let schema = match args.schema.as_str() {
        "trades" => databento::dbn::Schema::Trades,
        "depth" => databento::dbn::Schema::Mbp10,
        "ohlcv" => databento::dbn::Schema::Ohlcv1M,
        _ => anyhow::bail!("Unknown schema: {}", args.schema),
    };

    let schema_label = match args.schema.as_str() {
        "trades" => "trades",
        "depth" => "depth (MBP-10)",
        "ohlcv" => "ohlcv (1-min)",
        _ => "unknown",
    };

    println!("Kairos Data Download");
    println!("====================");
    println!("  Symbol:    {}", args.symbol);
    println!("  Schema:    {}", schema_label);
    println!("  Range:     {} to {}", start, end);
    println!("  Days:      {}", total_days);
    println!("  Cache dir: {}", cache_dir.display());
    println!();

    if !args.yes {
        eprint!("Proceed? [y/N] ");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err()
            || !matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
        {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Initialize cache
    let cache = Arc::new(CacheStore::new(cache_dir));
    cache.init().await
        .with_context(|| "Failed to initialize cache")?;

    let cache_schema = match args.schema.as_str() {
        "trades" => CacheSchema::Trades,
        "depth" => CacheSchema::Depth,
        "ohlcv" => CacheSchema::Ohlcv,
        _ => CacheSchema::Trades,
    };

    // Check which days are already cached
    let mut already_cached = 0u64;
    let mut current = start;
    while current <= end {
        if cache.has_day(CacheProvider::Databento, &args.symbol, cache_schema, current).await {
            already_cached += 1;
        }
        current += chrono::Duration::days(1);
    }

    if already_cached > 0 {
        println!("{} of {} days already cached, will skip those.", already_cached, total_days);
    }

    if already_cached as i64 == total_days {
        println!("All days already cached. Nothing to download.");
        return Ok(());
    }

    // Create adapter
    let config = DatabentoConfig::with_api_key(api_key);
    let mut adapter = DatabentoAdapter::new(config).await
        .with_context(|| "Failed to create Databento adapter")?;

    println!("Downloading...");

    match adapter.fetch_and_cache_range(&args.symbol, schema, start, end).await {
        Ok(saved) => {
            println!();
            println!("Done. {} days downloaded, {} were already cached.", saved, already_cached);
        }
        Err(e) => {
            anyhow::bail!("Download failed: {}", e);
        }
    }

    Ok(())
}
