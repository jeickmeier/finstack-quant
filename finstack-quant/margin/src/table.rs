//! Long-format sensitivity rows and [`TableEnvelope`] exports for the
//! regulatory sensitivity containers.
//!
//! [`FrtbSensitivities`] and [`SimmSensitivities`] store their inputs as many
//! `HashMap<key tuple, f64>` buckets. Exporting one column per bucket would
//! give a different schema for every portfolio, so both export the same long
//! format instead: one [`SensitivityRow`] per populated bucket, with the
//! columns `risk_class`, `bucket`, `tenor`, `issuer`, `kind`, `amount`. FRTB
//! adds the default-risk (DRC) columns `sector`, `seniority`, `asset_type`,
//! `maturity_years` and `pnl_adjustment`; SIMM adds `expiry_tenor` for
//! curvature rows.
//!
//! The encoding and its inverse live together: `to_rows` / `to_table` emit
//! the rows and `from_rows` reads them back, so a row set exported by one
//! container rebuilds an equivalent container.
//!
//! Rows are sorted by their key columns before the table is built, because
//! `HashMap` iteration order is not stable across runs; two exports of the
//! same container are therefore identical.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_margin::regulatory::frtb::FrtbSensitivities;
//!
//! let mut sens = FrtbSensitivities::new(Currency::USD);
//! sens.add_girr_delta(Currency::USD, "5Y", 25_000.0);
//! sens.add_equity_curvature("ACME", 1, 70.0, -60.0);
//!
//! let rows = sens.to_rows()?;
//! let restored = FrtbSensitivities::from_rows(Currency::USD, &rows)?;
//! assert_eq!(restored, sens);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use std::cmp::Ordering;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_core::wire::{serde_label, serde_parse};
use finstack_quant_core::{Error, Result};

use crate::regulatory::frtb::{DrcPosition, FrtbSensitivities};
use crate::types::{SimmCurvatureSensitivity, SimmSensitivities};

/// One long-format sensitivity row.
///
/// The row is the unit of the long format shared by
/// [`FrtbSensitivities::to_rows`] / [`FrtbSensitivities::from_rows`] and
/// [`SimmSensitivities::to_rows`] / [`SimmSensitivities::from_rows`]. Optional
/// fields are `None` for risk classes that do not carry that axis; they
/// surface as nulls in the exported table rather than empty strings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SensitivityRow {
    /// Risk-class label (`"girr"`, `"equity"`, … for FRTB; the SIMM risk
    /// class serde label for SIMM).
    pub risk_class: String,
    /// Bucket label: the numeric FRTB bucket or DRC rating bucket, the SIMM
    /// qualifying-credit sector, the SIMM commodity bucket or the SIMM
    /// curvature bucket.
    pub bucket: Option<String>,
    /// Tenor label; FRTB delta rows with a basis carry `"tenor/basis"` and
    /// GIRR vega rows carry `"option_maturity/underlying_tenor"`.
    pub tenor: Option<String>,
    /// SIMM curvature option-expiry tenor; `None` on every other row.
    pub expiry_tenor: Option<String>,
    /// Currency, issuer, underlier, commodity name, `"CCY1/CCY2"` currency
    /// pair, DRC issuer or RRAO instrument id, depending on the risk class.
    pub issuer: Option<String>,
    /// Sensitivity kind (`"delta"`, `"vega"`, `"curvature_up"`, `"jtd"`, …).
    pub kind: String,
    /// DRC sector serde label (`"corporate"`, …); `None` on non-DRC rows.
    pub sector: Option<String>,
    /// DRC seniority serde label (`"senior_unsecured"`, …); `None` on non-DRC rows.
    pub seniority: Option<String>,
    /// DRC asset-type serde label (`"corporate"`, `"sovereign"`, …); `None` on
    /// non-DRC rows.
    pub asset_type: Option<String>,
    /// DRC residual maturity in years; `None` on non-DRC rows.
    pub maturity_years: Option<f64>,
    /// DRC mark-to-market adjustment in base currency (MAR22.9); `None` on
    /// non-DRC rows, and read back as `0.0` when absent.
    pub pnl_adjustment: Option<f64>,
    /// Signed sensitivity amount in the container's base currency (DRC: the
    /// signed JTD notional; RRAO: the gross notional).
    pub amount: f64,
}

impl SensitivityRow {
    /// A row with the common columns set and every optional column empty.
    fn new(
        risk_class: impl Into<String>,
        kind: &str,
        issuer: Option<String>,
        bucket: Option<String>,
        tenor: Option<String>,
        amount: f64,
    ) -> Self {
        Self {
            risk_class: risk_class.into(),
            bucket,
            tenor,
            issuer,
            kind: kind.to_string(),
            amount,
            ..Self::default()
        }
    }

    /// The two rows of a curvature `(cvr_up, cvr_down)` pair.
    fn curvature(
        risk_class: &str,
        issuer: Option<String>,
        bucket: Option<String>,
        (cvr_up, cvr_down): (f64, f64),
    ) -> [Self; 2] {
        [
            Self::new(
                risk_class,
                "curvature_up",
                issuer.clone(),
                bucket.clone(),
                None,
                cvr_up,
            ),
            Self::new(risk_class, "curvature_down", issuer, bucket, None, cvr_down),
        ]
    }

    /// Deterministic export order: key columns first, then the numbers.
    fn order(&self, other: &Self) -> Ordering {
        (
            &self.risk_class,
            &self.kind,
            &self.issuer,
            &self.bucket,
            &self.tenor,
            &self.expiry_tenor,
            &self.sector,
            &self.seniority,
            &self.asset_type,
        )
            .cmp(&(
                &other.risk_class,
                &other.kind,
                &other.issuer,
                &other.bucket,
                &other.tenor,
                &other.expiry_tenor,
                &other.sector,
                &other.seniority,
                &other.asset_type,
            ))
            .then_with(|| cmp_opt_f64(self.maturity_years, other.maturity_years))
            .then_with(|| cmp_opt_f64(self.pnl_adjustment, other.pnl_adjustment))
            .then_with(|| self.amount.total_cmp(&other.amount))
    }

    /// Error for a row whose `(risk_class, kind)` the container cannot read.
    fn unsupported(&self, container: &str) -> Error {
        Error::Validation(format!(
            "unsupported {container} sensitivity row risk_class={:?} kind={:?}",
            self.risk_class, self.kind
        ))
    }

    /// A required optional column, or a validation error naming it.
    fn need<'a>(&self, value: &'a Option<String>, column: &str) -> Result<&'a str> {
        value.as_deref().ok_or_else(|| {
            Error::Validation(format!(
                "{} {} sensitivity row needs a '{column}' value",
                self.risk_class, self.kind
            ))
        })
    }

    /// The `issuer` column, required.
    fn issuer(&self) -> Result<&str> {
        self.need(&self.issuer, "issuer")
    }

    /// The `tenor` column, required.
    fn tenor(&self) -> Result<&str> {
        self.need(&self.tenor, "tenor")
    }

    /// The `bucket` column as a 1-based numeric bucket index.
    fn bucket_index(&self) -> Result<u8> {
        let text = self.need(&self.bucket, "bucket")?;
        text.trim().parse::<u8>().map_err(|_| {
            Error::Validation(format!(
                "{} {} sensitivity row bucket must be a bucket index, got {text:?}",
                self.risk_class, self.kind
            ))
        })
    }

    /// The `issuer` column as a currency code.
    fn issuer_currency(&self) -> Result<Currency> {
        parse_currency(self.issuer()?)
    }

    /// The `issuer` column as a `"CCY1/CCY2"` currency pair.
    fn issuer_pair(&self) -> Result<(Currency, Currency)> {
        let (ccy1, ccy2) = split_pair(self.issuer()?, "issuer")?;
        Ok((parse_currency(ccy1)?, parse_currency(ccy2)?))
    }

    /// The `tenor` column as an `"A/B"` pair.
    fn tenor_pair(&self) -> Result<(&str, &str)> {
        split_pair(self.tenor()?, "tenor")
    }

    /// `(cvr_up, cvr_down)` contribution of a curvature half, or `None` when
    /// the row is not a curvature half.
    fn curvature_half(&self) -> Option<(f64, f64)> {
        match self.kind.as_str() {
            "curvature_up" => Some((self.amount, 0.0)),
            "curvature_down" => Some((0.0, self.amount)),
            _ => None,
        }
    }
}

/// Total order on optional floats (`None` first).
fn cmp_opt_f64(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (a, b) => a.is_some().cmp(&b.is_some()),
    }
}

/// Parse an ISO-4217 currency code.
fn parse_currency(code: &str) -> Result<Currency> {
    code.parse::<Currency>()
        .map_err(|_| Error::Validation(format!("unknown currency code {code:?}")))
}

/// Split an `"A/B"` pair label into its two non-empty halves.
fn split_pair<'a>(label: &'a str, column: &str) -> Result<(&'a str, &'a str)> {
    match label.split_once('/') {
        Some((a, b)) if !a.is_empty() && !b.is_empty() => Ok((a, b)),
        _ => Err(Error::Validation(format!(
            "sensitivity row {column} must be an 'A/B' pair, got {label:?}"
        ))),
    }
}

/// Render a bucket index as the string form used by the long-format rows.
fn bucket_label(bucket: u8) -> Option<String> {
    Some(bucket.to_string())
}

/// Render a currency pair as a single `issuer` value (e.g. `"EUR/USD"`).
fn pair_label(ccy1: Currency, ccy2: Currency) -> Option<String> {
    Some(format!("{ccy1}/{ccy2}"))
}

/// Which optional column group a table carries.
#[derive(Clone, Copy)]
enum Layout {
    /// FRTB: the five DRC columns.
    Frtb,
    /// SIMM: the `expiry_tenor` column.
    Simm,
}

/// Sort rows into the deterministic export order.
fn sorted(mut rows: Vec<SensitivityRow>) -> Vec<SensitivityRow> {
    rows.sort_by(SensitivityRow::order);
    rows
}

/// Build the long-format table from already-sorted rows.
fn rows_to_table(rows: Vec<SensitivityRow>, layout: Layout) -> Result<TableEnvelope> {
    let n = rows.len();
    let mut risk_class = Vec::with_capacity(n);
    let mut bucket = Vec::with_capacity(n);
    let mut tenor = Vec::with_capacity(n);
    let mut expiry_tenor = Vec::with_capacity(n);
    let mut issuer = Vec::with_capacity(n);
    let mut kind = Vec::with_capacity(n);
    let mut amount = Vec::with_capacity(n);
    let mut sector = Vec::with_capacity(n);
    let mut seniority = Vec::with_capacity(n);
    let mut asset_type = Vec::with_capacity(n);
    let mut maturity_years = Vec::with_capacity(n);
    let mut pnl_adjustment = Vec::with_capacity(n);
    for row in rows {
        risk_class.push(row.risk_class);
        bucket.push(row.bucket);
        tenor.push(row.tenor);
        expiry_tenor.push(row.expiry_tenor);
        issuer.push(row.issuer);
        kind.push(row.kind);
        amount.push(row.amount);
        sector.push(row.sector);
        seniority.push(row.seniority);
        asset_type.push(row.asset_type);
        maturity_years.push(row.maturity_years);
        pnl_adjustment.push(row.pnl_adjustment);
    }
    let dimension = |name: &str, data: TableColumnData| {
        TableColumn::new(name, data).with_role(TableColumnRole::Dimension)
    };
    let mut columns = vec![
        dimension("risk_class", TableColumnData::String(risk_class)),
        dimension("bucket", TableColumnData::NullableString(bucket)),
        dimension("tenor", TableColumnData::NullableString(tenor)),
        dimension("issuer", TableColumnData::NullableString(issuer)),
        dimension("kind", TableColumnData::String(kind)),
        TableColumn::new("amount", TableColumnData::Float64(amount))
            .with_role(TableColumnRole::Measure),
    ];
    match layout {
        Layout::Frtb => {
            columns.push(dimension("sector", TableColumnData::NullableString(sector)));
            columns.push(dimension(
                "seniority",
                TableColumnData::NullableString(seniority),
            ));
            columns.push(dimension(
                "asset_type",
                TableColumnData::NullableString(asset_type),
            ));
            columns.push(
                TableColumn::new(
                    "maturity_years",
                    TableColumnData::NullableFloat64(maturity_years),
                )
                .with_role(TableColumnRole::Measure),
            );
            columns.push(
                TableColumn::new(
                    "pnl_adjustment",
                    TableColumnData::NullableFloat64(pnl_adjustment),
                )
                .with_role(TableColumnRole::Measure),
            );
        }
        Layout::Simm => {
            columns.push(dimension(
                "expiry_tenor",
                TableColumnData::NullableString(expiry_tenor),
            ));
        }
    }
    TableEnvelope::new(columns)
}

impl FrtbSensitivities {
    /// Export every sensitivity as one long-format row, sorted.
    ///
    /// `risk_class` is `"girr"`, `"csr_non_sec"`, `"csr_sec_ctp"`,
    /// `"csr_sec_non_ctp"`, `"equity"`, `"commodity"`, `"fx"`, `"drc"` or
    /// `"rrao"`. `kind` is `"delta"`, `"vega"`, `"inflation_delta"`,
    /// `"xccy_basis_delta"`, `"repo_delta"`, `"curvature_up"` /
    /// `"curvature_down"` (one row per half of a curvature pair), `"jtd"`
    /// (DRC) or `"exotic_notional"` / `"other_notional"` (RRAO).
    ///
    /// `issuer` carries the currency (GIRR), issuer / underlier / commodity
    /// name, currency pair (`"EUR/USD"` for FX), DRC issuer or RRAO
    /// instrument id; `bucket` is the numeric bucket (or DRC rating bucket)
    /// where the risk class has one; `tenor` is the delta tenor
    /// (`"tenor/basis"` for CSR and commodity deltas), vega maturity
    /// (`"option_maturity/underlying_tenor"` for GIRR vega) or null. DRC rows
    /// also carry `sector`, `seniority`, `asset_type`, `maturity_years` and
    /// `pnl_adjustment`, so [`Self::from_rows`] rebuilds every position.
    ///
    /// Amounts are signed sensitivities in `base_currency` in the caller's
    /// input convention.
    ///
    /// # Returns
    ///
    /// The rows in deterministic order, so two exports of the same container
    /// are identical.
    ///
    /// # Errors
    ///
    /// Returns an error if a DRC enum has no string serde label, which cannot
    /// happen for the shipped enums.
    pub fn to_rows(&self) -> Result<Vec<SensitivityRow>> {
        let mut rows = Vec::new();

        for ((currency, tenor), amount) in &self.girr_delta {
            rows.push(SensitivityRow::new(
                "girr",
                "delta",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            ));
        }
        for (currency, amount) in &self.girr_inflation_delta {
            rows.push(SensitivityRow::new(
                "girr",
                "inflation_delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            ));
        }
        for (currency, amount) in &self.girr_xccy_basis_delta {
            rows.push(SensitivityRow::new(
                "girr",
                "xccy_basis_delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            ));
        }
        for ((currency, option_maturity, underlying_tenor), amount) in &self.girr_vega {
            rows.push(SensitivityRow::new(
                "girr",
                "vega",
                Some(currency.to_string()),
                None,
                Some(format!("{option_maturity}/{underlying_tenor}")),
                *amount,
            ));
        }
        for (currency, pair) in &self.girr_curvature {
            rows.extend(SensitivityRow::curvature(
                "girr",
                Some(currency.to_string()),
                None,
                *pair,
            ));
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
                rows.push(SensitivityRow::new(
                    label,
                    "delta",
                    Some(issuer.clone()),
                    bucket_label(*bucket),
                    Some(format!("{tenor}/{basis}")),
                    *amount,
                ));
            }
            for ((issuer, bucket, maturity), amount) in vega {
                rows.push(SensitivityRow::new(
                    label,
                    "vega",
                    Some(issuer.clone()),
                    bucket_label(*bucket),
                    Some(maturity.clone()),
                    *amount,
                ));
            }
            for ((issuer, bucket), pair) in curvature {
                rows.extend(SensitivityRow::curvature(
                    label,
                    Some(issuer.clone()),
                    bucket_label(*bucket),
                    *pair,
                ));
            }
        }

        for ((underlier, bucket), amount) in &self.equity_delta {
            rows.push(SensitivityRow::new(
                "equity",
                "delta",
                Some(underlier.clone()),
                bucket_label(*bucket),
                None,
                *amount,
            ));
        }
        for ((underlier, bucket), amount) in &self.equity_repo_delta {
            rows.push(SensitivityRow::new(
                "equity",
                "repo_delta",
                Some(underlier.clone()),
                bucket_label(*bucket),
                None,
                *amount,
            ));
        }
        for ((underlier, bucket, maturity), amount) in &self.equity_vega {
            rows.push(SensitivityRow::new(
                "equity",
                "vega",
                Some(underlier.clone()),
                bucket_label(*bucket),
                Some(maturity.clone()),
                *amount,
            ));
        }
        for ((underlier, bucket), pair) in &self.equity_curvature {
            rows.extend(SensitivityRow::curvature(
                "equity",
                Some(underlier.clone()),
                bucket_label(*bucket),
                *pair,
            ));
        }

        for ((name, bucket, tenor, basis), amount) in &self.commodity_delta {
            rows.push(SensitivityRow::new(
                "commodity",
                "delta",
                Some(name.clone()),
                bucket_label(*bucket),
                Some(format!("{tenor}/{basis}")),
                *amount,
            ));
        }
        for ((name, bucket, maturity), amount) in &self.commodity_vega {
            rows.push(SensitivityRow::new(
                "commodity",
                "vega",
                Some(name.clone()),
                bucket_label(*bucket),
                Some(maturity.clone()),
                *amount,
            ));
        }
        for ((name, bucket), pair) in &self.commodity_curvature {
            rows.extend(SensitivityRow::curvature(
                "commodity",
                Some(name.clone()),
                bucket_label(*bucket),
                *pair,
            ));
        }

        for ((ccy1, ccy2), amount) in &self.fx_delta {
            rows.push(SensitivityRow::new(
                "fx",
                "delta",
                pair_label(*ccy1, *ccy2),
                None,
                None,
                *amount,
            ));
        }
        for ((ccy1, ccy2, maturity), amount) in &self.fx_vega {
            rows.push(SensitivityRow::new(
                "fx",
                "vega",
                pair_label(*ccy1, *ccy2),
                None,
                Some(maturity.clone()),
                *amount,
            ));
        }
        for ((ccy1, ccy2), pair) in &self.fx_curvature {
            rows.extend(SensitivityRow::curvature(
                "fx",
                pair_label(*ccy1, *ccy2),
                None,
                *pair,
            ));
        }

        for position in &self.drc_positions {
            rows.push(SensitivityRow {
                sector: Some(serde_label(&position.sector)?),
                seniority: Some(serde_label(&position.seniority)?),
                asset_type: Some(serde_label(&position.asset_type)?),
                maturity_years: Some(position.maturity_years),
                pnl_adjustment: Some(position.pnl_adjustment),
                ..SensitivityRow::new(
                    "drc",
                    "jtd",
                    Some(position.issuer.clone()),
                    bucket_label(position.rating_bucket),
                    None,
                    position.jtd_amount,
                )
            });
        }
        for position in &self.rrao_exotic_notionals {
            let kind = if position.is_exotic {
                "exotic_notional"
            } else {
                "other_notional"
            };
            rows.push(SensitivityRow::new(
                "rrao",
                kind,
                Some(position.instrument_id.clone()),
                None,
                None,
                position.notional,
            ));
        }

        Ok(sorted(rows))
    }

    /// Rebuild a container from long-format rows (the inverse of
    /// [`Self::to_rows`]).
    ///
    /// Rows with the same key accumulate through the matching `add_*`
    /// adder; the `curvature_up` and `curvature_down` halves of a pair
    /// recombine into one `(cvr_up, cvr_down)` entry. Each `drc` row becomes
    /// one [`DrcPosition`] (`pnl_adjustment` reads as `0.0` when absent, the
    /// wire default) and each `rrao` row one RRAO position.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Reporting currency in which every row `amount` is
    ///   expressed; the rows themselves do not carry it.
    /// * `rows` - Long-format rows encoded as [`Self::to_rows`] documents, in
    ///   any order.
    ///
    /// # Returns
    ///
    /// A container equal to the one that exported `rows` (DRC and RRAO
    /// positions in row order).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an unknown `(risk_class, kind)`, a
    /// missing column the row needs, a non-numeric bucket, an unknown
    /// currency code, a malformed `"A/B"` pair, or an unknown DRC label.
    pub fn from_rows(base_currency: Currency, rows: &[SensitivityRow]) -> Result<Self> {
        let mut sens = Self::new(base_currency);
        for row in rows {
            sens.add_row(row)?;
        }
        Ok(sens)
    }

    /// Add one long-format row through the matching adder.
    fn add_row(&mut self, row: &SensitivityRow) -> Result<()> {
        let amount = row.amount;
        let curvature = row.curvature_half();
        let unsupported = || Err(row.unsupported("FRTB"));
        match row.risk_class.as_str() {
            "girr" => {
                let ccy = row.issuer_currency()?;
                match (row.kind.as_str(), curvature) {
                    (_, Some((up, down))) => self.add_girr_curvature(ccy, up, down),
                    ("delta", _) => self.add_girr_delta(ccy, row.tenor()?, amount),
                    ("inflation_delta", _) => self.add_girr_inflation_delta(ccy, amount),
                    ("xccy_basis_delta", _) => self.add_girr_xccy_basis_delta(ccy, amount),
                    ("vega", _) => {
                        let (option_maturity, underlying_tenor) = row.tenor_pair()?;
                        self.add_girr_vega(ccy, option_maturity, underlying_tenor, amount);
                    }
                    _ => return unsupported(),
                }
            }
            "csr_non_sec" | "csr_sec_ctp" | "csr_sec_non_ctp" | "commodity" => {
                let name = row.issuer()?;
                let bucket = row.bucket_index()?;
                match (row.risk_class.as_str(), row.kind.as_str(), curvature) {
                    ("csr_non_sec", _, Some((up, down))) => {
                        self.add_csr_nonsec_curvature(name, bucket, up, down);
                    }
                    ("csr_sec_ctp", _, Some((up, down))) => {
                        self.add_csr_sec_ctp_curvature(name, bucket, up, down);
                    }
                    ("csr_sec_non_ctp", _, Some((up, down))) => {
                        self.add_csr_sec_nonctp_curvature(name, bucket, up, down);
                    }
                    ("commodity", _, Some((up, down))) => {
                        self.add_commodity_curvature(name, bucket, up, down);
                    }
                    ("csr_non_sec", "delta", _) => {
                        let (tenor, basis) = row.tenor_pair()?;
                        self.add_csr_nonsec_delta(name, bucket, tenor, basis, amount);
                    }
                    ("csr_sec_ctp", "delta", _) => {
                        let (tenor, basis) = row.tenor_pair()?;
                        self.add_csr_sec_ctp_delta(name, bucket, tenor, basis, amount);
                    }
                    ("csr_sec_non_ctp", "delta", _) => {
                        let (tenor, basis) = row.tenor_pair()?;
                        self.add_csr_sec_nonctp_delta(name, bucket, tenor, basis, amount);
                    }
                    ("commodity", "delta", _) => {
                        let (tenor, basis) = row.tenor_pair()?;
                        self.add_commodity_delta(name, bucket, tenor, basis, amount);
                    }
                    ("csr_non_sec", "vega", _) => {
                        self.add_csr_nonsec_vega(name, bucket, row.tenor()?, amount);
                    }
                    ("csr_sec_ctp", "vega", _) => {
                        self.add_csr_sec_ctp_vega(name, bucket, row.tenor()?, amount);
                    }
                    ("csr_sec_non_ctp", "vega", _) => {
                        self.add_csr_sec_nonctp_vega(name, bucket, row.tenor()?, amount);
                    }
                    ("commodity", "vega", _) => {
                        self.add_commodity_vega(name, bucket, row.tenor()?, amount);
                    }
                    _ => return unsupported(),
                }
            }
            "equity" => {
                let underlier = row.issuer()?;
                let bucket = row.bucket_index()?;
                match (row.kind.as_str(), curvature) {
                    (_, Some((up, down))) => self.add_equity_curvature(underlier, bucket, up, down),
                    ("delta", _) => self.add_equity_delta(underlier, bucket, amount),
                    ("repo_delta", _) => self.add_equity_repo_delta(underlier, bucket, amount),
                    ("vega", _) => self.add_equity_vega(underlier, bucket, row.tenor()?, amount),
                    _ => return unsupported(),
                }
            }
            "fx" => {
                let (ccy1, ccy2) = row.issuer_pair()?;
                match (row.kind.as_str(), curvature) {
                    (_, Some((up, down))) => self.add_fx_curvature(ccy1, ccy2, up, down),
                    ("delta", _) => self.add_fx_delta(ccy1, ccy2, amount),
                    ("vega", _) => self.add_fx_vega(ccy1, ccy2, row.tenor()?, amount),
                    _ => return unsupported(),
                }
            }
            "drc" if row.kind == "jtd" => {
                let maturity_years = row.maturity_years.ok_or_else(|| {
                    Error::Validation(format!(
                        "drc jtd sensitivity row for {:?} needs a 'maturity_years' value",
                        row.issuer
                    ))
                })?;
                self.add_drc_position(DrcPosition {
                    maturity_years,
                    issuer: row.issuer()?.to_string(),
                    jtd_amount: amount,
                    rating_bucket: row.bucket_index()?,
                    sector: serde_parse(row.need(&row.sector, "sector")?)?,
                    seniority: serde_parse(row.need(&row.seniority, "seniority")?)?,
                    asset_type: serde_parse(row.need(&row.asset_type, "asset_type")?)?,
                    pnl_adjustment: row.pnl_adjustment.unwrap_or(0.0),
                });
            }
            "rrao" => {
                let is_exotic = match row.kind.as_str() {
                    "exotic_notional" => true,
                    "other_notional" => false,
                    _ => return unsupported(),
                };
                self.add_rrao_position(row.issuer()?, amount, is_exotic);
            }
            _ => return unsupported(),
        }
        Ok(())
    }

    /// Export every sensitivity as one long-format table.
    ///
    /// Columns: `risk_class`, `bucket`, `tenor`, `issuer`, `kind`, `amount`,
    /// then the DRC columns `sector`, `seniority`, `asset_type`,
    /// `maturity_years`, `pnl_adjustment` (null on non-DRC rows). The rows
    /// are [`Self::to_rows`]; an empty container still yields every column.
    ///
    /// # Errors
    ///
    /// Returns an error only if [`Self::to_rows`] fails or the column lengths
    /// disagree, which cannot happen for rows built here.
    pub fn to_table(&self) -> Result<TableEnvelope> {
        rows_to_table(self.to_rows()?, Layout::Frtb)
    }
}

impl SimmSensitivities {
    /// Export every sensitivity as one long-format row, sorted.
    ///
    /// `risk_class` is the SIMM risk class (`"interest_rate"`,
    /// `"credit_qualifying"`, `"credit_non_qualifying"`, `"equity"`,
    /// `"commodity"`, `"fx"`); `kind` is `"delta"`, `"vega"` or
    /// `"curvature"`. `issuer` carries the currency (interest rate, FX
    /// delta), currency pair (`"EUR/USD"` for FX vega), credit name, equity
    /// underlier or curvature factor; `bucket` holds the SIMM credit sector
    /// for qualifying credit (e.g. `"sovereign"`), the commodity bucket label
    /// or the curvature bucket; `tenor` is the SIMM tenor bucket (`"2W"` …
    /// `"30Y"`) where the risk class has one; curvature rows also carry
    /// `expiry_tenor`.
    ///
    /// Amounts are signed sensitivities in `base_currency` in the caller's
    /// input convention — SIMM does not re-scale these on ingest.
    ///
    /// # Returns
    ///
    /// The rows in deterministic order, so two exports of the same container
    /// are identical.
    ///
    /// # Errors
    ///
    /// Returns an error if a credit sector or risk class has no string serde
    /// label, which cannot happen for the shipped enums.
    pub fn to_rows(&self) -> Result<Vec<SensitivityRow>> {
        let mut rows = Vec::new();

        for ((currency, tenor), amount) in &self.ir_delta {
            rows.push(SensitivityRow::new(
                "interest_rate",
                "delta",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            ));
        }
        for ((currency, tenor), amount) in &self.ir_vega {
            rows.push(SensitivityRow::new(
                "interest_rate",
                "vega",
                Some(currency.to_string()),
                None,
                Some(tenor.clone()),
                *amount,
            ));
        }
        for ((sector, name, tenor), amount) in &self.credit_qualifying_delta {
            rows.push(SensitivityRow::new(
                "credit_qualifying",
                "delta",
                Some(name.clone()),
                Some(serde_label(sector)?),
                Some(tenor.clone()),
                *amount,
            ));
        }
        for ((name, tenor), amount) in &self.credit_non_qualifying_delta {
            rows.push(SensitivityRow::new(
                "credit_non_qualifying",
                "delta",
                Some(name.clone()),
                None,
                Some(tenor.clone()),
                *amount,
            ));
        }
        for ((sector, name, tenor), amount) in &self.credit_qualifying_vega {
            rows.push(SensitivityRow::new(
                "credit_qualifying",
                "vega",
                Some(name.clone()),
                Some(serde_label(sector)?),
                Some(tenor.clone()),
                *amount,
            ));
        }
        for ((name, tenor), amount) in &self.credit_non_qualifying_vega {
            rows.push(SensitivityRow::new(
                "credit_non_qualifying",
                "vega",
                Some(name.clone()),
                None,
                Some(tenor.clone()),
                *amount,
            ));
        }
        for (underlier, amount) in &self.equity_delta {
            rows.push(SensitivityRow::new(
                "equity",
                "delta",
                Some(underlier.clone()),
                None,
                None,
                *amount,
            ));
        }
        for (underlier, amount) in &self.equity_vega {
            rows.push(SensitivityRow::new(
                "equity",
                "vega",
                Some(underlier.clone()),
                None,
                None,
                *amount,
            ));
        }
        for (currency, amount) in &self.fx_delta {
            rows.push(SensitivityRow::new(
                "fx",
                "delta",
                Some(currency.to_string()),
                None,
                None,
                *amount,
            ));
        }
        for ((ccy1, ccy2), amount) in &self.fx_vega {
            rows.push(SensitivityRow::new(
                "fx",
                "vega",
                pair_label(*ccy1, *ccy2),
                None,
                None,
                *amount,
            ));
        }
        for (bucket, amount) in &self.commodity_delta {
            rows.push(SensitivityRow::new(
                "commodity",
                "delta",
                None,
                Some(bucket.clone()),
                None,
                *amount,
            ));
        }
        for (bucket, amount) in &self.commodity_vega {
            rows.push(SensitivityRow::new(
                "commodity",
                "vega",
                None,
                Some(bucket.clone()),
                None,
                *amount,
            ));
        }
        for input in &self.curvature {
            rows.push(SensitivityRow {
                expiry_tenor: Some(input.expiry_tenor.clone()),
                ..SensitivityRow::new(
                    serde_label(&input.risk_class)?,
                    "curvature",
                    Some(input.factor.clone()),
                    Some(input.bucket.clone()),
                    input.risk_tenor.clone(),
                    input.volatility_weighted_vega,
                )
            });
        }

        Ok(sorted(rows))
    }

    /// Rebuild a container from long-format rows (the inverse of
    /// [`Self::to_rows`]).
    ///
    /// Rows with the same key accumulate through the matching `add_*`
    /// adder; each `curvature` row becomes one validated
    /// [`SimmCurvatureSensitivity`].
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Currency in which every row `amount` is expressed;
    ///   the rows themselves do not carry it.
    /// * `rows` - Long-format rows encoded as [`Self::to_rows`] documents, in
    ///   any order.
    ///
    /// # Returns
    ///
    /// A container equal to the one that exported `rows`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an unknown `(risk_class, kind)`, a
    /// missing column the row needs, an unknown currency, credit sector or
    /// risk class label, a malformed `"CCY1/CCY2"` pair, or a curvature row
    /// that fails [`SimmCurvatureSensitivity::validate`].
    pub fn from_rows(base_currency: Currency, rows: &[SensitivityRow]) -> Result<Self> {
        let mut sens = Self::new(base_currency);
        for row in rows {
            sens.add_row(row)?;
        }
        Ok(sens)
    }

    /// Add one long-format row through the matching adder.
    fn add_row(&mut self, row: &SensitivityRow) -> Result<()> {
        let amount = row.amount;
        if row.kind == "curvature" {
            let input = SimmCurvatureSensitivity {
                risk_class: serde_parse(&row.risk_class)?,
                bucket: row.need(&row.bucket, "bucket")?.to_string(),
                factor: row.issuer()?.to_string(),
                risk_tenor: row.tenor.clone(),
                expiry_tenor: row.need(&row.expiry_tenor, "expiry_tenor")?.to_string(),
                volatility_weighted_vega: amount,
            };
            input.validate()?;
            self.add_curvature(input);
            return Ok(());
        }
        let sector = || serde_parse(row.need(&row.bucket, "bucket")?);
        match (row.risk_class.as_str(), row.kind.as_str()) {
            ("interest_rate", "delta") => {
                self.add_ir_delta(row.issuer_currency()?, row.tenor()?, amount);
            }
            ("interest_rate", "vega") => {
                self.add_ir_vega(row.issuer_currency()?, row.tenor()?, amount);
            }
            ("credit_qualifying", "delta") => {
                self.add_credit_qualifying_delta(sector()?, row.issuer()?, row.tenor()?, amount);
            }
            ("credit_qualifying", "vega") => {
                self.add_credit_qualifying_vega(sector()?, row.issuer()?, row.tenor()?, amount);
            }
            ("credit_non_qualifying", "delta") => {
                self.add_credit_non_qualifying_delta(row.issuer()?, row.tenor()?, amount);
            }
            ("credit_non_qualifying", "vega") => {
                self.add_credit_non_qualifying_vega(row.issuer()?, row.tenor()?, amount);
            }
            ("equity", "delta") => self.add_equity_delta(row.issuer()?, amount),
            ("equity", "vega") => self.add_equity_vega(row.issuer()?, amount),
            ("fx", "delta") => self.add_fx_delta(row.issuer_currency()?, amount),
            ("fx", "vega") => {
                let (ccy1, ccy2) = row.issuer_pair()?;
                self.add_fx_vega(ccy1, ccy2, amount);
            }
            ("commodity", "delta") => {
                self.add_commodity_delta(row.need(&row.bucket, "bucket")?, amount);
            }
            ("commodity", "vega") => {
                self.add_commodity_vega(row.need(&row.bucket, "bucket")?, amount);
            }
            _ => return Err(row.unsupported("SIMM")),
        }
        Ok(())
    }

    /// Export every sensitivity as one long-format table.
    ///
    /// Columns: `risk_class`, `bucket`, `tenor`, `issuer`, `kind`, `amount`,
    /// `expiry_tenor` (null except on curvature rows). The rows are
    /// [`Self::to_rows`]; an empty container still yields every column.
    ///
    /// # Errors
    ///
    /// Returns an error if [`Self::to_rows`] fails or column lengths
    /// disagree.
    pub fn to_table(&self) -> Result<TableEnvelope> {
        rows_to_table(self.to_rows()?, Layout::Simm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regulatory::frtb::{DrcAssetType, DrcSector, DrcSeniority};
    use crate::types::{SimmCreditSector, SimmRiskClass};

    #[test]
    fn empty_frtb_table_keeps_schema() {
        let sens = FrtbSensitivities::new(Currency::USD);
        let table = sens.to_table().expect("table");
        let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "risk_class",
                "bucket",
                "tenor",
                "issuer",
                "kind",
                "amount",
                "sector",
                "seniority",
                "asset_type",
                "maturity_years",
                "pnl_adjustment",
            ]
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

    fn every_frtb_adder() -> FrtbSensitivities {
        let mut sens = FrtbSensitivities::new(Currency::EUR);
        sens.add_girr_delta(Currency::USD, "5Y", 25_000.0);
        sens.add_girr_inflation_delta(Currency::USD, 1_000.0);
        sens.add_girr_xccy_basis_delta(Currency::EUR, 500.0);
        sens.add_girr_vega(Currency::USD, "1Y", "5Y", 2_000.0);
        sens.add_girr_curvature(Currency::USD, 300.0, -200.0);
        sens.add_csr_nonsec_delta("ACME", 3, "5Y", "bond", 4_000.0);
        sens.add_csr_nonsec_vega("ACME", 3, "1Y", 400.0);
        sens.add_csr_nonsec_curvature("ACME", 3, 50.0, -40.0);
        sens.add_csr_sec_ctp_delta("CDX-T", 1, "5Y", "bond", 1_000.0);
        sens.add_csr_sec_ctp_vega("CDX-T", 1, "1Y", 100.0);
        sens.add_csr_sec_ctp_curvature("CDX-T", 1, 10.0, -8.0);
        sens.add_csr_sec_nonctp_delta("ABS-1", 1, "5Y", "bond", 1_000.0);
        sens.add_csr_sec_nonctp_vega("ABS-1", 1, "1Y", 100.0);
        sens.add_csr_sec_nonctp_curvature("ABS-1", 1, 10.0, -8.0);
        sens.add_equity_delta("ACME", 1, 12_000.0);
        sens.add_equity_repo_delta("ACME", 1, 1_000.0);
        sens.add_equity_vega("ACME", 1, "1Y", 600.0);
        sens.add_equity_curvature("ACME", 1, 70.0, -60.0);
        sens.add_commodity_delta("WTI", 2, "1Y", "cushing", 3_000.0);
        sens.add_commodity_vega("WTI", 2, "1Y", 300.0);
        sens.add_commodity_curvature("WTI", 2, 30.0, -20.0);
        sens.add_fx_delta(Currency::EUR, Currency::USD, 9_000.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, "1Y", 900.0);
        sens.add_fx_curvature(Currency::EUR, Currency::USD, 90.0, -80.0);
        sens.add_drc_position(DrcPosition {
            maturity_years: 0.5,
            issuer: "ACME".to_string(),
            jtd_amount: 1_000_000.0,
            rating_bucket: 3,
            sector: DrcSector::Corporate,
            seniority: DrcSeniority::SeniorUnsecured,
            asset_type: DrcAssetType::Corporate,
            pnl_adjustment: -2_500.0,
        });
        sens.add_rrao_position("EXOTIC-1", 5_000_000.0, true);
        sens.add_rrao_position("PLAIN-1", 5_000_000.0, false);
        sens
    }

    #[test]
    fn frtb_from_rows_inverts_to_rows_including_drc() {
        let sens = every_frtb_adder();
        let rows = sens.to_rows().expect("rows");
        let drc = rows
            .iter()
            .find(|row| row.risk_class == "drc")
            .expect("drc row");
        assert_eq!(drc.sector.as_deref(), Some("corporate"));
        assert_eq!(drc.seniority.as_deref(), Some("senior_unsecured"));
        assert_eq!(drc.maturity_years, Some(0.5));
        let restored = FrtbSensitivities::from_rows(Currency::EUR, &rows).expect("from_rows");
        assert_eq!(restored, sens);
    }

    #[test]
    fn frtb_from_rows_rejects_unknown_rows_and_incomplete_drc() {
        let unknown = SensitivityRow::new("girr", "gamma", Some("USD".into()), None, None, 1.0);
        let err = FrtbSensitivities::from_rows(Currency::USD, &[unknown]).expect_err("unknown");
        assert!(err.to_string().contains("unsupported FRTB"), "{err}");

        let drc = SensitivityRow::new(
            "drc",
            "jtd",
            Some("ACME".into()),
            Some("3".into()),
            None,
            1.0,
        );
        let err = FrtbSensitivities::from_rows(Currency::USD, &[drc]).expect_err("drc");
        assert!(err.to_string().contains("maturity_years"), "{err}");
    }

    #[test]
    fn simm_from_rows_inverts_to_rows() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 1_000.0);
        sens.add_ir_vega(Currency::EUR, "1Y", 50.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Sovereign, "UST", "5Y", 300.0);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK", "1Y", 30.0);
        sens.add_credit_non_qualifying_delta("RMBS", "5Y", 20.0);
        sens.add_credit_non_qualifying_vega("RMBS", "1Y", 2.0);
        sens.add_equity_delta("AAPL", 500.0);
        sens.add_equity_vega("AAPL", 5.0);
        sens.add_fx_delta(Currency::EUR, 700.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 2.0);
        sens.add_commodity_delta("1", 40.0);
        sens.add_commodity_vega("1", 4.0);
        sens.add_curvature(SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::Equity,
            bucket: "residual".to_string(),
            factor: "AAPL".to_string(),
            risk_tenor: None,
            expiry_tenor: "1Y".to_string(),
            volatility_weighted_vega: 100.0,
        });
        let rows = sens.to_rows().expect("rows");
        let restored = SimmSensitivities::from_rows(Currency::USD, &rows).expect("from_rows");
        assert_eq!(restored, sens);
    }
}
