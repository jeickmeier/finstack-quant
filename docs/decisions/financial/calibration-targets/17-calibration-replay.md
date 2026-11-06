# CALIBRATION-TARGETS-017 — Rate and hazard calibration recipe semantics

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and distinction

Rate recipes preserve benchmark economics and curve-role metadata for recalibration. Hazard recipes preserve both original calibration quotes and a par-spread risk representation, along with resolved dates/times and serialized configuration. These sidecars have different completeness contracts; storing a recipe does not demonstrate the current transformed curve still fits those inputs.

## Rate recipe stages

1. Convert Bootstrap/GlobalSolve (including custom-Jacobian flag) and Tenor/Date pillars bidirectionally. Preserve deposit/FRA/swap numeric rates, index, swap spread, futures contract/expiry/price and explicit convexity adjustment. Futures adjustment is stored as Some; legacy/core recipes with None replay as zero.
2. Store currency, method, curve day count, optional OIS compounding, role and quotes. Discount role records projection-curve ID; projection role records discount-curve ID. Numerical solver config, acceptance/weighting policy, interpolation/extrapolation and original QuoteIds are **not fields of this rate recipe**; their reconstruction belongs to the owning replay adapter/current curve.
3. Supported OIS compounding maps Simple, in-arrears lookback, observation shift and rate cutoff. SimpleAverage errors as unsupported for rate replay; it is not silently downgraded. Reverse mapping reconstructs the exact supported variant.
4. Replay reconstructs quote IDs as `<curve_id>-REPLAY-<index>` and restores quote shape/fields. Core `Basis` recipe quotes error because they require the dedicated basis replay path. Identifier provenance is regenerated rather than losslessly retained.

## Hazard recipe stages

1. Retain each original CDS JSON quote and exact resolved pillar date/time. Convert upfront quotes to model-implied par spreads on the calibrated market for spread-risk inputs; preserve par quotes directly. This permits bumping spread risk without changing contractual upfront running coupons.
2. Store params and full config JSON plus both aligned input collections. Core recipe construction validates ordering/identity/type constraints. Calibration-side `validate_hazard_recipe_bindings` decodes every original and risk quote, resolves compatible CDS conventions, rebuilds the quote under stored params/clock and requires exact pillar date and relative time tolerance `1e-12*max(abs(rebuilt),abs(stored),1)`.
3. The returned HazardCurve carries par points and recipe. Core transformations may invalidate recipes; [core risk/provenance](../core-market/calibration-provenance.md) and framework recalibration records own that lifecycle.

## Limits, alternatives and evidence

Rate recipes can preserve contractual quotes while using different future numerical policies; hazard JSON retains more solver policy but still depends on convention registry/pricer semantics and serialized schema validity. Neither format is a complete external market-data lineage or independent fit certificate. Alternatives include immutable convention/config hashes, original market observation IDs/timestamps, or a common typed replay contract with explicit exclusions. Those are adoption decisions, not a reason to invent a compatibility layer.

- Owning [rate_recipe.rs](../../../../finstack-quant/calibration/src/targets/rate_recipe.rs) and hazard recipe portions of [hazard.rs](../../../../finstack-quant/calibration/src/targets/hazard.rs) inspected.
- Located mixed-rate quote-shape preservation test in [discount.rs](../../../../finstack-quant/calibration/src/targets/discount.rs), absolute-date forward replay test, [hazard ACT365F/tampering tests](../../../../finstack-quant/calibration/src/hazard_curve_tests.rs), and upfront-to-par replay in [repricing_tests.rs](../../../../finstack-quant/calibration/src/repricing_tests.rs). These are serialization/rebuild/roundtrip evidence, not independent quote-to-price proof.

No replay tests were executed here.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
