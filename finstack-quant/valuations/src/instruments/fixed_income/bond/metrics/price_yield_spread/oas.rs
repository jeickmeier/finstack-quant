use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates Option-Adjusted Spread for bonds with embedded options.
///
/// Uses short-rate trees (or rates+credit trees when hazard curves are present)
/// to value callable/putable bonds and solve for the spread (in **decimal units**,
/// e.g. `0.01 = 100bp`) that makes the model price equal to the market price.
///
/// OAS accounts for the value of embedded call/put options by using tree-based
/// pricing with backward induction to properly value optionality.
///
/// # Dependencies
///
/// Requires `quoted_clean_price` to be set in `bond.pricing_overrides`.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // OAS is computed automatically when requesting bond metrics for callable/putable bonds
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Require quoted clean price
        let clean_price = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "bond.instrument_pricing_overrides.market_quotes.quoted_clean_price"
                        .to_string(),
                })
            })?;

        // Use MarketContext directly (no conversion needed)
        let market_context = context.curves.as_ref().clone();

        // Use the bond-owned tree model inputs (steps, volatility, mean
        // reversion, model choice) instead of the generic default pricer.
        let oas_calculator =
            crate::instruments::fixed_income::bond::pricing::engine::tree::TreePricer::with_config(
                crate::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config(
                    bond,
                )?,
            );
        // Tree pricer returns OAS in **basis points**; convert to decimal
        // so all bond spread-style metrics use a consistent convention
        // (0.01 = 100bp) at the public API surface.
        let oas_bp =
            oas_calculator.calculate_oas(bond, &market_context, context.as_of, clean_price)?;
        Ok(oas_bp / 10_000.0)
    }
}
