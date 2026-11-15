# CALIBRATION-TARGETS-017 — Rate and hazard calibration recipe semantics

- **Decision status:** Accepted 2026-11-15 — complete rate and hazard construction, replay and qualification package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent coordinate identity, evaluator, futures convexity and CDS quote-basis calculations verified. Production replay, market benchmarks, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for exact rate and hazard construction and qualified replay

1. **Capture one immutable effective construction record.** Preserve
   canonical quote IDs and forms, authored and executed order, prepared
   contracts, clocks, resolved conventions and calendars, fixing
   information and typed dependency state. Record the actual solver,
   weights, tolerances, evaluator representation, interpolation, tails
   and fit report. Core transports the record. Calibration strictly
   decodes and validates its supported executable meaning. Retire the
   mirrored rate quote types and reconstruction from current defaults
   or registry entries.
   [Accepted construction provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

2. **Preserve native rate quote forms and contractual profiles.**
   Retain decimal rates, swap spreads, futures IMM prices and explicit
   signed convexity. Keep original date and tenor requests, resolved
   cash dates, futures reference ends and numerical pillars distinct.
   Retire generated quote IDs and missing-as-zero convexity. Use the
   eligible instrument's actual observation law, including arithmetic
   averaging for qualified futures and the elected overnight swap
   compounding rule. Require the applicable qualified binding for
   linked OIS and basis replay.
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication) ·
   [CME One-Month SOFR futures](https://www.cmegroup.com/markets/interest-rates/stirs/one-month-sofr/specs).

3. **Bind original CDS quotes and par-risk inputs together.** Bind one
   original quote and risk view to the same original identity, entity,
   currency, terms, recovery, contractual maturity and canonical
   represented clock coordinate. Keep original par literal. Retain
   coupon, signed upfront and settlement basis on the original upfront
   row. Derive its par-risk spread on the qualified source fit with
   a stated clean or dirty basis. Preserve a valid derived zero and
   enforce positive-par admission when a requested spread replay
   requires that row. Retire independent sorting, ID-only pairing and
   the blanket `1e-12` tolerance for time identity.
   [Accepted hazard replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).

4. **Qualify the original source before each replay.** Reconstruct the
   unshocked fit on its explicitly bound immutable source market,
   including for zero moves. Require exact contractual and prepared
   identities and reproduction of the complete source evaluator.
   Apply owner-qualified numerical tolerances to fit and evaluator
   reproduction. Verify the actual representation, clock, roles,
   dependencies, interpolation and tails as well as fitted values.
   [Accepted source qualification](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

5. **Use the quote basis of the selected action.** Rate shocks target
   exact selected quotes in their native units. Hazard spread shifts
   and their center use the qualified par-risk inputs. Dependency
   changes and recovery refits preserve original CDS quote forms and
   derive a fresh par-risk view after admission. Deal overrides and
   model-derived horizon fits use their separately qualified contracts.
   [Accepted hazard actions](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication) ·
   [Accepted deal and horizon calibration](../calibration-framework/22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay).

6. **Publish the admitted target refit.** Solve on the explicitly
   bound target market, reprice every consumed quote and apply the
   output gates. Publish the delivered evaluator, new record and fit
   report together, including dependent updates where required. Retire
   curve overlays and fallback success. A zero quote move with
   changed dependencies requires a target fit.
   [Accepted target publication](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

7. **Preserve history through transforms and restoration.** Bind replay
   qualification to the exact request. Changed evaluator state,
   bindings or policy require fresh qualification or explicitly
   requested new calibration. Cache only admitted results under the
   exact request identity. Preserve typed errors through existing
   Rust and host surfaces. Provenance getters report available
   construction evidence.
   [Accepted provenance lifecycle](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted request-local cache](../calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

## Independent calculation checks

- At 98.50 IMM points, a +2.5 bp convexity adjustment gives a projected
  rate seed of 1.475%. Substituting zero gives 1.500%.
- The represented ACT/365F time for 91 days differs from its binary64
  successor by about `2.77556e-17`. The observed time tolerance admits
  both values as the same binding.
- Discount knots of `.96` and `.90` give a midpoint factor of `.93`
  with linear discount-factor interpolation and `.9295160031` with
  log-linear interpolation.
- In a one-period CDS example with unit discount, no default accrual
  and a 100 bp coupon paid on survival, 10% default probability and
  40% recovery imply 5.10% upfront and a par rate of `1/15`.
  Changing recovery to 20% implies default probability `61/810`
  when refitting the original upfront and coupon, and `1/13` when
  holding the derived par rate.

## Relationship to accepted construction and replay boundaries

[CORE-MARKET-06](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification)
was accepted on 2026-11-08: lossless effective construction records, original
quote identity/resolved bindings, explicit source/target state and request-time
replay qualification distinct from historical provenance. The accepted
package above reconciles the target recipe contracts to that boundary.
The relevant target and framework records own solver, materialization,
fit and horizon-calibration methodology. Accepted
[CORE-MARKET-05](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery)
already requires explicit quote risk, exact targets and separate recovery operations.

## Observed choice and distinction at the audit revision

Rate recipes preserve benchmark economics and curve-role metadata for recalibration. Hazard recipes preserve both original calibration quotes and a par-spread risk representation, along with resolved dates/times and serialized configuration. These sidecars have different completeness contracts; storing a recipe does not demonstrate the current transformed curve still fits those inputs.

## Observed rate recipe stages

1. Convert Bootstrap/GlobalSolve (including custom-Jacobian flag) and Tenor/Date pillars bidirectionally. Preserve deposit/FRA/swap numeric rates, index, swap spread, futures contract/expiry/price and explicit convexity adjustment. Futures adjustment is stored as Some; legacy/core recipes with None replay as zero.
2. Store currency, method, curve day count, optional OIS compounding, role and quotes. Discount role records projection-curve ID; projection role records discount-curve ID. Numerical solver config, acceptance/weighting policy, interpolation/extrapolation and original QuoteIds are **not fields of this rate recipe**; their reconstruction belongs to the owning replay adapter/current curve.
3. Supported OIS compounding maps Simple, in-arrears lookback, observation shift and rate cutoff. SimpleAverage errors as unsupported for rate replay; it is not silently downgraded. Reverse mapping reconstructs the exact supported variant.
4. Replay reconstructs quote IDs as `<curve_id>-REPLAY-<index>` and restores quote shape/fields. Core `Basis` recipe quotes error because they require the dedicated basis replay path. Identifier provenance is regenerated rather than losslessly retained.

## Observed hazard recipe stages

1. Retain each original CDS JSON quote and exact resolved pillar date/time. Convert upfront quotes to model-implied par spreads on the calibrated market for spread-risk inputs; preserve par quotes directly. This permits bumping spread risk without changing contractual upfront running coupons.
2. Store params and full config JSON plus both aligned input collections. Core recipe construction validates ordering/identity/type constraints. Calibration-side `validate_hazard_recipe_bindings` decodes every original and risk quote, resolves compatible CDS conventions, rebuilds the quote under stored params/clock and requires exact pillar date and relative time tolerance `1e-12*max(abs(rebuilt),abs(stored),1)`.
3. The returned HazardCurve carries par points and recipe. Core transformations may invalidate recipes; [core risk/provenance](../core-market/calibration-provenance.md) and framework recalibration records own that lifecycle.

## Observed limits, alternatives and evidence

Rate recipes can preserve contractual quotes while using different future numerical policies; hazard JSON retains more solver policy but still depends on convention registry/pricer semantics and serialized schema validity. Neither format is a complete external market-data lineage or independent fit certificate. Alternatives include immutable convention/config hashes, original market observation IDs/timestamps, or a common typed replay contract with explicit exclusions. Those are adoption decisions, not a reason to invent a compatibility layer.

- Owning [rate_recipe.rs](../../../../finstack-quant/calibration/src/targets/rate_recipe.rs) and hazard recipe portions of [hazard.rs](../../../../finstack-quant/calibration/src/targets/hazard.rs) inspected.
- Located mixed-rate quote-shape preservation test in [discount.rs](../../../../finstack-quant/calibration/src/targets/discount.rs), absolute-date forward replay test, [hazard ACT365F/tampering tests](../../../../finstack-quant/calibration/src/tests/hazard_curve.rs), and upfront-to-par replay in [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs). These are serialization/rebuild/roundtrip evidence, not independent quote-to-price proof.

No replay tests were executed here.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
