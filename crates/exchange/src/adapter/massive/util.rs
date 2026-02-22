use super::{MassiveError, MassiveResult};
use kairos_data::repository::RepositoryError;

/// Extract underlying ticker from option contract ticker.
///
/// Validates that:
/// - The ticker starts with the `O:` prefix
/// - The underlying symbol is alphabetic-only (e.g. `AAPL`, `SPY`)
///
/// Example: `"O:AAPL240119C00150000"` -> `"AAPL"`
pub(crate) fn extract_underlying_massive(contract_ticker: &str) -> MassiveResult<String> {
    if !contract_ticker.starts_with("O:") {
        return Err(MassiveError::InvalidContractTicker(format!(
            "Invalid format: {}",
            contract_ticker
        )));
    }

    let without_prefix = &contract_ticker[2..];

    if without_prefix.is_empty() {
        return Err(MassiveError::InvalidContractTicker(format!(
            "Contract ticker has no symbol after 'O:' prefix: {}",
            contract_ticker
        )));
    }

    let ticker_end = without_prefix
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(0);

    if ticker_end == 0 {
        return Err(MassiveError::InvalidContractTicker(format!(
            "Cannot extract underlying from: {}",
            contract_ticker
        )));
    }

    let underlying = &without_prefix[..ticker_end];

    if !underlying.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(MassiveError::InvalidContractTicker(format!(
            "Underlying ticker contains non-alphabetic characters: '{}'",
            underlying
        )));
    }

    Ok(underlying.to_string())
}

/// Extract underlying ticker from option contract ticker, returning a RepositoryError.
///
/// Thin wrapper over [`extract_underlying_massive`] for use in repository impls.
pub(crate) fn extract_underlying_repo(contract_ticker: &str) -> Result<String, RepositoryError> {
    extract_underlying_massive(contract_ticker).map_err(|e| {
        RepositoryError::InvalidData(format!("{}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_underlying_massive() {
        assert_eq!(
            extract_underlying_massive("O:AAPL240119C00150000").unwrap(),
            "AAPL"
        );
        assert_eq!(
            extract_underlying_massive("O:SPY240119P00450000").unwrap(),
            "SPY"
        );
        assert_eq!(
            extract_underlying_massive("O:TSLA240315C00200000").unwrap(),
            "TSLA"
        );

        // Missing O: prefix
        assert!(extract_underlying_massive("AAPL240119C00150000").is_err());
        // Empty after prefix
        assert!(extract_underlying_massive("O:").is_err());
        // No digits (no date portion)
        assert!(extract_underlying_massive("O:AAPL").is_err());
        // Non-alphabetic characters in underlying
        assert!(extract_underlying_massive("O:AA-PL240119C00150000").is_err());
        assert!(extract_underlying_massive("O:A.B240119C00150000").is_err());
    }
}
