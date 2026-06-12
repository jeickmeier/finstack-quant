# Scenarios Crate & Bindings — Quant Finance Review

**Date:** 2026-06-12
**Scope:** `finstack/scenarios` (spec, engine, all adapters, horizon, templates incl. embedded JSON data), `finstack-py/src/bindings/scenarios/` (mod, engine, horizon, operation_spec), `finstack-wasm/src/api/scenarios/`, parity contract section.
**Method:** Full read of spec.rs, engine.rs, utils.rs, horizon.rs, warning.rs, traits.rs, all adapters (curves, vol, fx, time_roll, statements, instruments, basecorr, equity), templates/registry.rs + the five embedded template JSONs, and both binding surfaces. Cross-crate verification of shock consumers in `finstack-valuations` (pricing_overrides, calibration/bumps) via grep + targeted reads. Every finding cites file:line. No code changes; tests were not executed.
**Status:** Findings reported, **remediation not started**.

---

## Findings

### Blockers

None. Nothing corrupts an on-pillar, market-only scenario (the dominant use case), and the failure modes below all surface warnings or errors — but the three Majors share a theme: operations that *report* success while delivering less than requested.

---

### Majors

#### M-1. Instrument spread shocks never affect valuation — including in a shipped template

**Location:** `finstack/scenarios/src/adapters/instruments.rs:153-163` (`ShockKind::Spread` arm of `apply_shock`); consumer absence verified in `finstack/valuations/src/instruments/pricing_overrides.rs:733-759`.
**Issue:** Spread shocks have no first-class pricing path. Every `InstrumentSpreadBpByType` / `InstrumentSpreadBpByAttr` unconditionally writes `scenario_spread_shock_bp` into instrument `Attributes.meta` as a string and emits `Warning::InstrumentShockFallback`. A workspace-wide grep finds **no consumer** of that metadata key: `ScenarioPricingOverrides` carries only `scenario_price_shock_pct`, and no pricer in `finstack-valuations` reads `scenario_spread_shock_bp`. The shock is a PV no-op, yet `operations_applied` still increments, and the tests (`tests/shocks/instrument_shocks_test.rs:174-182`) only assert the metadata write — nothing asserts pricing impact.
**Impact:** Spread-widening stress silently understates P&L for anyone not auditing warnings. The shipped **`svb_2023` builtin template's entire credit component** (`data/templates/svb_2023.json`, `instrument_spread_bp_by_attr` on `sector=regional_banks` +150bp) changes no valuation — a historical stress template whose credit leg does nothing. The warning text is honest ("will not affect valuation unless the downstream consumer reads that metadata"), but a stress operation in the stable serde wire contract that cannot move a price is a missing market-standard feature.
**Fix:** Add `scenario_spread_shock_bp` to `ScenarioPricingOverrides` and apply it in credit-sensitive pricers (z-spread or hazard bump), or reject these operations with a typed error until supported. Re-spec `svb_2023`'s credit leg as a `curve_parallel_bp` on a regional-banks `par_cds` curve. Add a test asserting PV impact (not just metadata).

#### M-2. Off-pillar node bumps deliver only `(w0² + w1²)` of the requested bp at the target tenor

**Location:** `finstack/scenarios/src/utils.rs:275-331` (`calculate_interpolation_weights`) consumed by `finstack/scenarios/src/adapters/curves.rs:104-131` (`resolve_bump_targets`, `TenorMatchMode::Interpolate`); discount path confirmed through `finstack/valuations/src/calibration/bumps/rates.rs:380-397` (`BumpRequest::Tenors` bumps the closest synthetic par quote per target).
**Issue:** A bump at off-pillar tenor *t* is split between adjacent pillars with linear weights `w0 + w1 = 1`. Under linear interpolation, the realized curve move at *t* is then `bp · (w0² + w1²)`: a +50bp request at 3Y on a {2Y, 4Y} curve moves the 3Y rate by only **+25bp**. The spec doc calls this "key-rate bump at interpolated time" (`spec.rs:793-794`), which a desk reads as "the curve at that tenor moves by the full bp". The behavior is codified nowhere: `tests/shocks/tenor_shocks_test.rs` only asserts "DF changed", not the magnitude. On-pillar requests are unaffected (the weight degenerates to 1.0 at a knot), which is why the shipped templates (2Y/5Y/10Y/30Y nodes on standard curves) are unlikely to hit it.
**Impact:** User-requested off-pillar key-rate stresses silently deliver as little as half the requested shock — materially wrong scenario risk with no warning. Affects all curve kinds routed through `resolve_bump_targets` (discount, forward, ParCDS, inflation, commodity, vol-index node bumps).
**Fix (pick one):** (a) insert a new knot at the requested tenor carrying the full bump — cleanest key-rate semantics; (b) rescale weights by `1/(w0² + w1²)` so the interpolated point realizes the full bump; (c) explicitly document the allocation semantics and emit a warning whenever the requested tenor is off-pillar. Add a magnitude-pinning test either way.

#### M-3. The engine discards the `RollForwardReport`, including valuation failures

**Location:** `finstack/scenarios/src/engine.rs:812` (Phase 0 drops the return of `apply_time_roll_forward`); report contents built in `finstack/scenarios/src/adapters/time_roll.rs:221-285`.
**Issue:** `apply_time_roll_forward` computes per-instrument carry (two full valuations per instrument: t0 and t1) and a `failed_instruments` list (instruments whose t0/t1 valuation errored during the roll). When invoked through `ScenarioEngine::apply`, the entire report is discarded.
**Impact:** (a) Valuation failures during an engine-driven roll vanish — no `Warning` reaches `ApplicationReport`, violating the fail-loudly expectation for a stress system; (b) the carry computation is pure wasted compute (2 PVs × N instruments) on the engine path. The carry decomposition is only available via the standalone `apply_time_roll_forward` public function.
**Fix:** Map `failed_instruments` into `Warning` variants appended to `ApplicationReport`. Either surface carry in the report or skip the carry computation entirely when invoked from `apply` (instruments-supplied contexts only).

---

### Moderates

#### MO-1. `rounding_context` stamps the default, not the active policy

**Location:** `finstack/scenarios/src/engine.rs:29-34` (`rounding_stamp`).
**Issue:** Formats `RoundingMode::default()` unconditionally. A caller running under a non-default `FinstackConfig` rounding mode gets the wrong policy stamped — contradicting the workspace invariant that the *active* `RoundingContext` is stamped into result envelopes.
**Fix:** Thread the active config through `ExecutionContext`, or stamp nothing rather than a potentially false value.

#### MO-2. Mid-apply errors leave the market partially mutated, undocumented

**Location:** `finstack/scenarios/src/engine.rs:775-1024` (`apply`); unknown-id bump rejection confirmed in `finstack/core/src/market_data/context/ops_bump.rs:123`.
**Issue:** `apply` mutates `ctx.market` in place across operations (curve replacements via `UpdateCurve`, batched bumps flushed between ops). A later op failing — `MarketDataNotFound`, or `MarketContext::bump` rejecting an unknown id — returns `Err` with earlier shocks already applied. No rollback, and the docs do not mention non-atomicity. Python/WASM callers are insulated (they operate on deserialized copies and serialize only on success); Rust callers passing a live `&mut MarketContext` get a half-applied scenario.
**Fix:** Document non-atomicity prominently in `apply`'s docs, or stage on a clone and swap on success.

#### MO-3. Hierarchy expansion does not filter resolved ids by kind

**Location:** `finstack/scenarios/src/engine.rs:374-470` (`expand_hierarchy_operations`).
**Issue:** Expansion maps every `curve_id` in the matched hierarchy subtree into the operation's kind (`HierarchyCurveParallelBp{Discount}` → one `CurveParallelBp` per id). The same hierarchy `curve_ids` collection serves discount curves, vol surfaces, *and* equity price ids (the equity variant resolves from it too), so a node grouping mixed content turns a taxonomy mismatch into a hard `MarketDataNotFound` abort mid-apply (compounding MO-2). Inconsistent failure modes: direct equity ops warn-and-skip on missing ids (`adapters/equity.rs:33`), while hierarchy-expanded curve ops hard-error — wrong way around for machine-derived ids that the user never typed.
**Fix:** Filter resolved ids by target collection (kind) at expansion time, or warn-and-skip on missing ids for hierarchy-expanded operations instead of aborting.

#### MO-4. Bindings cannot supply instruments, calendar, or rate bindings; rounding stamp dropped in Python

**Location:** `finstack-py/src/bindings/scenarios/engine.rs:35-42`, `finstack-wasm/src/api/scenarios/mod.rs:32-39`; report dict built at `finstack-py/src/bindings/scenarios/engine.rs:18-26`.
**Issue:** Both bindings hardcode `instruments: None, rate_bindings: None, calendar: None`. From Python/WASM: all instrument price/spread and correlation shock operations warn-and-noop (`InstrumentShockNoPortfolio` / `CorrelationShockNoPortfolio`); `TimeRollForward` in `BusinessDays` mode never sees a holiday calendar (ModifiedFollowing degrades to weekend-only adjustment); `RateBinding` persistence is unavailable. Additionally `set_report_items` omits `rounding_context`, so the policy stamp never reaches Python.
**Fix:** Accept optional calendar id and instruments JSON in the binding entry points; surface `rounding_context` in the report dict; at minimum, document in the binding docstrings which operations are inert without a portfolio.

#### MO-5. `compute_horizon_return` holds the GIL and strips currency

**Location:** `finstack-py/src/bindings/scenarios/horizon.rs:36-95` (no `py.detach`, unlike `apply_scenario`); getters at `horizon.rs:125-132`.
**Issue:** Horizon analysis (multiple revaluations + rayon-parallel attribution) runs with the GIL held, blocking other Python threads for the duration. Separately, the `initial_value` / `terminal_value` getters return bare `f64` amounts with no currency, and `total_return_pct` can return `NaN` on currency mismatch with no Python-side documentation of that sentinel.
**Fix:** Wrap the compute in `py.detach`; return `(amount, currency)` or document that `to_json()` is the currency-faithful accessor; document the NaN sentinel.

---

### Minors

#### MI-1. Vol-index parallel shock silently clamps spot to zero

`finstack/scenarios/src/adapters/curves.rs:655` — knot levels are validated strictly positive post-shock (hard error via `check_vol_index_post_shock_positivity`), but `spot_level` is silently floored at `0.0`. A zero vol-index spot is as degenerate as a zero knot; make the spot check consistent with the knot check.

#### MI-2. Hazard fallback shift ignores the credit triangle

`finstack/scenarios/src/adapters/curves.rs:328-357` — when par-CDS recalibration fails, the fallback (`bump_hazard_shift`) shifts hazard rates by the raw spread bp. The standard approximation is `Δλ ≈ Δs/(1−R)`, so the fallback under-shocks default intensities by ~(1−R) (≈40% for senior unsecured). The `HazardRecalibrationFallback` warning is honest about additive-shift semantics but not the magnitude bias. Scale the fallback by `1/(1−R)` or state the bias in the warning.

#### MI-3. Template labeling anachronisms

`data/templates/{ltcm_1998,gfc_2008,covid_2020}.json` reference `USD-SOFR` (SOFR launched April 2018). The ids are overridable placeholders (`ScenarioSpecBuilder::override_curve`), but the metadata should note that ids are placeholders, not historical instruments. Calibration magnitudes are otherwise defensible (GFC: −200bp rates, IG +300/HY +800, SPX −50%, vol +200%, GBP −25%; COVID: −150bp, SPX −34%; 2022: +300bp, NDX −33%, EUR −15%; LTCM: RUB −50%).

#### MI-4. Discount-curve heuristic depends on a 3-char uppercase prefix

`finstack/scenarios/src/adapters/curves.rs:219-249` — ids like `sofr-usd` or `OIS-USD` silently bypass the currency-prefix heuristic into single-curve fallback or error. Warned via `DiscountCurveHeuristic`, but brittle; document the naming assumption.

#### MI-5. `update_rate_from_binding` lookup shadowing

`finstack/scenarios/src/adapters/statements.rs:122-145` — if a curve id exists as both a discount and a forward curve, the discount silently wins. Document or disambiguate.

---

## Open Questions / Assumptions

- **M-2 intent.** The under-delivery was treated as unintended because the spec doc says "key-rate bump at interpolated time" and no test pins the magnitude. If the intent is DV01-style *allocation* to adjacent buckets (the sum of pillar bumps does equal the requested bp), this downgrades to a documentation fix — but a stress engine applying shocks, not bucketing risk, should deliver the full move at the requested tenor.
- **Carry vs. realized-forward roll consistency.** `calculate_instrument_pnl` prices t1 against the *un-rolled* curve (base date still t0). Whether that is exactly consistent with the realized-forward roll semantics depends on the still-open `roll_forward` DF decision from the 2026-06-09 core review; the `apply_time_roll_forward_realizes_discount_forwards` test suggests coherence for discount curves, so it was not flagged.
- **Coverage limits.** All adapters, engine, spec, utils, horizon, warning, templates registry + JSON data, and both binding surfaces were read in full; `templates/builder.rs`, `templates/json.rs`, `templates/metadata.rs`, and `error.rs` were covered via their tests and call sites only. No tests were executed (review-only; no code changes).

---

## Summary

This is a well-engineered crate. The spec layer has disciplined unit conventions (bp vs vol-points vs correlation-points are separate variants, with validation that catches unit confusion such as Δρ outside [−2, 2]), the engine's phase ordering and bump-batching with conflict-flushing is carefully reasoned, warnings are structured rather than stringly, hierarchy resolution-mode semantics are explicit, and the arbitrage/triangulation preview checks on vol and FX shocks are above-average production hygiene. The three Majors share a theme: operations that *report* success while delivering less than requested — spread shocks that don't price (M-1), off-pillar bumps that halve (M-2), and roll-forward failures that vanish (M-3). None corrupts an on-pillar, market-only scenario, which is why nothing is graded Blocker, but all three would mislead a stress-P&L consumer who trusts `operations_applied`.

---

## Quant Notes

- **Key-rate shift semantics:** the standard reference is the triangular key-rate decomposition (Ho, "Key Rate Durations: Measure of Interest Rate Risks", *Journal of Fixed Income*, 1992) — but Ho's triangles are for *risk bucketing*; applying a *stress* at a tenor should reproduce the full shift at that point, which is M-2's crux.
- **Credit triangle:** the fallback scaling `Δλ ≈ Δs/(1−R)` is the flat-hazard approximation (O'Kane, *Modelling Single-name and Multi-name Credit Derivatives*, 2008, ch. 4); using it in the recalibration fallback would keep fallback CS01 within a few percent of the solved path for flat curves.
- **`Approximate` time-roll mode:** the non-additivity (`6M + 6M = 366d ≠ 1Y`) is documented honestly in `spec.rs:807-815`. Consider deprecating the mode if nothing depends on it, since `CalendarDays` dominates it.
