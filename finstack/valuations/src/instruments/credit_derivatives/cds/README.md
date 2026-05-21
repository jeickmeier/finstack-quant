# Credit Default Swap

## Features

- Single-name CDS with configurable pay/receive leg, coupon, schedule, and accrual-on-default policy.
- Protection leg integrated via the ISDA Standard Model (analytical piecewise-constant integration over the union of hazard- and discount-curve knot points).
- Computes par spread, risky annuity (RPV01), upfront, PV01/CS01, and protection/premium leg PVs.

## Methodology & References

- Deterministic hazard-curve valuation following ISDA CDS Standard Model conventions (survival × discount integration).
- Accrual-on-default uses analytical piecewise-constant integration over hazard and discount knots, with discounting to the default settlement date.
- The protection leg segments on hazard- and discount-curve breakpoints rather than using arbitrary equal-width steps.
- Par-spread denominator can include or exclude accrual-on-default per configuration, matching CDSW/ISDA styles.

## Usage Example

```rust
use finstack_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_valuations::metrics::MetricId;
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 5)?;
let cds = CreditDefaultSwap::example();
let pv = cds.value(&market_context, as_of)?;
let result = cds.price_with_metrics(&market_context, as_of, &[MetricId::ParSpread])?;
let par_spread = result.measures[&MetricId::ParSpread];
```

---

## Margining

Credit default swaps implement full margin support following **ISDA CSA** standards. CDS instruments are categorized under the **Credit** risk classes (Qualifying or Non-Qualifying) for SIMM purposes.

### Regulatory Framework

| Standard | Scope | Key Requirements |
|----------|-------|------------------|
| **BCBS-IOSCO** | Bilateral OTC derivatives | VM/IM requirements, eligible collateral |
| **ISDA SIMM** | Initial margin calculation | Credit delta/vega sensitivities |
| **ICE Clear Credit** | Cleared CDS | Index and single-name clearing |

### Adding Margin Specification

```rust
use finstack_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_margin::{
    OtcMarginSpec, CsaSpec, ClearingStatus, ImMethodology, MarginFrequency,
};

let mut cds = CreditDefaultSwap::example();

// Add bilateral margin specification with SIMM
cds.margin_spec = Some(OtcMarginSpec {
    csa: CsaSpec::usd_regulatory(),
    clearing_status: ClearingStatus::Bilateral,
    im_methodology: ImMethodology::Simm,
    vm_frequency: MarginFrequency::Daily,
    settlement_lag: 1,
});
```

### Cleared CDS

```rust
// For cleared CDS (e.g., ICE Clear Credit)
cds.margin_spec = Some(OtcMarginSpec {
    csa: CsaSpec::usd_regulatory(),
    clearing_status: ClearingStatus::Cleared { ccp: "ICE".to_string() },
    im_methodology: ImMethodology::ClearingHouse,
    vm_frequency: MarginFrequency::Daily,
    settlement_lag: 0,
});
```

### Calculating SIMM Sensitivities

CDS instruments produce **Credit** sensitivities (CS01) distributed by tenor:

```rust
use finstack_margin::{Marginable, SimmSensitivities};

let cds = CreditDefaultSwap::example();
let market = MarketContext::new();
let as_of = date!(2024-01-15);

// Calculate SIMM sensitivities
let sensitivities = cds.simm_sensitivities(&market, as_of)?;

// CDS produces credit delta sensitivities
// Qualifying (investment grade) or Non-Qualifying (HY/EM)
for ((entity, tenor), delta) in &sensitivities.credit_qualifying_delta {
    println!("{} {} bucket: ${:.2}", entity, tenor, delta);
}
```

### Credit Risk Classification

CDS instruments are classified as **Qualifying** or **Non-Qualifying** based on credit quality:

| Classification | Criteria | SIMM Treatment |
|----------------|----------|----------------|
| **Qualifying** | Investment grade (spread < 500bp) | Credit Qualifying risk class |
| **Non-Qualifying** | High yield, EM, distressed | Credit Non-Qualifying risk class |

### Calculating Margin Requirements

```rust
use finstack_margin::metrics::{
    InitialMarginMetric, VariationMarginMetric, TotalMarginMetric,
};

// Calculate initial margin
let im_metric = InitialMarginMetric::new();
let im_result = im_metric.calculate(&cds, &market, as_of)?;

// Calculate variation margin
let vm_metric = VariationMarginMetric::new();
let vm_result = vm_metric.calculate(&cds, &market, as_of)?;

// Get MTM for VM purposes
let mtm = cds.mtm_for_vm(&market, as_of)?;
```

---

## Limitations / Known Issues

- Assumes deterministic recovery and hazard curves; no stochastic credit or default correlation modeling.
- No quanto/currency basis handling beyond chosen discount curve.
- Does not yet expose full CDS-family lifecycle semantics such as explicit step-in date, cash settlement date, or front-end protection toggles.
- Broader CDS-family ISDA remediation for index, tranche, option, and canonical hazard calibration is still in progress.

## Pricing Methodology

- Premium/protection legs projected using hazard and discount curves with accrual-on-default handled per config.
- Accrual-on-default integrated analytically per piecewise-constant interval, discounted to default settlement date.
- Protection leg uses ISDA Standard Model breakpoint integration over the union of hazard and discount curve knots.
- Par spread computed analytically from `Protection_PV / Risky_Annuity` (or full premium leg per bp when CDSW-style is requested).

## Metrics

- PV (buyer/seller), par spread, risky annuity (RPV01), PV01/CS01 (parallel and bucketed).
- Accrual-on-default impact, protection/premium leg PV decomposition, expected loss.
- Upfront-to-spread conversions and clean/dirty accrual reporting.
- Initial margin (SIMM-based) and variation margin via `Marginable` trait.
