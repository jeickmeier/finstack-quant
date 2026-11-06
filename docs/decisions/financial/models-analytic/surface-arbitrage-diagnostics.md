# MODELS-ANALYTIC-20: Surface arbitrage scans, severity and pass policy

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Black-lognormal strike surfaces. Top-level check validates convention/tolerance and requires explicit forwards when any check is enabled; a single forward expands across expiries.

## Choice and rationale

Discrete butterfly, fixed-log-moneyness calendar and Dupire-density scans produce diagnostics. Severity/filtering controls the report’s pass boolean; a passed report is not a proof of global absence of arbitrage.

## Algorithm

Butterfly uses unequal-log-strike derivatives of w and Durrleman g at interior strikes. Calendar compares adjacent-expiry w at fixed k, with clamped second-expiry strike. Density uses shared Dupire kernel, strict matched-k time lookup, flags negative denominator or negative local variance when denominator>tol. All comparisons use configured tolerance (default1e−10). Magnitudes classify negligible<1e−8, minor<1e−5, major<1e−3, critical otherwise. Apply min_severity filter first, sort descending severity, then passed=true if no retained severity above Negligible.

## Selection and controls

All three checks default enabled; default config has no forward and therefore errors until populated. Low-level checker structs are public and weaker: empty/short grids return no violations; butterfly/calendar repeat last forward for missing expiry entries and skip nonpositive forwards; density requires exact count and positive finite forwards.

## Conventions

Magnitudes have different units: density g dimensionless, calendar total-variance decrease, local-variance negativity. Shared fixed thresholds do not normalize these to a single economic loss.

## Limitations, failure and fallback

Filtering can make passed true after removing real lower-severity violations. Thin grids/near-zero total variance are skipped; nonfinite low-level calculations can evade comparisons. Wings outside sampled points and interpolation between points are not certified. Declared SVI violation enum variants are reporting vocabulary, not checks automatically executed by check_surface.

## Alternatives and tradeoffs

Analytic parameter constraints/global price convexity tests would be alternatives. Top-level strict configuration is safer than direct low-level checker construction but both remain exposed.

## Implementation and validation evidence

[checks.rs](../../../../finstack-quant/models/src/volatility/arbitrage/checks.rs), [mod.rs](../../../../finstack-quant/models/src/volatility/arbitrage/mod.rs) and [types.rs](../../../../finstack-quant/models/src/volatility/arbitrage/types.rs). Inline constructed-violation and missing-forward tests establish reporting behavior; local-vol analytic SSVI evidence covers a related kernel, not all report classifications. References: [Static arbitrage](../../../REFERENCES.md#gatheral-jacquier-2014-svi), [Dupire](../../../REFERENCES.md#dupire-1994).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
