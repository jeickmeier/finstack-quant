//! Shared currency-from-curve-id inference for bumping helpers.
//!
//! Best-effort heuristic used when a caller holds a curve but not its
//! currency metadata explicitly. Returns `USD` when no known token matches.

use finstack_quant_core::currency::Currency;

/// Infer currency from a curve ID using token-by-token matching.
///
/// Splits the ID on non-alphanumeric separators, uppercases each token, and
/// matches against well-known currency and benchmark-rate aliases
/// (OIS flavours, SOFR, ESTR, SONIA, TONA, …). Returns `Currency::USD` when
/// no token matches.
pub(crate) fn infer_currency_from_id(id: &str) -> Currency {
    let uppercase = id.to_ascii_uppercase();
    let tokens = uppercase
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty());

    for token in tokens {
        match token {
            "USD" | "USDOIS" | "SOFR" => return Currency::USD,
            "EUR" | "EUROIS" | "ESTR" | "ESTER" => return Currency::EUR,
            "GBP" | "GBPOIS" | "SONIA" => return Currency::GBP,
            "JPY" | "JPYOIS" | "TONA" => return Currency::JPY,
            _ => {}
        }
    }

    Currency::USD
}
