use anyhow::Result;
use clap::{Parser, Subcommand};

mod backtest;
mod download;
mod ml;

#[derive(Parser)]
#[command(name = "kairos")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Backtest(backtest::BacktestArgs),
    Download(download::DownloadArgs),
    ListStrategies,
    ListSymbols,
    DebugData(DebugDataArgs),
    /// ML model management and training commands
    Ml(ml::MlArgs),
}

#[derive(clap::Args)]
struct DebugDataArgs {
    #[arg(long, required = true)]
    path: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Backtest(args) => backtest::run(args).await?,
        Commands::Download(args) => download::run(args).await?,
        Commands::ListStrategies => list_strategies(),
        Commands::ListSymbols => list_symbols(),
        Commands::DebugData(args) => debug_data(&args.path).await?,
        Commands::Ml(args) => ml::run(args).await?,
    }

    Ok(())
}

fn list_strategies() {
    use kairos_backtest::strategy::registry::StrategyRegistry;
    let registry = StrategyRegistry::with_built_ins();
    
    println!("Available Strategies");
    println!("====================");
    for info in registry.list() {
        println!("{}: {}", info.id, info.name);
        println!("   {}", info.description);
        println!();
    }
    
    // Print ML strategy info
    println!("ml: LSTM Neural Network Strategy");
    println!("   Machine learning-based strategy using trained PyTorch models.");
    println!("   Requires: --model-path <path-to-trained-model.pt>");
    println!("   Features: 12 technical indicators (SMA, EMA, RSI, ATR, MACD, BB, VWAP)");
    println!("   Usage: kairos backtest --strategy ml --model-path models/model.pt [options]");
    println!();
}

fn list_symbols() {
    println!("Supported Futures Symbols");
    println!("========================");
    println!("NQ  - Nasdaq 100 (MNQ micro: $2/tick, NQ: $20/tick)");
    println!("ES  - S&P 500 (MES micro: $1.25/tick, ES: $50/tick)");
    println!("YM  - Dow Jones (MYM micro: $0.50/tick, YM: $5/tick)");
    println!("RTY - Russell (M2K micro: $1/tick, RTY: $50/tick)");
    println!("GC  - Gold (MGC micro: $0.10/tick, GC: $100/tick)");
    println!("SI  - Silver (SIL micro: $0.25/tick, SI: $5000/tick)");
    println!("CL  - Crude Oil (CL: $10/tick)");
    println!("NG  - Natural Gas (NG: $10,000/tick)");
    println!("HG  - Copper (HG: $25,000/tick)");
    println!("ZN  - 10Y Treasury (ZN: $1000/tick)");
    println!("ZB  - 30Y Treasury (ZB: $1000/tick)");
    println!("ZF  - 5Y Treasury (ZF: $1000/tick)");
}

async fn debug_data(path: &std::path::Path) -> Result<()> {
    use databento::dbn::decode::AsyncDbnDecoder;
    use databento::dbn::TradeMsg;
    
    println!("Reading DBN file: {}", path.display());
    println!("==================");
    
    let mut decoder = AsyncDbnDecoder::from_zstd_file(&path).await?;
    
    let mut count = 0;
    let mut prices: Vec<i64> = Vec::new();
    
    while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
        let raw_price = msg.price;
        prices.push(raw_price);
        
        if count < 10 {
            let rp = raw_price as f64;
            println!("\nRecord {}", count + 1);
            println!("  Raw price bytes: {:016x}", raw_price);
            println!("  Raw price dec:   {}", raw_price);
            println!("  Price / 1e9:     {:.4}", rp / 1e9);
            println!("  Price / 1e8:     {:.4}", rp / 1e8);
            println!("  Price / 1e7:     {:.4}", rp / 1e7);
            println!("  Price / 1e6:     {:.4}", rp / 1e6);
        }
        count += 1;
        if count >= 1000 { break; }
    }
    
    println!("\n==================");
    println!("Total records: {}", count);
    
    if !prices.is_empty() {
        let min = prices.iter().min().unwrap();
        let max = prices.iter().max().unwrap();
        let min_f = *min as f64;
        let max_f = *max as f64;
        println!("Price range: {} to {}", min, max);
        println!("As $/unit (÷1e9): {:.2} to {:.2}", min_f / 1e9, max_f / 1e9);
        println!("As $/unit (÷1e8): {:.4} to {:.4}", min_f / 1e8, max_f / 1e8);
    }
    
    Ok(())
}

async fn analyze_outliers(path: &std::path::Path) -> anyhow::Result<()> {
    use databento::dbn::decode::AsyncDbnDecoder;
    use databento::dbn::TradeMsg;
    
    let mut decoder = AsyncDbnDecoder::from_zstd_file(&path).await?;
    
    let mut prices: Vec<f64> = Vec::new();
    let mut total = 0u64;
    let mut outliers = 0u64;
    
    while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
        total += 1;
        
        let price = msg.price as f64 / 1e9;
        
        if prices.len() >= 5 {
            let last_5: f64 = prices.iter().rev().take(5).sum::<f64>() / 5.0;
            let pct_diff = ((price - last_5) / last_5 * 100.0).abs();
            
            if pct_diff > 25.0 {
                outliers += 1;
            }
        }
        
        prices.push(price);
        
        if total >= 500_000 {
            break;
        }
    }
    
    let pct = outliers as f64 / total as f64 * 100.0;
    println!("\n=== OUTLIER ANALYSIS ===");
    println!("File: {}", path.display());
    println!("Total ticks: {}", total);
    println!("Outliers (>25% off 5-tick avg): {}", outliers);
    println!("Percentage: {:.2}%", pct);
    
    Ok(())
}
