//! JSON construction helpers for bond instruments.

/// Construct tagged bond instrument JSON from a cashflow schedule JSON payload.
///
/// Parses and validates a canonical cashflow schedule, constructs a [`super::Bond`],
/// wraps it in [`crate::instruments::InstrumentJson::Bond`], and serializes the
/// tagged instrument.
///
/// # Arguments
///
/// * `instrument_id` - Stable caller-assigned identifier stored on the
///   constructed bond and used by valuation and portfolio lookups.
/// * `schedule_json` - JSON representation of a canonical
///   `CashFlowSchedule`; it must contain a valid, internally consistent
///   principal and coupon schedule.
/// * `discount_curve_id` - Market-context key of the discount curve used to
///   value the bond's cashflows.
/// * `quoted_clean` - Optional clean market price expressed as a percentage of
///   par. `None` leaves the bond without an explicit clean-price override.
///
/// # Errors
///
/// Returns an error if the schedule JSON is invalid, bond construction fails, or
/// the tagged instrument cannot be serialized.
pub fn bond_from_cashflows_json(
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> finstack_quant_core::Result<String> {
    let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(schedule_json).map_err(|err| {
            finstack_quant_core::Error::Validation(format!("invalid cashflow schedule JSON: {err}"))
        })?;
    finstack_quant_cashflows::validate_cashflow_schedule(&schedule)?;
    let bond =
        super::Bond::from_cashflows(instrument_id, schedule, discount_curve_id, quoted_clean)?;
    let instrument = crate::instruments::InstrumentJson::Bond(bond);
    serde_json::to_string(&instrument).map_err(|err| {
        finstack_quant_core::Error::Validation(format!(
            "failed to serialize bond instrument JSON: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::bond_from_cashflows_json;

    #[test]
    fn builds_tagged_bond_from_raw_schedule_json() {
        let spec = serde_json::json!({
            "notional": {
                "initial": {"amount": "1000000", "currency": "USD"},
                "amort": "None"
            },
            "issue": "2024-08-31",
            "maturity": "2025-08-31",
            "coupon_program": [{
                "kind": "fixed",
                "spec": {
                    "coupon_type": "Cash",
                    "rate": "0.06",
                    "freq": {"count": 12, "unit": "months"},
                    "dc": "Thirty360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None",
                    "end_of_month": false,
                    "payment_lag_days": 0
                }
            }]
        })
        .to_string();
        let schedule = finstack_quant_cashflows::build_cashflow_schedule_json(&spec, None)
            .expect("raw schedule should build");

        let instrument = bond_from_cashflows_json("CUSTOM-CF", &schedule, "USD-OIS", Some(99.0))
            .expect("bond JSON should build from a raw schedule");
        let instrument: serde_json::Value =
            serde_json::from_str(&instrument).expect("instrument JSON should parse");

        assert_eq!(instrument["type"], "bond");
        assert_eq!(instrument["spec"]["id"], "CUSTOM-CF");
    }
}
