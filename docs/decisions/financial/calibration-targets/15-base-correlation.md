# CALIBRATION-TARGETS-015 — Sequential base-correlation calibration from tranche upfront quotes

- **Decision status:** Accepted 2026-11-15 — complete dated base-correlation calibration and publication package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent attachment, full-detachment and tranche-loss calculations verified. Production calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for identified base-correlation bootstrap and atomic publication

1. **Freeze one dated tranche market per fit.** Select exact quote IDs for
   one index, series, version, currency, literal contractual maturity and
   valuation cut. Prepare buyer-side terms, seasoning, dated cashflows and
   upfront settlement under the accepted tranche contract. Freeze pool
   weights, hazards, recoveries, discounting and the elected copula and
   loss model. Convert original-pool attachment and detachment fractions
   to percent once. Reject represented coordinate collisions. Retire
   inferred tenor and tolerance-based detachment matching.
   [Accepted tranche preparation](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement).

2. **Fit one identifying quote per detachment in ascending order.**
   Require zero attachment or an exact previously fitted attachment
   and hold its correlation fixed throughout the next solve. A 100%
   detachment cannot identify its own correlation with fixed marginals:
   its loss cap gives the uncapped marginal mean and its recovery cap
   is zero. Reject exhausted, deterministic and numerically insensitive
   fit rows.
   [Accepted tranche loss geometry](../models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).

3. **Require an identified root on an explicit bounded branch.**
   Declare model-supported correlation bounds, seed, scan points,
   coordinate tolerance and work limits. Reuse the shared bounded
   search and checked Brent solver on a continuous feasible branch.
   Require a computed zero or sign-bracketed result tied to the
   returned candidate, with local residual sensitivity resolved
   against pricing allowances. Reject ambiguous branches and
   unbracketed approximate knots. Retire the hardcoded `.999`
   ceiling and nearest-seed root selection.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence).

4. **Make correlation ordering an explicit fit choice.** Elect no
   ordering constraint or a nondecreasing fit. For a nondecreasing
   fit, the preceding correlation raises the lower bound for the
   next solve and equality is allowed. Preserve actual drops where
   admitted. Apply the accepted model's loss, recovery erosion and
   outstanding checks on the actual dated pricing path independently
   of curve shape. Report actual coverage. Retire shape repair.
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Brigo, Pallavicini and Torresetti, §3](https://arxiv.org/html/0912.5427v3#S3).

5. **Gate the proposed knot and the completed curve in native units.**
   Use the signed buyer residual `(P − c A_dirty − B C) / N₀`.
   Include dated upfront settlement on the accepted basis and divide
   by original tranche notional. Require an explicit positive native
   tolerance for each quote. Keep it distinct from coordinate tolerance
   and operational pricing allowances. Reprice the new knot on the
   private prefix and every consumed quote on the exact final curve.
   Preserve authored report order. Retire the fixed 10 bp budget.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

6. **Publish only fitted knots.** Use the canonical ordered
   detachment-percent source with checked linear interpolation and
   flat tails. Publish a genuine constant singleton for a one-knot
   fit. Preserve the actual fitted extent in provenance.
   [Accepted base-correlation source](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks).

7. **Publish the curve and rebound index atomically.** Require an
   explicit output ID. Rebind only the correlation dependency and
   preserve exact pool composition, state, hazards and recovery
   roles. Retain quote identities, root evidence, actual work, local
   and final residuals, and typed failures through Rust-owned
   construction and restoration. Failed fits may retain diagnostics.
   Only the admitted pair may enter the market or qualify for replay.
   [Accepted pool composition](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts) ·
   [Accepted curve admission](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

## Independent calculation checks

- A fitted singleton of `.20` at 3% gives `.20` at 5%.
  Appending a 7% node at `.60` changes linear lookup at 5%
  to `.40`. A 5% attachment without its own fitted knot would
  move as the 7% trial changes.
- For a tranche with 100% detachment and fixed attachment
  correlation, the detachment-side loss equals the marginal
  expected pool loss and its recovery cap is zero. Varying the
  detachment correlation leaves both terms unchanged.
- For two equally weighted zero-recovery Gaussian names, each with
  default probability `.5`, capped equity losses at 30% with
  rho `0` and at 32% with rho `.5` are `9/40` and `16/75`
  of pool notional. The implied layer loss is `−7/600` of pool
  notional despite the rising correlation.

## Observed choice and units at the audit revision

[CALIBRATION-FRAMEWORK-008](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement)
provides the qualified literal tranche maturity, explicit dated terms,
quote settlement and raw buyer fit basis per original tranche notional
for the accepted package above.

Sequentially fit a base-correlation curve by tranche detachment, using complete contractual tranche PV divided by **tranche notional**. Recovery/index hazard and loss-distribution/pricing methodology are supplied by the existing credit index and CdsTranchePricer. Increasing detachment is the bootstrap coordinate, not time; each run is one fixed contractual maturity/series.

## Observed algorithm

1. Extract tranche quotes. Require exact index ID and series. Parameter detachment points, if supplied, must be finite decimals in `(0,1]`; convert to percentage points and require every quote detachment belongs to the expected set within `1e-8`, and every expected point appears. General quote validation enforces attachment<detachment and upfront/coupon domains.
2. Require maturity_years to represent a positive whole number of months (month tolerance `1e-8`). Resolve the exact expected maturity from base date plus months for RollRule None, or CDS IMM tenor logic for CdsImm; other rolls error. Quote maturity must equal it. Resolve convention and build full tranche with frequency/day-count/BDC/calendar/roll overrides. Retain contractual upfront so settlement discounting remains in PV.
3. Bootstrap by detachment percentage using no origin seed. Candidate correlations must be finite in `[0,.999]`; initial guess/lower scan bound is previous correlation or zero. Scan includes endpoints, six linear subdivisions of `[previous,.999]`, plus ±`{.0001,.0005,.001,.005,.01,.05}` around an interior center (`min(previous+.30,.85)` below .5; midpoint above). Deduplicate at `1e-12`; nearest-first bracketing is enabled.
4. Trial curve sorts/deduplicates detachment knots and validates `(0,100]` detachment and `[0,1]` correlation. If only one knot exists, synthesize a second equal-correlation point ten percentage points above if possible, otherwise below (with a final one-point fallback); this permits the two-point core curve. Output ID is `<index_id>_CORR`.
5. Insert candidate curve and rebind the index to it. Residual is full `price_tranche(...).amount()/tranche.notional.amount()`, including settlement upfront. Hardcoded acceptance budget is `1e-3` discounted upfront fraction, i.e. 10 bp of tranche notional. Approximate no-bracket knots are allowed only under framework fit validation and are flagged in report metadata.
6. Final shape validation requires nondecreasing correlations; failure errors. Return both the curve and a refreshed credit-index object with its correlation dependency updated. The curve's monotonicity diagnostic is an implementation policy, not a complete tranche no-arbitrage theorem.

## Observed alternatives and limitations

A parameter-space monotone constraint can prevent matching some input tranche markets. Approximate knot acceptance explicitly permits bounded residuals, not a claim that all quotes have an exact model root. Nearest-first search requires qualification of residual shape/uniqueness for the supported attachment/detachment/loss distributions; it is not justified merely by parameter monotonicity. The synthetic second knot in a one-quote result carries no additional observation.

Alternatives include unconstrained per-tranche compound correlation, global base-correlation fits, constrained expected-loss surfaces, maturity-dependent base-correlation surfaces, and basket/correlation joint fitting. Fixed hazards/recoveries and a single maturity should be kept explicit.

## Evidence

- Owning [base_correlation.rs](../../../../finstack-quant/calibration/src/targets/base_correlation.rs), [tranche construction](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement), [core correlation representation](../core-market/base-correlation.md).
- Inline index-refresh, maturity/roll, synthetic-knot and final monotonicity tests are located. [Base-correlation integration tests](../../../../finstack-quant/calibration/tests/calibration/base_correlation.rs) build model-generated settlement upfront quotes and reject thin-tranche mispricing over 10 bp; their same-pricer roundtrip role is explicit.

No execution or external model certification occurred.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires strict ordered canonical detachment-percent/rho data and permits an
explicit constant singleton. Remove synthetic second knots and tolerance-based
source deduplication. Fraction-valued quote inputs convert to percent exactly
once; retain actual maturity/series/pool/hazard/recovery/loss-model provenance.
The accepted calibration package above owns solver bounds, elected ordering,
quote completeness and financial admission. The source record owns structure,
lookup and shocks.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires published CreditIndexData to retain explicit pool composition, complete
Named effective inputs, exact reference bindings and financial construction
association. Refreshed composites must preserve mode, membership, cash recovery
and weights when replacing correlation or hazard sources. The accepted target
binds the atomic output pair and its fit evidence to those exact inputs.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
