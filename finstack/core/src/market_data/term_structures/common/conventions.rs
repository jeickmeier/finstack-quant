use crate::dates::DayCount;

/// Convention defaults inferred from a forward-curve identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ForwardConventionDefaults {
    pub day_count: DayCount,
    pub reset_lag_business_days: i32,
}

#[inline]
fn normalize_curve_id(id: &str) -> String {
    id.trim().to_ascii_uppercase()
}

#[inline]
fn leading_currency_code(normalized_id: &str) -> Option<&str> {
    match normalized_id.split(['-', '_']).next() {
        Some(
            code @ ("USD" | "EUR" | "GBP" | "JPY" | "CHF" | "CAD" | "AUD" | "NZD" | "SEK" | "NOK"),
        ) => Some(code),
        _ => None,
    }
}

#[inline]
fn contains_any(normalized_id: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| normalized_id.contains(needle))
}

#[inline]
fn has_explicit_term_marker(normalized_id: &str) -> bool {
    contains_any(
        normalized_id,
        &[
            "1D", "1W", "2W", "1M", "2M", "3M", "6M", "9M", "12M", "18M", "1Y",
        ],
    )
}

#[inline]
fn inferred_currency_day_count(currency: &str) -> DayCount {
    match currency {
        "USD" | "EUR" | "CHF" | "SEK" | "NOK" => DayCount::Act360,
        "GBP" | "JPY" | "CAD" | "AUD" | "NZD" => DayCount::Act365F,
        _ => DayCount::Act365F,
    }
}

/// Infer a market-standard day-count basis from a curve identifier.
///
/// Matching is by substring on the normalized (trimmed, upper-cased) ID:
/// known index names first (e.g. `SOFR` ⇒ Act/360, `SONIA` ⇒ Act/365F), then
/// a leading currency code (e.g. `USD-...` ⇒ Act/360). The fallback remains
/// `Act365F` for synthetic IDs that carry no market hint.
///
/// **Build-vs-query basis trap**: the inferred basis only affects how knot
/// *dates* are converted to year fractions when a curve is built from dated
/// pillars, and how query dates are converted back. If the ID is renamed
/// (e.g. `USD-SOFR` → `USD-OIS-1`) the inferred basis can silently change
/// from Act/360 to Act/365F, shifting every pillar time by ~1.4%. Callers
/// that care about the basis should set `day_count(...)` explicitly on the
/// builder rather than relying on inference. Each inference is logged at
/// `debug` level for auditability.
#[inline]
pub(crate) fn infer_discount_curve_day_count(id: &str) -> DayCount {
    let normalized_id = normalize_curve_id(id);

    let inferred = if contains_any(
        &normalized_id,
        &["SOFR", "FEDFUNDS", "EFFR", "ESTR", "EURIBOR", "SARON"],
    ) {
        DayCount::Act360
    } else if contains_any(
        &normalized_id,
        &[
            "SONIA", "TONAR", "TONA", "TIBOR", "CORRA", "CDOR", "AONIA", "BBSW", "BKBM",
        ],
    ) {
        DayCount::Act365F
    } else if let Some(currency) = leading_currency_code(&normalized_id) {
        inferred_currency_day_count(currency)
    } else {
        DayCount::Act365F
    };

    tracing::debug!(
        curve_id = id,
        day_count = ?inferred,
        "Inferred day-count basis from curve ID; set day_count explicitly on the builder to override"
    );

    inferred
}

/// Infer forward-curve day-count and reset-lag defaults from an index identifier.
///
/// Reset lag is interpreted in business days using positive T-minus semantics.
#[inline]
pub(crate) fn infer_forward_curve_defaults(id: &str) -> ForwardConventionDefaults {
    let normalized_id = normalize_curve_id(id);
    let day_count = infer_discount_curve_day_count(id);

    let is_overnight = normalized_id.contains("OIS")
        || contains_any(
            &normalized_id,
            &[
                "SONIA", "TONAR", "TONA", "SARON", "ESTR", "FEDFUNDS", "EFFR", "CORRA", "AONIA",
            ],
        )
        || (normalized_id.contains("SOFR") && !has_explicit_term_marker(&normalized_id));

    let reset_lag_business_days = if is_overnight {
        0
    } else if contains_any(
        &normalized_id,
        &["SOFR", "EURIBOR", "LIBOR", "TIBOR", "BBSW", "CDOR", "BKBM"],
    ) {
        2
    } else {
        0
    };

    ForwardConventionDefaults {
        day_count,
        reset_lag_business_days,
    }
}
