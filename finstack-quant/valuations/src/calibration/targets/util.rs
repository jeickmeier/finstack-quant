//! Calibration target construction and shared input validation.
//!
use crate::calibration::prepared::CalibrationQuote;
use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::market::build::context::BuildCtx;
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::quotes::market_quote::{ExtractQuotes, MarketQuote};
use crate::market::quotes::rates::RateQuote;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use std::cell::RefCell;

/// Resolve the day count convention for a discount or forward curve from market conventions.
pub(crate) fn curve_day_count_from_quotes(quotes: &[RateQuote]) -> Result<DayCount> {
    let registry = ConventionRegistry::try_global()?;
    let mut curve_dc: Option<DayCount> = None;

    for q in quotes {
        let index_id = match q {
            RateQuote::Deposit { index, .. }
            | RateQuote::Fra { index, .. }
            | RateQuote::Swap { index, .. } => index.clone(),
            RateQuote::Futures { contract, .. } => {
                registry.require_ir_future(contract)?.index_id.clone()
            }
        };

        let idx_conv = registry.require_rate_index(&index_id)?;
        match curve_dc {
            Some(dc) if dc != idx_conv.day_count => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Mixed rate index day counts for curve construction: got {:?} and {:?}",
                    dc, idx_conv.day_count
                )));
            }
            Some(_) => {}
            None => curve_dc = Some(idx_conv.day_count),
        }
    }

    curve_dc.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Unable to resolve curve day count: no rate quotes provided".to_string(),
        )
    })
}

/// Result of preparing rate quotes for a calibration target.
pub(crate) struct PreparedRateQuotes {
    /// Prepared quotes ready for the solver.
    pub(crate) quotes: Vec<CalibrationQuote>,
    /// Day count convention used for curve time-axis.
    pub(crate) curve_day_count: DayCount,
}

/// Common preflight for rates calibration targets: extract `RateQuote`s, resolve day count,
/// build a `BuildCtx`, and convert each quote into a `PreparedQuote` wrapped in
/// [`CalibrationQuote::Rates`].
///
/// Pass `curve_ids` as the role -> id mapping the underlying instruments expect (typically
/// "discount" and, for projection-aware quotes, "forward"). Pass `explicit_curve_dc = None`
/// to derive the curve time-axis day count from the quote indices.
pub(crate) fn prepare_rate_calibration_quotes(
    quotes: &[MarketQuote],
    base_date: Date,
    curve_ids: finstack_quant_core::HashMap<String, String>,
    explicit_curve_dc: Option<DayCount>,
    residual_notional: f64,
) -> Result<PreparedRateQuotes> {
    prepare_rate_calibration_quotes_with_ois_override(
        quotes,
        base_date,
        curve_ids,
        explicit_curve_dc,
        residual_notional,
        None,
    )
}

/// Variant of [`prepare_rate_calibration_quotes`] that threads an OIS compounding
/// override through `BuildCtx`. Used by `DiscountCurveTarget` to honour
/// step-level OIS-compounding selection without forcing every caller to know
/// about the override.
pub(crate) fn prepare_rate_calibration_quotes_with_ois_override(
    quotes: &[MarketQuote],
    base_date: Date,
    curve_ids: finstack_quant_core::HashMap<String, String>,
    explicit_curve_dc: Option<DayCount>,
    residual_notional: f64,
    ois_compounding_override: Option<FloatingLegCompounding>,
) -> Result<PreparedRateQuotes> {
    let rates_quotes: Vec<RateQuote> = quotes.extract_quotes();
    if rates_quotes.is_empty() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::TooFewPoints,
        ));
    }

    let curve_day_count = match explicit_curve_dc {
        Some(dc) => dc,
        None => curve_day_count_from_quotes(&rates_quotes)?,
    };

    let build_ctx = BuildCtx::new(base_date, residual_notional, curve_ids)
        .with_ois_compounding_override(ois_compounding_override);

    let mut prepared = Vec::with_capacity(rates_quotes.len());
    for q in rates_quotes {
        let pq = crate::market::build::prepared::prepare_rate_quote(
            q,
            &build_ctx,
            curve_day_count,
            base_date,
            true,
        )?;
        prepared.push(CalibrationQuote::Rates(pq));
    }

    Ok(PreparedRateQuotes {
        quotes: prepared,
        curve_day_count,
    })
}

/// Convenience: `{ "discount" => discount_id }` curve-ids map.
pub(crate) fn discount_only_curve_ids(
    discount_id: &str,
) -> finstack_quant_core::HashMap<String, String> {
    let mut m = finstack_quant_core::HashMap::default();
    m.insert("discount".to_string(), discount_id.to_string());
    m
}

/// Convenience: `{ "discount" => discount_id, "forward" => forward_id }` curve-ids map.
pub(crate) fn discount_and_forward_curve_ids(
    discount_id: &str,
    forward_id: &str,
) -> finstack_quant_core::HashMap<String, String> {
    let mut m = finstack_quant_core::HashMap::default();
    m.insert("discount".to_string(), discount_id.to_string());
    m.insert("forward".to_string(), forward_id.to_string());
    m
}

/// Reusable scratch context for sequential bootstrap targets.
///
/// Holds a `RefCell<MarketContext>` that gets mutated in place with the candidate
/// curve before each residual evaluation, avoiding a full `MarketContext::clone()` per call.
/// Bootstrap is inherently sequential per-pillar; the `RefCell` itself is `!Sync`,
/// which prevents accidental cross-thread reuse.
pub(crate) struct ContextScratch {
    base_context: MarketContext,
    reuse: Option<RefCell<MarketContext>>,
}

impl ContextScratch {
    /// Create a new scratch. Always reuses a single `RefCell<MarketContext>` via
    /// `insert_mut` to avoid `MarketContext::clone()` per residual evaluation.
    pub(crate) fn new(base_context: MarketContext) -> Self {
        let reuse = Some(RefCell::new(base_context.clone()));
        Self {
            base_context,
            reuse,
        }
    }

    /// Run `op` against a `MarketContext` containing `curve` plus the base context's data.
    /// Reuses internal scratch (no clone) when configured single-threaded.
    pub(crate) fn with_curve<C, F, T>(&self, curve: &C, op: F) -> Result<T>
    where
        C: Clone + Into<finstack_quant_core::market_data::context::CurveStorage>,
        F: FnOnce(&MarketContext) -> Result<T>,
    {
        if let Some(cell) = &self.reuse {
            // Use `insert_mut` (in-place) rather than the consuming `insert` + `mem::take`
            // pattern. The old code briefly left `Default::default()` inside the cell while
            // `.insert(curve.clone())` ran; a panic in `curve.clone()` or `insert` would
            // poison the scratch with an empty MarketContext (missing the base data) on
            // every subsequent call. `insert_mut` keeps the existing storage intact and
            // only overwrites the single curve entry.
            let mut ctx = cell.borrow_mut();
            ctx.insert_mut(curve.clone());
            op(&ctx)
        } else {
            let temp = self.base_context.clone().insert(curve.clone());
            op(&temp)
        }
    }
}
