# finstack-quant-margin

Margin agreement terms, collateral rules, VM/IM calculators, XVA formulas, and
standardized regulatory capital engines.

The crate is standalone from `finstack-quant-valuations` so consumers can share
CSA terms, IM/VM engines, registry-backed defaults, and capital helpers without
pulling in the instrument stack.

## Position in the stack

Depends only on `finstack-quant-core` (plus `serde`, `serde_json`, `time`,
`tracing`, and optional `schemars` behind the default `json-schema` feature).
It does **not** depend on `finstack-quant-valuations` or
`finstack-quant-models`; the [`Marginable`](src/traits.rs) trait is the seam,
and valuations implements it as a bridge layer.

Consumed by `finstack-quant-valuations`, re-exported from the umbrella crate as
`finstack_quant::margin`, and bound in both Python and WASM.

## Workflows

- **OTC and repo agreements** — CSA terms, VM parameters, IM methodology,
  eligible collateral schedules, repo margining rules
  (`OtcMarginSpec`, `CsaSpec`, `RepoMarginSpec`).
- **Margin calculators** — variation margin, ISDA SIMM, BCBS-IOSCO schedule IM,
  collateral-haircut IM, and CCP proxy IM.
- **Regulatory capital** — FRTB sensitivity-based approach and SA-CCR EAD
  (`regulatory::frtb`, `regulatory::sa_ccr`).
- **XVA** — netting/collateral helpers plus CVA/DVA/FVA/MVA over
  caller-supplied exposure profiles (`xva::{cva, mva, netting, types}`).

## Public modules

| Module | Role |
|--------|------|
| `types` | CSA, collateral, repo, SIMM, netting, margin-call, and threshold types |
| `calculators` | `VmCalculator` plus the four IM engines and the `ImCalculator` trait |
| `traits` | `Marginable` — the integration point for consumer crates |
| `metrics` | `MarginUtilization`, `ExcessCollateral`, `MarginFundingCost`, `Haircut01` — the two-`Money` constructors return `Result` and reject cross-currency inputs |
| `regulatory` | `frtb` (SBA) and `sa_ccr` (EAD) |
| `xva` | `types`, `cva`, `mva`, `netting` |
| `schema` | `MarginEnvelope`, `MarginSchema`, and the generated JSON Schema contract |
| `constants` | Shared heuristics (`ONE_BP`, standard tenor buckets, IG spread threshold, …) |

The registry module is `pub(crate)`. Configured defaults are reached through
public constructors such as `CsaSpec::regulatory_from_config`,
`SimmCalculator::from_finstack_config`, and
`ScheduleImCalculator::from_finstack_config`.

## Quick examples

### Bilateral OTC spec

```rust
use finstack_quant_margin::{CsaSpec, OtcMarginSpec};

let csa = CsaSpec::usd_regulatory()?;
let spec = OtcMarginSpec::bilateral_simm(csa);

assert!(spec.csa.requires_im());
assert_eq!(spec.vm_frequency.to_string(), "daily");
# Ok::<(), finstack_quant_core::Error>(())
```

`OtcMarginSpec::usd_bilateral()` is the one-call equivalent that resolves the CSA
from the embedded registry.

### SIMM from sensitivities

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_margin::{SimmCalculator, SimmSensitivities, SimmVersion};

let calc = SimmCalculator::new(SimmVersion::V2_6)?;

let mut sensitivities = SimmSensitivities::new(Currency::USD);
sensitivities.add_ir_delta(Currency::USD, "5Y", 50_000.0);
sensitivities.add_equity_delta("AAPL", 100_000.0);

// (total_im: f64, breakdown: HashMap<String, Money>)
let (total_im, breakdown) = calc.calculate_from_sensitivities_parts(&sensitivities, Currency::USD)?;
assert!(total_im >= 0.0);
assert!(breakdown.contains_key("IR_Delta"));
# Ok::<(), finstack_quant_core::Error>(())
```

`calculate_from_sensitivities(&sens, currency, as_of)` is the canonical entry
point: it validates the container (unknown tenors and commodity buckets are
rejected rather than priced as zero) and returns the same numbers wrapped in
an `ImResult` with methodology and MPOR stamped.

### Variation margin

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Month};
use finstack_quant_core::money::Money;
use finstack_quant_margin::{CsaSpec, VmCalculator};

let calc = VmCalculator::new(CsaSpec::usd_regulatory()?);

let exposure = Money::new(5_000_000.0, Currency::USD)?;
let posted = Money::new(3_000_000.0, Currency::USD)?;
let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");

let result = calc.calculate(exposure, posted, as_of)?;
assert!(result.requires_call());
# Ok::<(), finstack_quant_core::Error>(())
```

`VmCalculator::calculate` rejects an exposure or collateral amount whose currency
differs from `csa.base_currency` — it never converts implicitly. `VmResult`
carries `gross_exposure`, `net_exposure`, `post_amount`, `collect_amount`, and
the `settlement_date` derived from the CSA's call timing.

### `Marginable` integration

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_margin::{
    Marginable, NettingSetId, OtcMarginSpec, SimmCalculator, SimmSensitivities, SimmVersion,
};

struct ExampleTrade {
    id: String,
    spec: OtcMarginSpec,
    mtm: Money,
    sensitivities: SimmSensitivities,
}

impl Marginable for ExampleTrade {
    fn id(&self) -> &str {
        &self.id
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        Some(&self.spec)
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        None
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<SimmSensitivities> {
        Ok(self.sensitivities.clone())
    }

    fn mtm_for_vm(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(self.mtm)
    }
}
```

Only `id`, `margin_spec`, `netting_set_id`, `simm_sensitivities`, and
`mtm_for_vm` are required. `repo_margin_spec`, `im_exposure_base`, and
`has_margin` have defaults.

## Conventions

- Rates, spreads, and haircuts are decimal fractions unless a field name says
  otherwise — for example `funding_spread_bp` and `margin_funding_spread_bp` on
  `xva::types::FundingConfig` are basis points.
- VM/IM thresholds, MTAs, independent amounts, and all calculator results are
  `Money`. Currency mismatches error rather than converting.
- `Marginable::simm_sensitivities` expects currency-denominated risk measures
  per 1bp rate/credit move or per 1% relative equity/FX/commodity price move.
  Vegas are `sigma * dPV/dsigma`, before VRW, HVR and concentration; non-IR
  sigma follows paragraph 10(b). Concentration uses raw sensitivities, in the
  units of the official USD-million threshold tables. The supported
  v2.6 calculation is an indicative USD-only approximation: product-class,
  subcurve delta/vega, equity classification, non-qualifying credit classification,
  commodity within-bucket factor detail and IR vega underlying-maturity dimensions
  remain incomplete. Unclassified equity and non-qualifying credit use the
  published residual parameters. Curvature uses `SimmCurvatureSensitivity` to
  retain risk tenor, factor and option expiry, applying expiry scaling before
  netting. Curvature no longer accepts a scalar per class. Results carry
  `approximation = true`; this is not a current regulatory SIMM implementation.
- **FRTB risk weights reproduce the Basel tables as published, so the expected
  sensitivity scale varies by risk class**: GIRR/CSR/equity/commodity/FX delta
  weights are in percent (so feed `$` per 1 percentage-point or 1 % move — GIRR
  delta is 100× DV01), while vega weights are decimal and multiply volatility-scaled vega
  (`sigma * dV/dsigma`). Pre-scale your sensitivity feed; do not edit the weight tables. The full
  table is in the [`regulatory::frtb`](src/regulatory/frtb/mod.rs) module docs.
- Schedule IM, the cleared-IM proxy, and haircut IM invoked through the
  `ImCalculator` trait require `Marginable::im_exposure_base`. They fail closed
  rather than falling back to MtM as a pseudo-notional.
- `CsaSpec::apply_im_terms` applies the CSA's allocated threshold and MTA once
  to summed gross IM already calculated for the elected MPOR. It returns
  `ImCollateralResult` with gross model IM, target collateral, current balance,
  signed transfer and segregation. Positive transfers post additional IM;
  negative transfers return excess. MTA equality triggers a transfer. IM and VM
  remain separate; segregated IM cannot be reused to meet VM. The caller allocates
  any group-wide threshold across CSAs.
- XVA adjustments are positive when they cost the desk and compose as
  `total_xva = CVA − DVA + FVA + MVA`. Exposure times are nonnegative, strictly increasing year fractions and
  may include zero. Exposure is held constant before the first supplied point
  for CVA/DVA/FVA, consistent with IM extrapolation for MVA.
- Persisted SIMM and FRTB tuple-keyed sensitivities serialize as deterministic
  sorted entry arrays (`SimmSensitivitiesJson`, the FRTB wire type), because
  tuple keys were never representable as JSON object keys.

See [`INVARIANTS.md`](../../INVARIANTS.md) for the workspace-wide Decimal/f64,
currency-safety, and serde rules.

## Calculators

| Calculator | Entry point | Notes |
|------------|-------------|-------|
| `VmCalculator` | `calculate(exposure, posted, as_of)` | Applies CSA threshold, MTA, rounding, and settlement dating. `generate_margin_calls` and `margin_call_dates` cover schedules. |
| `SimmCalculator` | `calculate_from_sensitivities` | Indicative historical v2.6 parameters from the registry; USD inputs only and results explicitly approximate. Risk-class aggregation reduces in a canonical order so the quadratic form is bit-reproducible. |
| `ScheduleImCalculator` | `calculate_for_notional` | BCBS-IOSCO grid (`BCBS_IOSCO_SCHEDULE_ID = "bcbs_iosco"`); the `ImCalculator` path uses `im_exposure_base`. |
| `ClearingHouseImCalculator` | `calculate_conservative`, `with_input_source` | Accepts an external CCP value via `ExternalImSource`, or scales an exposure base by registry-backed proxy rates (`lch_swapclear`, `ice_clear_credit`, `cme`, `generic_var`). |
| `HaircutImCalculator` | `calculate_for_collateral` | Uses `with_collateral_terms` to match maturity/rating eligibility and the matching entry’s haircut and FX add-on; rejects ineligible or negative collateral. |

Repo margining rules are separate from the haircut IM engine: `RepoMarginSpec`
carries a `margin_type: RepoMarginType` (`None`, `MarkToMarket`, `NetExposure`,
`Triparty`) alongside the margin ratio, call threshold, frequency, and
settlement lag, and answers `required_collateral`, `call_trigger_value`,
`requires_margin_call`, `margin_deficit`, and `excess_collateral` directly.
Separately, `types::{generate_margin_cashflows,
generate_margin_interest_cashflows, margin_calls_to_cashflows}` turn calls into
`CashFlow`s. Repo cashflow generation requires the contractual business-day calendar.
The signed VM balance is positive collateral held and negative collateral posted,
including pending agreed calls. `post_amount` is desk cash paid; `collect_amount`
is desk cash received, including returns in either direction.

## Regulatory capital

`FrtbSbaEngine::default()` runs all scenarios and risk classes;
`FrtbSbaEngine::new(scenarios, risk_classes)` restricts either.
`calculate(&FrtbSensitivities)` returns an
`FrtbSbaResult` with delta/vega/curvature by risk class, DRC, RRAO, the binding
correlation scenario, per-scenario charges, and stamped `ResultsMeta`.

`SaCcrEngine::default()` (or `SaCcrEngine::with_alpha(alpha)`) plus
`calculate_ead(&SaCcrNettingSetConfig, &[SaCcrTrade])` returns an `EadResult`
with replacement cost, PFE, multiplier, add-on breakdown, alpha, and a reporting
maturity factor. Note that `EadResult::maturity_factor` is a **reporting summary
only** for unmargined sets — a tenor-weighted average, not the per-trade factors
actually used inside the add-on.

## XVA scope

`xva::types::ExposureProfile` is a container: callers supply the EPE/ENE grid
from their own simulation or valuation stack, and `xva::cva::{compute_cva,
compute_dva, compute_fva, compute_bilateral_xva}` and `xva::mva::compute_mva`
turn it into adjustments. `xva::mva::im_profile_from_simm` builds an IM profile
from a SIMM amount and an `ImDecayProfile`.

Exposure *generation* — rolling curves forward and repricing instruments — is out
of scope: it needs the pricing stack, which sits above this crate. Wrong-way risk
and scenario carry are also out of scope.

## Embedded data and configuration

Registry JSON is embedded at build time from `data/margin/`:

| File | Purpose |
|------|---------|
| [`data/margin/defaults.v1.json`](data/margin/defaults.v1.json) | Default VM/IM parameters, call timing, cleared settlement |
| [`data/margin/schedule_im.v1.json`](data/margin/schedule_im.v1.json) | Schedule IM grids (e.g. `bcbs_iosco`) |
| [`data/margin/collateral_schedules.v1.json`](data/margin/collateral_schedules.v1.json) | Eligible collateral and haircuts |
| [`data/margin/ccp_methodologies.v1.json`](data/margin/ccp_methodologies.v1.json) | CCP proxy rates and MPOR |
| [`data/margin/simm.v1.json`](data/margin/simm.v1.json) | SIMM weights, correlations, concentration thresholds |

Overlays go in the `FinstackConfig` extension key `margin.registry.v1`. The
overlay is a deep object merge over a root whose sections are `defaults`,
`schedule_im`, `collateral_schedules`, `ccp`, and `simm` — each
section mirroring its file, so overriding a VM default nests `defaults` twice:

```json
{
  "extensions": {
    "margin.registry.v1": {
      "defaults": {
        "defaults": {
          "vm": { "mta": 250000.0 }
        }
      }
    }
  }
}
```

Every wire struct denies unknown fields, so a mis-nested overlay errors instead
of being silently ignored. Objects merge key-by-key; arrays and scalars are
replaced wholesale, and `null` replaces rather than deletes.

## JSON Schema contract

`MarginEnvelope` is the strict root: exactly one of an `OtcMarginSpec`, a
`CsaSpec`, or a `MarginCall`, each carrying the required marker
`"schema": "finstack_quant.margin/1"`. The generated schema is checked in at
[`schemas/margin/1/margin.schema.json`](schemas/margin/1/margin.schema.json) and
indexed by [`schemas/index.json`](schemas/index.json).

```bash
cargo run -p finstack-quant-margin --bin gen_margin_schema -- --write   # regenerate
cargo run -p finstack-quant-margin --bin gen_margin_schema -- --check   # verify
```

Both are wired into `mise run rust-gen-schemas` and `mise run rust-check-schemas`.
See [`docs/SERDE_STABILITY.md`](../../docs/SERDE_STABILITY.md) and
[`docs/CONTRACTS.md`](../../docs/CONTRACTS.md).

## Bindings

- **Python** — `finstack_quant.margin` exposes `CsaSpec`,
  `EligibleCollateralSchedule`, the domain enums and identifiers
  (`ImMethodology`, `MarginTenor`, `MarginCallType`, `ClearingStatus`,
  `CollateralAssetClass`, `NettingSetId`), `VmCalculator`/`VmResult`,
  `SimmCalculator`/`SimmSensitivities`, `ScheduleImCalculator`,
  `HaircutImCalculator`, `ImResult`, the XVA surface (`FundingConfig`,
  `ExposureProfile`, `ExposureDiagnostics`, `XvaResult`,
  `ImProfile`, `ImDecayProfile`, `MvaResult`,
  `compute_bilateral_xva`, `compute_mva`, `im_profile_from_simm`), the four
  metrics types, `FrtbSensitivities`/`FrtbSbaEngine`/`FrtbSbaResult`/`frtb_sba_charge`,
  `SaCcrTrade`/`SaCcrNettingSetConfig`/`SaCcrEngine`/`EadResult`/`saccr_ead`, `CONSTANTS`,
  and a `schema` submodule.
- **WASM** — the `margin` namespace in
  [`exports/margin.js`](../../finstack-quant-wasm/exports/margin.js) is a much
  smaller JSON-oriented surface: `csaUsdRegulatoryJson`, `csaEurRegulatoryJson`,
  `validateCsaJson`, `calculateVm`, `computeBilateralXva`.

## Verification

```bash
cargo nextest run -p finstack-quant-margin --lib --test '*'
cargo clippy -p finstack-quant-margin --lib --bins --tests --examples -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-margin --no-deps
```

Integration suites in [`tests/`](tests):

| Test | Covers |
|------|--------|
| `regulatory_determinism.rs` | Deterministic sorted FRTB wire output |
| `schema_parity.rs` | Checked-in JSON Schema matches the Rust types |
| `simm_schedule_parity.rs` | Pins published ISDA SIMM v2.6 weights, correlations, and concentration thresholds against `data/margin/simm.v1.json` |

Workspace gates: `mise run rust-test`, `mise run rust-lint`,
`mise run rust-check-schemas`. Do not run `cargo test` directly — it pulls in doc
tests the workspace gates run separately.

## References

- [ISDA SIMM](../../docs/REFERENCES.md#isda-simm)
- [ISDA 2016 VM CSA](../../docs/REFERENCES.md#isda-vm-csa-2016)
- [ISDA 2018 IM CSA](../../docs/REFERENCES.md#isda-im-csa-2018)
- [ISDA 2002 Master Agreement](../../docs/REFERENCES.md#isda-2002-master-agreement)
- [BCBS-IOSCO uncleared margin](../../docs/REFERENCES.md#bcbs-iosco-uncleared-margin)
- [BCBS 279 SA-CCR](../../docs/REFERENCES.md#bcbs-279-saccr)
- [BCBS FRTB minimum capital requirements (d457)](../../docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements)
- [Gregory XVA Challenge](../../docs/REFERENCES.md#gregory-xva-challenge)
- [Green XVA](../../docs/REFERENCES.md#green-xva)

Regulatory input boundaries: SA-CCR credit, equity and commodity trades require
an explicit supervisory category; options require their expiry separately from
the underlying maturity. MPOR is at least ten business days for bilateral sets
or five for cleared sets, and the caller must supply any applicable longer
period. Margined EAD is capped by its unmargined counterpart; reported RC/PFE
are the components before that cap. FRTB CSR/commodity delta keys include spread
basis/delivery location; equity repo-rate delta is separate from equity spot.
DRC inputs include residual maturity and seniority, with corporate, sovereign
and local-government buckets. Securitization DRC is explicitly unsupported.
