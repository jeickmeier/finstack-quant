# MARGIN-COLLATERAL-IM-012 — Schedule initial margin from notional and maturity

**Decision status:** Accepted 2026-11-17 for checked gross schedule initial margin.

**Implementation status:** Existing behavior is described below. Profile validation, explicit trade inputs, Money arithmetic, horizon handling and caller reconciliation with the accepted contract are pending.

**Validation status:** Independent comparison matched all 18 embedded rate cells to the published grid. Decimal examples checked the selected maturity endpoints and gross amount arithmetic. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked gross schedule initial margin

1. Elect the immutable bundled reference or a complete typed custom schedule. Bind its source, version, effective dates and reference horizon. Each supported class has one flat rate or a complete three-rate grid. Require exact decimal rates in `[0, 1]` and reject duplicate, missing or invalid entries at construction and restoration. Use the explicit `other` class only when assigned by the applicable classification. A canonical `custom_[a-z0-9_]+` class must be declared in the elected profile. [Parameter selection and validation](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms).
2. Require qualified effective notional as currency-bearing Money, the applicable class and maturity where the rate varies. Bind each input to the valuation date and information cut. The trade or portfolio owner resolves the effective notional and classification. Retire the five-year interest-rate defaults, the trait route that supplies them and the generic 15% fallback. Return typed errors for missing required input, unsupported class and absent or invalid rates. [Portfolio input ownership](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
3. Require finite nonnegative residual years resolved on the elected schedule basis for a maturity-sensitive class. Apply `[0, 2)`, `[2, 5)` and `[5, ∞)` for the bundled grid. Two and five years enter the higher bucket. Apply the same endpoint rule at a custom grid's finite, positive and strictly ordered cutoffs. A flat class needs no maturity for lookup. A still-eligible trade with zero residual maturity enters the short bucket when maturity applies.
4. Use the bundled rates of 1%, 2% and 4% for interest rate and 2%, 5% and 10% for credit in order of increasing maturity. FX uses a flat 6%. Equity, commodity and Other each use a flat 15%. Compute `abs(effective_notional) × rate` using checked Decimal multiplication in the notional currency. Validate applicable inputs before returning zero for a zero notional or rate. Apply contractual rounding only at its elected boundary. [Basel MGN20.16–17 and Table 1](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn?allChapters=true) · [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
5. The bundled schedule's reference horizon is ten business days. Carry the profile's reference horizon through the result. A use that requires another horizon needs an eligible profile for that horizon and returns a typed error when unavailable. Remove automatic square-root rescaling in the portfolio adapter. [Portfolio Schedule handling](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
6. Return a checked gross Schedule result with the selected profile and qualification, date and cut, class, maturity basis and selected bucket where applicable, exact rate, input notional, currency and horizon. Reuse one Rust calculation through portfolio, Python and WASM. Record 013 owns net-to-gross reduction, record 017 owns contractual collateral terms and record 022 owns the wire format. [Schedule netting](13-schedule-net-gross.md) · [Contractual IM](17-csa-selection-and-im-terms.md) · [Result transport](22-wire-tables-and-provenance.md).

For USD 1 million of interest-rate notional, exact residual maturities of two and five years give gross IM of USD 20,000 and USD 40,000. A trade with the same qualified notional magnitude gives the same standalone gross charge for either sign.

## Choice and rationale

The observed calculator applies a deterministic rate grid. The accepted calculation uses the elected profile and qualified trade inputs so a gross charge can be reproduced from its exact notional, class, maturity and rate.

## Input domain and conventions

Finite monetary notional (sign is discarded), one of six built-in classes or `custom_[a-z0-9_]+`, and finite nonnegative maturity in years. Registered default is interest_rate,5 years, MPOR 10. Maturity is supplied; standalone calculation does not derive it from dates.

## Ordered algorithm

1. Validate maturity; select short if years<2, medium if 2≤years<5, long if years≥5. Thus exact 2 and 5 enter the higher buckets.
2. Lookup `(asset_class,bucket)` or use fallback default_rate0.15. Validate the selected rate is finite and within[0,1].
3. Compute IM=`abs(notional)*rate` in the notional currency and return one class-labelled component, as-of date and MPOR metadata.
4. Embedded short/medium/long rates are IR 1%/2%/4%, credit 2%/5%/10%, equity/commodity/other 15% throughout and FX 6% throughout.
5. Trait calculation requires `Marginable::im_exposure_base`; absence errors. It uses the calculator's default class/maturity and does not substitute current MTM for notional or automatically infer an instrument class.

## Selection, controls and limitations

Configured constructors use schedule `bcbs_iosco`; explicit ID constructors use embedded lookup. Builder maturity validates; public fields can be mutated directly. Registry validates boundaries increasing and positive and default maturity within[0,100], but runtime `rate` only validates maturity and selected rate. MPOR is a label in this standalone calculator; portfolio rescaling is owned by the portfolio inventory.

Registry parsing accepts schedule rates above one before consumer rejection and does not reject zero schedule MPOR; its admitted domain differs from the calculator’s.

## Alternatives and tradeoffs

The observed fallback fills a missing cell with 15%. The accepted complete grid makes the quoted rate traceable to the elected profile. Netting across eligible trades is evaluated in record 013.

## Source and validation evidence

Inline tests inspect selected grid cells, boundary behavior, absence of explicit exposure base and single-trade formulas. `audit_regressions.rs` checks invalid maturity at entry points. The data README records historical Appendix A provenance. The independent comparison covers the rate values, with the exact endpoint rule defined by this record.

Implementation and data: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [src/calculators/im/mod.rs](../../../../finstack-quant/margin/src/calculators/im/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/schedule_im.v1.json](../../../../finstack-quant/margin/data/margin/schedule_im.v1.json).

Existing evidence: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
