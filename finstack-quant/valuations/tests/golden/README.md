# Golden Tests

Golden fixtures pin Finstack Quant's valuation outputs against externally sourced
reference values (Bloomberg screens, QuantLib, closed-form formulas). Fixtures
live under `data/` and use the strict `finstack_quant.golden/2` schema. Both the Rust
runner (`tests/golden/`) and the Python bindings layer
(`finstack-quant-py/tests/golden/`) load the same JSON files and must agree.

Pricing fixtures are grouped first by golden type, then by instrument:

```text
data/pricing/
├── regression_goldens/  # Locks Finstack pricing behavior
├── quantlib/             # Compares Finstack with QuantLib
└── bloomberg/            # Compares Finstack with Bloomberg references
```

The directory must agree with `metadata.source`: `quantlib` belongs under
`quantlib/`; `bloomberg-api` and `bloomberg-screen` belong under `bloomberg/`;
formula, textbook, and Intex fixtures belong under `regression_goldens/`.
Instrument directories such as `bond/`, `fra/`, and `swaption/` live beneath
each golden type. Bloomberg screenshot evidence stays beside its fixture in an
instrument-level `screenshots/` directory.

## Schema (`finstack_quant.golden/2`)

Each fixture is one JSON object with exactly these top-level sections:

```jsonc
{
  "schema_version": "finstack_quant.golden/2",
  "metadata": { /* identity, provenance, valuation date, screenshots */ },
  "kind": "pricing",            // or "sabr_smile"
  /* ...kind-specific body fields (see below)... */
  "expected":   { "npv": 1039530.56, "dv01": -333.0 },
  "tolerances": { "npv": { "abs": 0.01, "tolerance_reason": "..." }, "dv01": { "abs": 1.0 } }
}
```

Unknown top-level keys and unknown `metadata` keys are rejected. The canonical
struct definitions are in [`schema.rs`](schema.rs); the Python mirror is in
[`finstack-quant-py/tests/golden/schema.py`](../../../../finstack-quant-py/tests/golden/schema.py).

### `metadata`

`name`, `domain`, `description`, `valuation_date` (YYYY-MM-DD), `source`
(`quantlib` | `bloomberg-api` | `bloomberg-screen` | `intex` | `formula` |
`textbook`), `source_detail`, `captured_by`, `captured_on`, `last_reviewed_by`,
`last_reviewed_on`, `review_interval_months`, `regen_command`, and optional
`screenshots`. `bloomberg-screen` and `intex` sources require at least one
screenshot under a `screenshots/` directory next to the fixture.

### `kind: "pricing"`

Instrument-pricing body:

- `model` — pricing model selector (`discounting`, `tree`, `hull_white_1f`, …).
- `market` — a tagged union, exactly one of:
  - `{ "kind": "snapshot", "data": { /* MarketContext */ } }`
  - `{ "kind": "envelope", "envelope": { /* CalibrationEnvelope */ } }`
- `instrument` — a `finstack_quant.instrument/1` envelope.

The requested metric list is **derived** from the `expected` keys (each key's
base name before `::`, excluding `npv`); there is no separate `metrics` field.

### `kind: "sabr_smile"`

Closed-form SABR smile body: `alpha`, `beta`, `nu`, `rho`, optional `shift`,
`forward`, `time_to_expiry`, and `strikes` (`[{ "key": "...", "strike": ... }]`).
The strike keys must match the `expected` keys exactly. SABR fixtures live under
`data/market_data/sabr/` and run through their own entry point in `sabr.rs`.

### `expected` and `tolerances`

`expected` holds the externally sourced reference values and is the single
source of truth — tests never rewrite it to match model output. `tolerances`
must cover every `expected` metric, each with an `abs` and/or `rel` bound; a
`tolerance_reason` is required for any expected risk metric that is exactly zero.

## Non-executable fixtures

A few Bloomberg fixtures retain screen values that the executable runner cannot
yet reproduce. Rather than encode that status in each fixture, both layers read
one shared allowlist, [`known_non_executable.json`](known_non_executable.json)
(`{ "path": ..., "reason": ... }` entries, paths relative to `data/`). The Rust
runner ([`pricing.rs`](pricing.rs)) reports their failures non-fatally; the
Python layer turns each into `pytest.mark.xfail(strict=False)` via
`discover_fixtures_with_marks` in `conftest.py`. Add a fixture to that one file
to park it on both sides.

To run every fixture strictly and surface the parked failures, set
`GOLDEN_IGNORE_NON_EXECUTABLE=1` (both layers honor it) or run
`mise run goldens-test-strict`.

## Native QuantLib fixtures

Deterministic native QuantLib generators live in
[`scripts/golden/quantlib/`](../../../../scripts/golden/quantlib/). Regenerate
the committed set with `mise run goldens-quantlib-generate`; verify committed
fixtures have not drifted with `mise run goldens-quantlib-check`.

| Product | Status | Native QuantLib API |
| --- | --- | --- |
| FRA | Executable | `ForwardRateAgreement` |
| SOFR future helper | Executable | `SofrFutureRateHelper` |
| Fixed risk-free bond | Executable | `FixedRateBond` |
| Fixed zero-recovery hazard bond | Executable | `RiskyBondEngine` |
| Floating risk-free bond | Parity deferred | `FloatingRateBond`; price and parallel DV01 exceed validation targets |
| Floating zero-recovery hazard bond | Parity deferred | `FloatingRateBond` + `RiskyBondEngine`; CS01 passes, price and parallel DV01 do not |
| Fixed callable bond OAS | Parity deferred | `CallableFixedRateBond` + Hull-White tree; OAS passes, option-adjusted DV01 does not |
| European equity option | Executable | `VanillaOption` + `AnalyticEuropeanEngine`; NPV, delta, gamma, vega, and rho |
| European FX option | Executable | `GarmanKohlagenProcess` + `AnalyticEuropeanEngine`; NPV, three delta conventions, gamma, vega, and domestic/foreign rho |
| EUR/USD cash digital option | Executable | `CashOrNothingPayoff` + `AnalyticEuropeanEngine`; Garman-Kohlhagen NPV |
| EUR/USD barrier option | Executable | `BarrierOption` + `AnalyticBarrierEngine`; continuous zero-rebate up-and-out call NPV |
| Nikkei USD quanto option | Executable | `QuantoVanillaOption` + `QuantoEuropeanEngine`; fixed-conversion call NPV |
| Black caplet | Executable | `Cap` + `BlackCapFloorEngine`; NPV, vega, and parallel DV01 |
| Four-period Black cap | Executable | `Cap` + `BlackCapFloorEngine`; quarterly NPV, vega, and parallel DV01 |
| Bachelier floorlet | Executable | `Floor` + `BachelierCapFloorEngine`; negative-rate NPV, vega, and parallel DV01 |
| Black European swaption | Executable | `Swaption` + `BlackSwaptionEngine`; NPV, vega, and parallel DV01 |
| Bachelier European swaption | Executable | `Swaption` + `BachelierSwaptionEngine`; NPV, vega, and parallel DV01 |
| EUR/USD forward | Executable | `FxForward` + `DiscountingFxForwardEngine`; USD NPV, FX01, and parallel DV01 |
| Continuous barrier option | Executable | `BarrierOption` + `AnalyticBarrierEngine`; non-degenerate down-and-out call NPV |
| Discrete geometric Asian option | Executable | `DiscreteAveragingAsianOption` + `AnalyticDiscreteGeometricAveragePriceAsianEngine`; actual-fixing-time NPV |
| Discrete arithmetic Asian option | Executable | `DiscreteAveragingAsianOption` + `TurnbullWakemanAsianEngine`; future-fixing NPV |
| Continuous fixed-strike lookback option | Executable | `ContinuousFixedLookbackOption` + `AnalyticContinuousFixedLookbackEngine`; OTM direct-formula NPV |
| Continuous floating-strike lookback option | Executable | `ContinuousFloatingLookbackOption` + `AnalyticContinuousFloatingLookbackEngine`; fresh-trade NPV |
| Single-name CDS | Executable | `CreditDefaultSwap` + `IsdaCdsEngine`; decomposition, par spread, DV01, and direct-hazard CS01 |
| USD deposit | Executable | `LogLinearInterpolation` and QuantLib day counts; holder-view NPV, par rate, and parallel DV01 |
| IRS | Deferred | Native `VanillaSwap` schedule and projected-fixing conventions do not yet match Finstack strictly; generate the diagnostic candidate with `--product irs` |
| CDS tranche | Unsupported | QuantLib 1.42 Python bindings expose no native tranche, synthetic CDO, or base-correlation instrument |
| Floating callable bond OAS | Unsupported | QuantLib 1.42 exposes callable fixed and zero-coupon bonds, but no callable floating-rate bond |

Hazard-bond references use zero recovery because QuantLib and Finstack place
recovery payments at different points within coupon intervals. Validation
targets are set independently of current residuals: vanilla prices must agree
within 0.01-0.02 per 100, vanilla DV01 and credit CS01 within 0.1-0.5%, and
callable OAS within 1.5bp with option-adjusted DV01 within 5%. Fixtures outside
those targets remain source-backed but are parked in
[`known_non_executable.json`](known_non_executable.json) until conventions or
model risk are aligned; strict golden runs continue to fail on those gaps.

The analytical option fixtures use absolute tolerances of 1e-7 for equity, FX,
cap/floor, and swaption outputs. FX-forward NPV and risk retain one-cent
financial tolerances. Equity theta is intentionally excluded because QuantLib's
analytic `thetaPerDay` and Finstack's one-calendar-day market roll are different
contracts. Cap/floor and swaption implied volatility are excluded because the
metric requires an externally supplied market price; feeding the generated NPV
back into the fixture would be circular.

The second-wave exotic fixtures compare closed-form NPV at `1e-7` absolute
tolerance. The fixed-strike lookback case is deliberately out of the money so
it exercises the direct Conze-Viswanathan formula. The separate
`observed_max >= strike` decomposition currently differs materially from
QuantLib and is not hidden by a broader tolerance.

Third-wave analytical FX and exotic fixtures also use `1e-7` absolute NPV
tolerance. The full cap uses the same strict numerical-parity tolerance for its
four-period aggregate and risk outputs. The single-name CDS and deposit are
registered in the same unified generator while preserving their original
valuation dates, committed market curves, holder-view deposit treatment, and
canonical Finstack risky-annuity definitions.

## Migrating fixtures

The one-off v1→v2 migration script lives at
[`scripts/golden/migrate_v1_to_v2.py`](../../../../scripts/golden/migrate_v1_to_v2.py)
(`uv run scripts/golden/migrate_v1_to_v2.py [--check]`).
