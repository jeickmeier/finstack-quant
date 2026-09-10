//! Long-format [`TableEnvelope`] exports for the regulatory sensitivity containers.
//!
//! [`FrtbSensitivities`] and [`SimmSensitivities`] store their inputs as many
//! `HashMap<key tuple, f64>` buckets. Exporting one column per bucket would
//! give a different schema for every portfolio, so both export the same
//! six-column long format instead: `risk_class`, `bucket`, `tenor`, `issuer`,
//! `kind`, `amount`.
//!
//! Rows are sorted by `(risk_class, kind, issuer, bucket, tenor)` before the
//! table is built, because `HashMap` iteration order is not stable across
//! runs; two exports of the same container are therefore identical.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_core::wire::serde_label;
use finstack_quant_core::Result;

use crate::regulatory::frtb::FrtbSensitivities;
use crate::types::SimmSensitivities;

/// One long-format sensitivity row.
#[derive(Debug, Clone, PartialEq)]
struct SensitivityRow {
    risk_class: String,
    bucket: Option<String>,
    tenor: Option<String>,
    expiry_tenor: Option<String>,
    issuer: Option<String>,
    kind: &'static str,
    amount: f64,
}

/// Accumulator that emits sensitivity rows in a deterministic order.
#[derive(Debug, Default)]
struct SensitivityRows {
    rows: Vec<SensitivityRow>,
}

impl SensitivityRows {
    /// Record one sensitivity value.
    ///
    /// `bucket`, `tenor` and `issuer` are `None` for risk classes that do not
    /// carry that axis; they surface as nulls rather than empty strings.
    fn push(
        &mut self,
        risk_class: impl Into<String>,
        kind: &'static str,
        issuer: Option<String>,
        bucket: Option<String>,
        tenor: Option<String>,
        amount: f64,
    ) {
        self.rows.push(SensitivityRow {
            risk_class: risk_class.into(),
            bucket,
            tenor,
            expiry_tenor: None,
            issuer,
            kind,
            amount,
        });
    }

    /// Record both halves of a curvature `(cvr_up, cvr_down)` pair.
    fn push_curvature(
        &mut self,
        risk_class: &str,
        issuer: Option<String>,
        bucket: Option<String>,
        (cvr_up, cvr_down): (f64, f64),
    ) {
        self.push(
            risk_class,
            "curvature_up",
            issuer.clone(),
            bucket.clone(),
            None,
            cvr_up,
        );
        self.push(risk_class, "curvature_down", issuer, bucket, None, cvr_down);
    }

    /// Sort the rows and build the six-column table.
    fn into_table(mut self, with_expiry: bool) -> Result<TableEnvelope> {
        self.rows.sort_by(|left, right| {
            (
                &left.risk_class,
                left.kind,
                &left.issuer,
                &left.bucket,
                &left.tenor,
                &left.expiry_tenor,
            )
                .cmp(&(
                    &right.risk_class,
                    right.kind,
                    &right.issuer,
                    &right.bucket,
                    &right.tenor,
                    &right.expiry_tenor,
                ))
                .then_with(|| left.amount.total_cmp(&right.amount))
        });
        let n = self.rows.len();
        let mut risk_class = Vec::with_capacity(n);
        let mut bucket = Vec::with_capacity(n);
        let mut tenor = Vec::with_capacity(n);
        let mut expiry_tenor = Vec::with_capacity(n);
        let mut issuer = Vec::with_capacity(n);
        let mut kind = Vec::with_capacity(n);
        let mut amount = Vec::with_capacity(n);
        for row in self.rows {
            risk_class.push(row.risk_class);
            bucket.push(row.bucket);
            tenor.push(row.tenor);
            expiry_tenor.push(row.expiry_tenor);
            issuer.push(row.issuer);
            kind.push(row.kind.to_string());
            amount.push(row.amount);
        }
        let mut columns = vec![
            TableColumn::new("risk_class", TableColumnData::String(risk_class))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("bucket", TableColumnData::NullableString(bucket))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("tenor", TableColumnData::NullableString(tenor))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("issuer", TableColumnData::NullableString(issuer))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("kind", TableColumnData::String(kind))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("amount", TableColumnData::Float64(amount))
                .with_role(TableColumnRole::Measure),
        ];
        if with_expiry {
            columns.push(
                TableColumn::new(
                    "expiry_tenor",
                    TableColumnData::NullableString(expiry_tenor),
                )
                .with_role(TableColumnRole::Dimension),
            );
        }
        TableEnvelope::new(columns)
    }
}

/// Render a bucket index as the string form used by the long-format frames.
fn bucket_label(bucket: u8) -> Option<String> {
    Some(bucket.to_string())
}

/// Render a currency pair as a single `issuer` value (e.g. `"EUR/USD"`).
fn pair_label(ccy1: Currency, ccy2: Currency) -> Option<String> {
    Some(format!("{ccy1}/{ccy2}"))
}

impl FrtbSensitivities {
    /// Export every sensitivity as one long-format row.
    ///
    /// Columns: `risk_class`, `bucket`, `tenor`, `issuer`, `kind`, `amount`.
    ///
    /// `risk_class` is `"girr"`, `"csr_non_sec"`, `"csr_sec_ctp"`,
    /// `"csr_sec_non_ctp"`, `"equity"`, `"commodity"`, `"fx"`, `"drc"` or
    /// `"rrao"`. `kind` is `"delta"`, `"vega"`, `"inflation_delta"`,
    /// `"xccy_basis_delta"`, `"curvature_up"` / `"curvature_down"` (one row
    /// per half of a curvature pair), `"jtd"` (DRC) or
    /// `"exotic_notional"` / `"other_notional"` (RRAO).
    ///
    /// `issuer` carries the currency (GIRR), issuer / underlier / commodity
    /// name, currency pair (`"EUR/USD"` for FX), DRC issuer or RRAO
    /// instrument id; `bucket` is the numeric bucket (or DRC rating bucket)
    /// where the risk class has one; `tenor` is the delta tenor, vega
    /// maturity (`"option_maturity/underlying_tenor"` for GIRR vega) or null.
    ///
    /// Amounts are signed sensitivities in `base_currency` in the caller's
    /// input convention. Rows are sorted by
    /// `(risk_class, kind, issuer, bucket, tenor)` so two exports of the same
    /// container are identical; an empty container still yields all six
    /// columns.
    ///
    /// # Errors
    ///
    /// Returns an error only if the column lengths disagree, which cannot
    /// happen for rows built here.
    pub fn to_table(&self) -> Result<TableEnvelope> {
        let mut rows = SensitivityRows::default();

        for ((currency, tenor), amount) in &self.girr_delta {
            rows.push(
                "girr",
                "delta",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            );
        }
        for (currency, amount) in &self.girr_inflation_delta {
            rows.push(
                "girr",
                "inflation_delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            );
        }
        for (currency, amount) in &self.girr_xccy_basis_delta {
            rows.push(
                "girr",
                "xccy_basis_delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            );
        }
        for ((currency, option_maturity, underlying_tenor), amount) in &self.girr_vega {
            rows.push(
                "girr",
                "vega",
                Some(currency.to_string()),
                None,
                Some(format!("{option_maturity}/{underlying_tenor}")),
                *amount,
            );
        }
        for (currency, pair) in &self.girr_curvature {
            rows.push_curvature("girr", Some(currency.to_string()), None, *pair);
        }

        for (label, delta, vega, curvature) in [
            (
                "csr_non_sec",
                &self.csr_nonsec_delta,
                &self.csr_nonsec_vega,
                &self.csr_nonsec_curvature,
            ),
            (
                "csr_sec_ctp",
                &self.csr_sec_ctp_delta,
                &self.csr_sec_ctp_vega,
                &self.csr_sec_ctp_curvature,
            ),
            (
                "csr_sec_non_ctp",
                &self.csr_sec_nonctp_delta,
                &self.csr_sec_nonctp_vega,
                &self.csr_sec_nonctp_curvature,
            ),
        ] {
            for ((issuer, bucket, tenor, basis), amount) in delta {
                rows.push(
                    label,
                    "delta",
                    Some(issuer.clone()),
                    bucket_label(*bucket),
                    Some(format!("{tenor}/{basis}")),
                    *amount,
                );
            }
            for ((issuer, bucket, maturity), amount) in vega {
                rows.push(
                    label,
                    "vega",
                    Some(issuer.clone()),
                    bucket_label(*bucket),
                    Some(maturity.clone()),
                    *amount,
                );
            }
            for ((issuer, bucket), pair) in curvature {
                rows.push_curvature(label, Some(issuer.clone()), bucket_label(*bucket), *pair);
            }
        }

        for ((underlier, bucket), amount) in &self.equity_delta {
            rows.push(
                "equity",
                "delta",
                Some(underlier.clone()),
                bucket_label(*bucket),
                None,
                *amount,
            );
        }
        for ((underlier, bucket), amount) in &self.equity_repo_delta {
            rows.push(
                "equity",
                "repo_delta",
                Some(underlier.clone()),
                bucket_label(*bucket),
                None,
                *amount,
            );
        }
        for ((underlier, bucket, maturity), amount) in &self.equity_vega {
            rows.push(
                "equity",
                "vega",
                Some(underlier.clone()),
                bucket_label(*bucket),
                Some(maturity.clone()),
                *amount,
            );
        }
        for ((underlier, bucket), pair) in &self.equity_curvature {
            rows.push_curvature(
                "equity",
                Some(underlier.clone()),
                bucket_label(*bucket),
                *pair,
            );
        }

        for ((name, bucket, tenor, basis), amount) in &self.commodity_delta {
            rows.push(
                "commodity",
                "delta",
                Some(name.clone()),
                bucket_label(*bucket),
                Some(format!("{tenor}/{basis}")),
                *amount,
            );
        }
        for ((name, bucket, maturity), amount) in &self.commodity_vega {
            rows.push(
                "commodity",
                "vega",
                Some(name.clone()),
                bucket_label(*bucket),
                Some(maturity.clone()),
                *amount,
            );
        }
        for ((name, bucket), pair) in &self.commodity_curvature {
            rows.push_curvature(
                "commodity",
                Some(name.clone()),
                bucket_label(*bucket),
                *pair,
            );
        }

        for ((ccy1, ccy2), amount) in &self.fx_delta {
            rows.push("fx", "delta", pair_label(*ccy1, *ccy2), None, None, *amount);
        }
        for ((ccy1, ccy2, maturity), amount) in &self.fx_vega {
            rows.push(
                "fx",
                "vega",
                pair_label(*ccy1, *ccy2),
                None,
                Some(maturity.clone()),
                *amount,
            );
        }
        for ((ccy1, ccy2), pair) in &self.fx_curvature {
            rows.push_curvature("fx", pair_label(*ccy1, *ccy2), None, *pair);
        }

        for position in &self.drc_positions {
            rows.push(
                "drc",
                "jtd",
                Some(position.issuer.clone()),
                bucket_label(position.rating_bucket),
                None,
                position.jtd_amount,
            );
        }
        for position in &self.rrao_exotic_notionals {
            let kind = if position.is_exotic {
                "exotic_notional"
            } else {
                "other_notional"
            };
            rows.push(
                "rrao",
                kind,
                Some(position.instrument_id.clone()),
                None,
                None,
                position.notional,
            );
        }

        rows.into_table(false)
    }
}

impl SimmSensitivities {
    /// Export every sensitivity as one long-format row.
    ///
    /// Columns: `risk_class`, `bucket`, `tenor`, `issuer`, `kind`, `amount`.
    ///
    /// `risk_class` is the SIMM risk class (`"interest_rate"`,
    /// `"credit_qualifying"`, `"credit_non_qualifying"`, `"equity"`,
    /// `"commodity"`, `"fx"`); `kind` is `"delta"`, `"vega"` or
    /// `"curvature"`. `issuer` carries the currency (interest rate, FX
    /// delta), currency pair (`"EUR/USD"` for FX vega), credit name or
    /// equity underlier; `bucket` holds the SIMM credit sector for qualifying
    /// credit deltas (e.g. `"sovereign"`) and the commodity bucket label;
    /// `tenor` is the SIMM tenor bucket (`"2W"` … `"30Y"`) where the risk
    /// class has one.
    ///
    /// Amounts are signed sensitivities in `base_currency` in the caller's
    /// input convention — SIMM does not re-scale these on ingest. Rows are
    /// sorted by `(risk_class, kind, issuer, bucket, tenor)`; an empty
    /// container still yields all six columns.
    ///
    /// # Errors
    ///
    /// Returns an error if a credit sector or risk class has no serde label
    /// (cannot happen for the shipped enums) or if column lengths disagree.
    pub fn to_table(&self) -> Result<TableEnvelope> {
        let mut rows = SensitivityRows::default();

        for ((currency, tenor), amount) in &self.ir_delta {
            rows.push(
                "interest_rate",
                "delta",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            );
        }
        for ((currency, tenor), amount) in &self.ir_vega {
            rows.push(
                "interest_rate",
                "vega",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            );
        }
        for ((sector, name, tenor), amount) in &self.credit_qualifying_delta {
            rows.push(
                "credit_qualifying",
                "delta",
                Some(name.clone()),
                Some(serde_label(sector)?),
                Some(tenor.clone()),
                *amount,
            );
        }
        for ((name, tenor), amount) in &self.credit_non_qualifying_delta {
            rows.push(
                "credit_non_qualifying",
                "delta",
                Some(name.clone()),
                None,
                Some(tenor.clone()),
                *amount,
            );
        }
        for ((sector, name, tenor), amount) in &self.credit_qualifying_vega {
            rows.push(
                "credit_qualifying",
                "vega",
                Some(name.clone()),
                Some(serde_label(sector)?),
                Some(tenor.clone()),
                *amount,
            );
        }
        for ((name, tenor), amount) in &self.credit_non_qualifying_vega {
            rows.push(
                "credit_non_qualifying",
                "vega",
                Some(name.clone()),
                None,
                Some(tenor.clone()),
                *amount,
            );
        }
        for (underlier, amount) in &self.equity_delta {
            rows.push(
                "equity",
                "delta",
                Some(underlier.clone()),
                None,
                None,
                *amount,
            );
        }
        for (underlier, amount) in &self.equity_vega {
            rows.push(
                "equity",
                "vega",
                Some(underlier.clone()),
                None,
                None,
                *amount,
            );
        }
        for (currency, amount) in &self.fx_delta {
            rows.push(
                "fx",
                "delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            );
        }
        for ((ccy1, ccy2), amount) in &self.fx_vega {
            rows.push("fx", "vega", pair_label(*ccy1, *ccy2), None, None, *amount);
        }
        for (bucket, amount) in &self.commodity_delta {
            rows.push(
                "commodity",
                "delta",
                None,
                Some(bucket.clone()),
                None,
                *amount,
            );
        }
        for (bucket, amount) in &self.commodity_vega {
            rows.push(
                "commodity",
                "vega",
                None,
                Some(bucket.clone()),
                None,
                *amount,
            );
        }
        for input in &self.curvature {
            rows.rows.push(SensitivityRow {
                risk_class: serde_label(&input.risk_class)?,
                kind: "curvature",
                issuer: Some(input.factor.clone()),
                bucket: Some(input.bucket.clone()),
                tenor: input.risk_tenor.clone(),
                expiry_tenor: Some(input.expiry_tenor.clone()),
                amount: input.volatility_weighted_vega,
            });
        }

        rows.into_table(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frtb_table_keeps_schema() {
        let sens = FrtbSensitivities::new(Currency::USD);
        let table = sens.to_table().expect("table");
        let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["risk_class", "bucket", "tenor", "issuer", "kind", "amount"]
        );
        assert!(table.is_empty());
    }

    #[test]
    fn frtb_rows_are_sorted_and_curvature_is_split() {
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.girr_delta
            .insert((Currency::USD, "5Y".to_string()), 100.0);
        sens.girr_delta
            .insert((Currency::EUR, "2Y".to_string()), 50.0);
        sens.equity_curvature
            .insert(("ACME".to_string(), 3), (7.0, -4.0));
        let table = sens.to_table().expect("table");
        let kind = table
            .column("kind")
            .and_then(|c| c.as_strings())
            .expect("kind");
        let issuer = table
            .column("issuer")
            .and_then(|c| c.as_nullable_strings())
            .expect("issuer");
        let amount = table
            .column("amount")
            .and_then(|c| c.as_f64())
            .expect("amount");
        assert_eq!(kind, ["curvature_down", "curvature_up", "delta", "delta"]);
        assert_eq!(issuer[2].as_deref(), Some("EUR"));
        assert_eq!(issuer[3].as_deref(), Some("USD"));
        assert_eq!(amount, [-4.0, 7.0, 50.0, 100.0]);
    }
}
