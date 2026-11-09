# CALIBRATION-TARGETS-015 — Tranche upfront bootstrap to monotone base correlations

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and units

Sequentially fit a base-correlation curve by tranche detachment, using complete contractual tranche PV divided by **tranche notional**. Recovery/index hazard and loss-distribution/pricing methodology are supplied by the existing credit index and CdsTranchePricer. Increasing detachment is the bootstrap coordinate, not time; each run is one fixed contractual maturity/series.

## Algorithm

1. Extract tranche quotes. Require exact index ID and series. Parameter detachment points, if supplied, must be finite decimals in `(0,1]`; convert to percentage points and require every quote detachment belongs to the expected set within `1e-8`, and every expected point appears. General quote validation enforces attachment<detachment and upfront/coupon domains.
2. Require maturity_years to represent a positive whole number of months (month tolerance `1e-8`). Resolve the exact expected maturity from base date plus months for RollRule None, or CDS IMM tenor logic for CdsImm; other rolls error. Quote maturity must equal it. Resolve convention and build full tranche with frequency/day-count/BDC/calendar/roll overrides. Retain contractual upfront so settlement discounting remains in PV.
3. Bootstrap by detachment percentage using no origin seed. Candidate correlations must be finite in `[0,.999]`; initial guess/lower scan bound is previous correlation or zero. Scan includes endpoints, six linear subdivisions of `[previous,.999]`, plus ±`{.0001,.0005,.001,.005,.01,.05}` around an interior center (`min(previous+.30,.85)` below .5; midpoint above). Deduplicate at `1e-12`; nearest-first bracketing is enabled.
4. Trial curve sorts/deduplicates detachment knots and validates `(0,100]` detachment and `[0,1]` correlation. If only one knot exists, synthesize a second equal-correlation point ten percentage points above if possible, otherwise below (with a final one-point fallback); this permits the two-point core curve. Output ID is `<index_id>_CORR`.
5. Insert candidate curve and rebind the index to it. Residual is full `price_tranche(...).amount()/tranche.notional.amount()`, including settlement upfront. Hardcoded acceptance budget is `1e-3` discounted upfront fraction, i.e. 10 bp of tranche notional. Approximate no-bracket knots are allowed only under framework fit validation and are flagged in report metadata.
6. Final shape validation requires nondecreasing correlations; failure errors. Return both the curve and a refreshed credit-index object with its correlation dependency updated. The curve's monotonicity diagnostic is an implementation policy, not a complete tranche no-arbitrage theorem.

## Alternatives and limitations

A parameter-space monotone constraint can prevent matching some input tranche markets. Approximate knot acceptance explicitly permits bounded residuals, not a claim that all quotes have an exact model root. Nearest-first search requires qualification of residual shape/uniqueness for the supported attachment/detachment/loss distributions; it is not justified merely by parameter monotonicity. The synthetic second knot in a one-quote result carries no additional observation.

Alternatives include unconstrained per-tranche compound correlation, global base-correlation fits, constrained expected-loss surfaces, maturity-dependent base-correlation surfaces, and basket/correlation joint fitting. Fixed hazards/recoveries and a single maturity should be kept explicit.

## Evidence

- Owning [base_correlation.rs](../../../../finstack-quant/calibration/src/targets/base_correlation.rs), [tranche construction](../calibration-framework/08-tranche-materialization.md), [core correlation representation](../core-market/base-correlation.md).
- Inline index-refresh, maturity/roll, synthetic-knot and final monotonicity tests are located. [Base-correlation integration tests](../../../../finstack-quant/calibration/tests/calibration/base_correlation.rs) build model-generated settlement upfront quotes and reject thin-tranche mispricing over 10 bp; their same-pricer roundtrip role is explicit.

No execution or external model certification occurred.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires strict ordered canonical detachment-percent/rho data and permits an
explicit constant singleton. Remove synthetic second knots and tolerance-based
source deduplication. Fraction-valued quote inputs convert to percent exactly
once; retain actual maturity/series/pool/hazard/recovery/loss-model provenance.
A monotone-fit constraint and restricted solver bounds are calibration choices,
not core source admission or a tranche no-arbitrage theorem. Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's solver, fit constraints/budgets, quote
selection or complete publication methodology.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires published CreditIndexData to retain explicit pool composition, complete
Named effective inputs, exact reference bindings and financial construction
association. Refreshed composites must preserve mode, membership, cash recovery
and weights when replacing correlation/hazard sources. Typed context restoration
is structural evidence, not new source fit authority. Record 19 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's bootstrap, monotone-fit constraints, quote
selection or acceptance budgets.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
