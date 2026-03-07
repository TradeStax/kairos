//! Standalone CLI for downloading historical futures data via Databento.
//!
//! Usage:
//!   kairos-download --symbol ES.c.0 --start 2025-01-01 --end 2025-01-31
//!   kairos-download --symbol NQ.c.0 --start 2025-03-01 --end 2025-03-05 --yes

use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDate;

use kairos_data::adapter::databento::{DatabentoConfig, fetcher::DatabentoAdapter};
use kairos_data::cache::store::CacheStore;

fn print_usage() {
    eprintln!(
        "kairos-download — download historical futures data via Databento\n\
         \n\
         Usage:\n  \
           kairos-download --symbol <SYM> --start <YYYY-MM-DD> \
         --end <YYYY-MM-DD> [OPTIONS]\n\
         \n\
         Options:\n  \
           --symbol <SYM>         Continuous contract symbol (e.g. ES.c.0)\n  \
           --start <YYYY-MM-DD>   Start date (inclusive)\n  \
           --end   <YYYY-MM-DD>   End date (inclusive)\n  \
           --schema <SCHEMA>      Data schema: trades (default), depth, ohlcv\n  \
           --api-key <KEY>        Databento API key \
         (default: DATABENTO_API_KEY env)\n  \
           --cache-dir <PATH>     Cache directory \
         (default: ~/.cache/kairos)\n  \
           --yes                  Skip confirmation prompt\n  \
           --help                 Show this help"
    );
}

fn default_cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("kairos")
}

fn parse_date(s: &str, label: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_else(|e| {
        eprintln!("Error: invalid {} date '{}': {}", label, s, e);
        std::process::exit(1);
    })
}

struct CliArgs {
    symbol: String,
    start: NaiveDate,
    end: NaiveDate,
    schema: databento::dbn::Schema,
    api_key: String,
    cache_dir: PathBuf,
    skip_confirm: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();

    let mut symbol: Option<String> = None;
    let mut start: Option<String> = None;
    let mut end: Option<String> = None;
    let mut schema_str: Option<String> = None;
    let mut api_key: Option<String> = None;
    let mut cache_dir: Option<String> = None;
    let mut skip_confirm = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--symbol" => {
                i += 1;
                symbol = args.get(i).cloned();
            }
            "--start" => {
                i += 1;
                start = args.get(i).cloned();
            }
            "--end" => {
                i += 1;
                end = args.get(i).cloned();
            }
            "--schema" => {
                i += 1;
                schema_str = args.get(i).cloned();
            }
            "--api-key" => {
                i += 1;
                api_key = args.get(i).cloned();
            }
            "--cache-dir" => {
                i += 1;
                cache_dir = args.get(i).cloned();
            }
            "--yes" | "-y" => {
                skip_confirm = true;
            }
            other => {
                eprintln!("Error: unknown argument '{}'", other);
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let symbol = symbol.unwrap_or_else(|| {
        eprintln!("Error: --symbol is required");
        print_usage();
        std::process::exit(1);
    });

    let start = start.unwrap_or_else(|| {
        eprintln!("Error: --start is required");
        print_usage();
        std::process::exit(1);
    });

    let end = end.unwrap_or_else(|| {
        eprintln!("Error: --end is required");
        print_usage();
        std::process::exit(1);
    });

    let start = parse_date(&start, "start");
    let end = parse_date(&end, "end");

    if end < start {
        eprintln!("Error: --end date must be >= --start date");
        std::process::exit(1);
    }

    let schema = match schema_str.as_deref() {
        Some("trades") | None => databento::dbn::Schema::Trades,
        Some("depth") => databento::dbn::Schema::Mbp10,
        Some("ohlcv") => databento::dbn::Schema::Ohlcv1M,
        Some(other) => {
            eprintln!(
                "Error: unsupported schema '{}'. \
                 Use 'trades', 'depth', or 'ohlcv'.",
                other
            );
            std::process::exit(1);
        }
    };

    let api_key = api_key
        .or_else(|| std::env::var("DATABENTO_API_KEY").ok())
        .unwrap_or_else(|| {
            eprintln!(
                "Error: no API key provided. Use --api-key or set \
                 DATABENTO_API_KEY env var."
            );
            std::process::exit(1);
        });

    let cache_dir = cache_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_cache_dir);

    CliArgs {
        symbol,
        start,
        end,
        schema,
        api_key,
        cache_dir,
        skip_confirm,
    }
}

#[tokio::main]
async fn main() {

    let args = parse_args();
    let total_days = (args.end - args.start).num_days() + 1;

    let schema_label = match args.schema {
        databento::dbn::Schema::Trades => "trades",
        databento::dbn::Schema::Mbp10 => "depth (MBP-10)",
        databento::dbn::Schema::Ohlcv1M => "ohlcv (1-min)",
        _ => "unknown",
    };

    println!("Kairos Data Download");
    println!("--------------------");
    println!("  Symbol:    {}", args.symbol);
    println!("  Schema:    {}", schema_label);
    println!("  Range:     {} to {}", args.start, args.end);
    println!("  Days:      {}", total_days);
    println!("  Cache dir: {}", args.cache_dir.display());
    println!();

    if !args.skip_confirm {
        eprint!("Proceed? [y/N] ");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err()
            || !matches!(
                input.trim().to_lowercase().as_str(),
                "y" | "yes"
            )
        {
            println!("Aborted.");
            std::process::exit(0);
        }
    }

    // Initialize cache
    let cache = Arc::new(CacheStore::new(args.cache_dir.clone()));
    if let Err(e) = cache.init().await {
        eprintln!("Error: failed to initialize cache: {}", e);
        std::process::exit(1);
    }

    // Check which days are already cached
    let cache_schema = match args.schema {
        databento::dbn::Schema::Trades => {
            kairos_data::cache::store::CacheSchema::Trades
        }
        databento::dbn::Schema::Mbp10 => {
            kairos_data::cache::store::CacheSchema::Depth
        }
        databento::dbn::Schema::Ohlcv1M => {
            kairos_data::cache::store::CacheSchema::Ohlcv
        }
        _ => kairos_data::cache::store::CacheSchema::Trades,
    };

    let mut already_cached = 0u64;
    let mut current = args.start;
    while current <= args.end {
        if cache.has_day(&args.symbol, cache_schema, current).await {
            already_cached += 1;
        }
        current += chrono::Duration::days(1);
    }

    if already_cached > 0 {
        println!(
            "{} of {} days already cached, will skip those.",
            already_cached, total_days
        );
    }

    if already_cached as i64 == total_days {
        println!("All days already cached. Nothing to download.");
        std::process::exit(0);
    }

    // Create adapter
    let config = DatabentoConfig::with_api_key(args.api_key);
    let mut adapter =
        match DatabentoAdapter::new(config, cache).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!(
                    "Error: failed to create Databento adapter: {}",
                    e
                );
                std::process::exit(1);
            }
        };

    // Download the full range (adapter skips already-cached days)
    println!("Downloading...");

    match adapter
        .fetch_and_cache_range(
            &args.symbol,
            args.schema,
            args.start,
            args.end,
        )
        .await
    {
        Ok(saved) => {
            println!();
            println!(
                "Done. {} days downloaded, {} were already cached.",
                saved, already_cached
            );
        }
        Err(e) => {
            eprintln!("Error during download: {}", e);
            std::process::exit(1);
        }
    }
}
