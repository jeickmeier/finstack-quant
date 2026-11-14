//! Forward-curve convention defaults inferred from the curve identifier.
//!
use crate::dates::DayCount;

const KNOWN_CURRENCIES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "SEK", "NOK",
];

const EXPLICIT_TERM_MARKERS: &[&str] =
    &["1W", "2W", "1M", "2M", "3M", "6M", "9M", "12M", "18M", "1Y"];

#[inline]
fn normalize_curve_id(id: &str) -> String {
    id.trim().to_ascii_uppercase()
}

#[inline]
fn tokenize_curve_id(normalized_id: &str) -> Vec<&str> {
    normalized_id
        .split(['-', '_', '/', ' '])
        .filter(|s| !s.is_empty())
        .collect()
}

#[inline]
fn leading_currency_code<'a>(tokens: &[&'a str]) -> Option<&'a str> {
    match tokens.first().copied() {
        Some(code) if KNOWN_CURRENCIES.contains(&code) => Some(code),
        Some(first) => KNOWN_CURRENCIES
            .iter()
            .copied()
            .find(|&ccy| first.starts_with(ccy) && first.len() > ccy.len()),
        None => None,
    }
}

#[inline]
fn matches_bare_index(candidate: &str, index: &str) -> bool {
    if candidate == index {
        return true;
    }
    if let Some(rest) = candidate.strip_prefix(index) {
        return rest.starts_with(|c: char| c.is_ascii_digit()) || rest == "ON" || rest == "OIS";
    }
    if index == "LIBOR"
        && let Some(rest) = candidate.strip_prefix("LIB")
    {
        return EXPLICIT_TERM_MARKERS.contains(&rest) || rest == "1D" || rest == "ON";
    }
    false
}

#[inline]
fn token_matches_index(token: &str, index: &str) -> bool {
    if matches_bare_index(token, index) {
        return true;
    }
    KNOWN_CURRENCIES
        .iter()
        .filter_map(|&ccy| token.strip_prefix(ccy).filter(|rest| !rest.is_empty()))
        .any(|stripped| matches_bare_index(stripped, index))
}

#[inline]
fn has_any_index(tokens: &[&str], indices: &[&str]) -> bool {
    tokens.iter().any(|token| {
        indices
            .iter()
            .any(|index| token_matches_index(token, index))
    })
}

#[inline]
fn has_explicit_term_marker(tokens: &[&str]) -> bool {
    tokens.iter().any(|token| {
        EXPLICIT_TERM_MARKERS
            .iter()
            .any(|marker| *token == *marker || token.ends_with(marker))
    })
}

#[inline]
fn has_overnight_tenor_marker(tokens: &[&str]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(*token, "1D" | "ON") || token.ends_with("1D"))
}

#[inline]
fn inferred_currency_day_count(currency: &str) -> DayCount {
    if matches!(currency, "USD" | "EUR" | "CHF" | "SEK" | "NOK") {
        DayCount::Act360
    } else {
        DayCount::Act365F
    }
}

/// Infer forward-curve `(day_count, reset_lag_business_days)` defaults from an
/// index identifier.
///
/// The *index* day-count basis is matched by token on the normalized (trimmed,
/// upper-cased) ID: known index names first (e.g. `SOFR` ⇒ Act/360,
/// `SONIA` ⇒ Act/365F), then a leading currency code (e.g. `USD-...` ⇒
/// Act/360). The fallback remains `Act365F` for synthetic IDs that carry no
/// market hint.
///
/// Discount curves do **not** use this helper: their default time basis is
/// always [`DayCount::Act365F`]. Callers that need a money-market Act/360
/// discount axis must set `day_count(...)` explicitly on the discount builder.
///
/// Reset lag is interpreted in business days using positive T-minus semantics.
#[inline]
pub(crate) fn infer_forward_curve_defaults(id: &str) -> (DayCount, i32) {
    let normalized_id = normalize_curve_id(id);
    let tokens = tokenize_curve_id(&normalized_id);
    let currency = leading_currency_code(&tokens);

    let day_count = if has_any_index(
        &tokens,
        &["SOFR", "FEDFUNDS", "EFFR", "ESTR", "EURIBOR", "SARON"],
    ) {
        DayCount::Act360
    } else if has_any_index(
        &tokens,
        &[
            "SONIA", "TONAR", "TONA", "TIBOR", "CORRA", "CDOR", "AONIA", "BBSW", "BKBM",
        ],
    ) {
        DayCount::Act365F
    } else if let Some(ccy) = currency {
        inferred_currency_day_count(ccy)
    } else {
        DayCount::Act365F
    };

    tracing::debug!(
        curve_id = id,
        day_count = ?day_count,
        "Inferred day-count basis from curve ID; set day_count explicitly on the builder to override"
    );

    let is_overnight = has_any_index(&tokens, &["OIS"])
        || has_overnight_tenor_marker(&tokens)
        || has_any_index(
            &tokens,
            &[
                "SONIA", "TONAR", "TONA", "SARON", "ESTR", "FEDFUNDS", "EFFR", "CORRA", "AONIA",
            ],
        )
        || (has_any_index(&tokens, &["SOFR"]) && !has_explicit_term_marker(&tokens));

    let reset_lag_business_days = if is_overnight
        || currency == Some("GBP")
        || has_any_index(&tokens, &["BBSW", "CDOR", "BKBM"])
    {
        0
    } else if has_any_index(&tokens, &["SOFR", "EURIBOR", "LIBOR", "TIBOR"]) {
        2
    } else {
        0
    };

    (day_count, reset_lag_business_days)
}
