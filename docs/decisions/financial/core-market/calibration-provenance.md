# CORE-MARKET-06: Curve calibration recipe semantics and replay boundaries

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Core stores the recipe needed by downstream calibration/risk code without depending on that code. These files implement recipe typing, ordering and partial validation; no bootstrap or global inverse solver runs here. Storing provenance supports replay, but presence of a recipe is not proof that current curve values still fit it.

## Algorithm and controls

`RateCalibrationRecipe` carries currency, curve day count, method (`Bootstrap` or `GlobalSolve { use_analytical_jacobian }`), curve role (discount with projection-curve ID, or projection with discount-curve ID), optional OIS compounding, and quotes. OIS modes are Simple, CompoundedInArrears with lookback, observation-shift compounding, and cutoff compounding. Deposits, FRAs, futures, swaps and basis quotes carry typed date/tenor pillars and identifiers; futures carry price and optional convexity adjustment, swaps optional spread. Rates/spreads are decimal inputs. This is a public data struct; core does not validate quote finiteness, duplicate maturities, consistency of IDs, or solver feasibility here.

`HazardCalibrationRecipe` stores JSON hazard parameters/configuration and two input sets: original calibration quotes and par-spread risk quotes. Construction and deserialization independently sort each set by numeric pillar time, date, quote ID and finally serialized quote text. Both sets must be nonempty, have finite nonnegative strictly increasing times, nonempty IDs, and recognized `cds_par_spread` or `cds_upfront` types; the risk set must contain par-spread quotes only. Their lengths and each sorted position's quote ID must match. Dates/times across the paired sets are not directly checked for equality, and quote numeric fields/configuration remain opaque JSON at this layer.

Discount/forward builders and wire forms preserve attached rate recipes; their shocks, rolls and replacement-knot rebuilds can retain the same recipe. Hazard synthetic shocks, recovery changes and rolls clear their hazard recipe; a pure rename/rebuild preserves it. Recipe invalidation policy therefore differs between rate and hazard curves.

## Limitations, alternatives

No recipe field selects or overrides evaluation of an already-built core curve. Downstream consumers must distinguish original calibration provenance from a replay recipe representing the current curve, validate raw JSON, and resolve dependent curves. A cryptographic quote snapshot, typed hazard quote/config storage, or invalidation on all synthetic transforms would be possible design changes; none is adopted by this audit.

## Evidence

Owning code: [rate recipe](../../../../finstack-quant/core/src/market_data/term_structures/rate_calibration.rs), [hazard recipe](../../../../finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs), [forward wire](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/wire.rs), [hazard transforms](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), and [discount metadata rebuild](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs). Inspected `full_rate_calibration_retains_one_day_cutoff` in [forward tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) proves round-trip preservation, not calibration correctness. Inspected `hazard_recipe_invalidation_covers_synthetic_transformations` in [hazard tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) proves invalidation behavior. Recipe unit tests are located in their owning files. [ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) and [multi-curve bootstrapping](../../../REFERENCES.md#ametrano-bianchetti-2013) provide wider method context.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
