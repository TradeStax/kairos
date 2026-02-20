use exchange::{FuturesTicker, FuturesTickerInfo, FuturesVenue};
use rustc_hash::FxHashMap;

/// CME Futures Products - Lookup table for ticker info (tick sizes, etc.)
pub(crate) const FUTURES_PRODUCTS: &[(&str, &str, f32, f32, f32)] = &[
    ("ES.c.0", "E-mini S&P 500", 0.25, 1.0, 50.0),
    ("NQ.c.0", "E-mini Nasdaq-100", 0.25, 1.0, 20.0),
    ("YM.c.0", "E-mini Dow", 1.0, 1.0, 5.0),
    ("RTY.c.0", "E-mini Russell 2000", 0.1, 1.0, 50.0),
    ("CL.c.0", "Crude Oil", 0.01, 1.0, 1000.0),
    ("GC.c.0", "Gold", 0.10, 1.0, 100.0),
    ("SI.c.0", "Silver", 0.005, 1.0, 5000.0),
    ("ZN.c.0", "10-Year T-Note", 0.015625, 1.0, 1000.0),
    ("ZB.c.0", "30-Year T-Bond", 0.03125, 1.0, 1000.0),
    ("ZF.c.0", "5-Year T-Note", 0.0078125, 1.0, 1000.0),
    ("NG.c.0", "Natural Gas", 0.001, 1.0, 10000.0),
    ("HG.c.0", "Copper", 0.0005, 1.0, 25000.0),
];

/// Rebuild the tickers_info map from a set of available symbols.
pub(crate) fn build_tickers_info(
    available_symbols: std::collections::HashSet<String>,
) -> FxHashMap<FuturesTicker, FuturesTickerInfo> {
    log::info!(
        "Ticker list updated: {} tickers available",
        available_symbols.len()
    );

    let venue = FuturesVenue::CMEGlobex;
    let mut info = FxHashMap::default();

    for (symbol, product_name, tick_size, min_qty, contract_size) in FUTURES_PRODUCTS {
        if !available_symbols.contains(*symbol) {
            continue;
        }

        let ticker = FuturesTicker::new_with_display(
            symbol,
            venue,
            Some(symbol.split('.').next().unwrap()),
            Some(product_name),
        );

        let ticker_info = FuturesTickerInfo::new(ticker, *tick_size, *min_qty, *contract_size);

        info.insert(ticker, ticker_info);
    }

    info
}
