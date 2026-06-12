# Portfolio Crate & Bindings — Quant Finance Review

**Date:** 2026-06-12
**Scope:** `finstack/portfolio` (all modules), `finstack-py/src/bindings/portfolio/`, `finstack-wasm/src/api/portfolio/`, parity contract, stubs, JS facade.
**Method:** Six review passes — core aggregation path (valuation/FX/position/metrics/container), factor models, liquidity + optimization, margin + sensitivity + scenarios/replay, attribution + cashflows + performance, and bindings parity. Every finding cites file:line and was verified in context (callers/tests checked for compensation). DV01 sign convention (dPV/dy native) was treated as intentional and not flagged.

---

## Findings

### Blockers

#### B-1. Percentage-unit positions over-scaled 100× through the entire factor-risk pipeline
**Location:** `finstack/portfolio/src/factor_model/model.rs:313` (also 370, 451)
**Issue:** `FactorModel::compute_sensitivities` passes raw `position.quantity` as the engine weight instead of `position.scale_factor()`. For `PositionUnit::Percentage` the correct multiplier is `quantity / 100.0`. The same raw quantity is passed to `credit_curve_parallel_delta` in the assignment overlay (model.rs:370) and to `add_credit_residual_risk` (model.rs:451). The codebase already fixed exactly this bug in the stress-P&L path (whatif.rs C5 regression comment) — but only for P&L, not sensitivities.
**Impact:** For Percentage positions: factor exposures, CS01 overlays, and idio exposures 100× too large; vol/VaR/ES 100× too large; variance 10,000× too large; risk internally inconsistent with `factor_stress` P&L (which scales correctly). The C5 test doesn't catch it (uses a `FixedSensitivityEngine` that ignores quantity).
**Fix:** Use `position.scale_factor()` at all three sites; add an integration test asserting `analyze` parity between a `Percentage` portfolio and the equivalent `Units` portfolio.

#### B-2. Margin aggregator double-counts SIMM sensitivities on repeated `calculate` calls
**Location:** `finstack/portfolio/src/margin/aggregator.rs:163-178` with `finstack/portfolio/src/margin/netting_set.rs:99-105`
**Issue:** Each `calculate(&mut self, portfolio, market, as_of)` call recomputes per-position SIMM sensitivities and merges them into `NettingSet::aggregated_sensitivities`, which is never reset (`SimmSensitivities::merge` is additive per bucket; no `clear()` exists anywhere in the manager).
**Impact:** Second call ≈ 2× IM, third ≈ 3× (further distorted by concentration factors). VM is recomputed fresh, so IM and VM silently diverge. Wrong margin number, no error.
**Fix:** Reset aggregated sensitivities at the top of `calculate` (e.g. `NettingSetManager::reset_sensitivities()`), or aggregate into a per-call local map instead of mutating `self.netting_sets`.

#### B-3. Selective repricing misses the FX-to-base dependency — stale base-currency values after FX moves
**Location:** `finstack/portfolio/src/dependencies.rs:174-195` (`DependencyIndex::build`), `finstack/portfolio/src/valuation.rs:513-514, 528-541, 584-685`
**Issue:** Every `value_base` depends on the (instrument ccy → portfolio base) FX pair via `convert_to_base`, but the dependency index is built only from `instrument.market_dependencies()` and never receives the portfolio base currency. A plain EUR deposit in a USD-base portfolio declares only its EUR curve; when `changed = [Fx{EUR,USD}]` is passed to `revalue_affected`, the position is not in the affected set and `reuse_prior_or_value_position` returns the prior `PositionValue` with the old FX rate embedded. The doc claim ("identical to what `value_portfolio` would produce") is false for this case. `tests/selective_repricing.rs` only exercises curve keys — never FX.
**Impact:** Stale base-currency PVs and wrong portfolio totals after any FX move processed through the selective path. The `verify_full_eval` debug check would catch it but defaults off.
**Fix:** Pass `base_ccy` into `DependencyIndex::build` and insert `MarketFactorKey::Fx{instrument_ccy, base_ccy}` (both orientations, since `FxMatrix` may triangulate) for every position whose valuation currency differs from base; or treat any `Fx` key involving `portfolio.base_ccy` as affecting all positions denominated in the other currency. Add an FX-shock selective-repricing test.

#### B-4. Kyle's lambda calibrated from Amihud is dimensionally wrong (scaled by ADV instead of mid)
**Location:** `finstack/portfolio/src/liquidity/kyle.rs:78-90` (also `lambda_from_series`, kyle.rs:57-67)
**Issue:** `Self::new(amihud_ratio * avg_daily_volume)` produces a dimensionless "return for trading one full ADV", but `estimate_cost` treats λ as price-impact per share² (`total_cost = λ·q²/2`, converted to bps against `q·reference_price`). Per-share price impact is ILLIQ × mid, not ILLIQ × ADV. The unit test only checks the λq²/2 arithmetic for a hand-picked λ.
**Impact:** Cost estimates from the only built-in calibration paths (`from_amihud`, `lambda_from_series`) wrong by ~ADV/mid (≈4 orders of magnitude for a $100 stock, 1M ADV). Direct `KyleLambdaModel::new(λ)` with correctly-unitized λ unaffected.
**Fix:** Calibrate λ in price units per share (`amihud_ratio * mid`, requires passing the reference price), or keep λ in return space and multiply by `reference_price` once in `estimate_cost`. Document λ's units; add a test pinning bps for a realistic profile.

#### B-5. `position_what_if` omits the credit-residual overlay — before/after risk incomparable
**Location:** `finstack/portfolio/src/factor_model/whatif.rs:155-159`
**Issue:** The "after" decomposition calls `decomposer().decompose(...)` directly, but the baseline comes from `FactorModel::analyze`, which additionally applies `add_credit_residual_risk` (folds per-issuer idio variance into totals and rescales every factor contribution by `combined/systematic`). The "after" side never gets the overlay.
**Impact:** With non-zero `adder_vol_annualized`, `after.total_risk` drops the entire idiosyncratic component; even an **empty** change list yields non-zero `WhatIfResult.delta` entries. Resize/Remove never rescale residual exposure (∝ quantity²). The existing what-if credit test uses `adder_vol_annualized: 0.0`, so this is uncovered.
**Fix:** Route the what-if "after" path through the same residual overlay (with exposures rescaled for Resize/Remove), or error when `credit_idiosyncratic_variance` is non-empty. Add a what-if test with `adder_vol_annualized > 0`.

---

### Major

#### M-1. NaN sensitivities are silently converted into a zero-risk result
**Location:** `finstack/portfolio/src/factor_model/parametric.rs:200-208`; `simulation.rs:413`
`validated_variance` uses `variance.max(0.0)`; `f64::max(NaN, 0.0) == 0.0`, so a NaN exposure (e.g. NaN PV from a bumped market, or a NaN `Resize` quantity — unvalidated in whatif.rs) reports **zero portfolio risk** with NaN `absolute_risk` rows. The historical decomposer pre-screens non-finite P&Ls precisely to avoid this; the parametric and simulation decomposers do not.
**Fix:** Validate sensitivity-matrix finiteness in both decomposers; make `validated_variance` reject non-finite input; validate `PositionChange::Resize.new_quantity.is_finite()`.

#### M-2. No currency handling anywhere in the factor-model pipeline
**Location:** `finstack/portfolio/src/factor_model/whatif.rs:175-181`, `parametric.rs:124-137`
Sensitivities and stress P&L come from `Instrument::value_raw` (native-currency f64). `portfolio_exposures` and `factor_stress` sum across positions with no FX conversion and no validation that instrument currencies match `portfolio.base_ccy`. `PositionRiskDecomposition` docs claim base-currency values; nothing enforces it. Direct violation of the no-implicit-cross-currency invariant; no FX policy stamped.
**Fix:** Minimum: error in `compute_sensitivities`/`factor_stress` when any instrument's valuation currency ≠ `base_ccy`. Longer term: convert via `FxProvider` and stamp the policy.

#### M-3. VaR/ES sign conventions are inconsistent within the module; `RiskBudget` breaks under the negative convention
**Location:** `finstack/portfolio/src/factor_model/types.rs:14-16` vs `position_risk.rs:558-559`; `risk_budget.rs:185-188`
Factor-level decomposers report VaR/ES as non-positive (loss convention); position-level decomposers report positive. `RiskBudget::evaluate_components` assumes positive: with negative-convention components, an over-risk position yields `excess < 0`, so `total_overbudget` reports 0 while `utilization > 1` still flags breach — internally contradictory.
**Fix:** Pick one convention module-wide (mod.rs docs already declare loss-negative) and convert the position-level decomposers, or normalize sign inside `evaluate_components`.

#### M-4. Almgren-Chriss `from_profile` calibration mixes return-space and price-space units
**Location:** `finstack/portfolio/src/liquidity/almgren_chriss.rs:105-116`
`estimate_cost` treats γ/η as price-space ($/share per share), but calibration uses `relative_spread()` for γ (off by mid, ≈100× understated for a $100 stock) and `σ·√(mid/ADV)` for η (off by √mid). Coefficients are off by *different* factors, so even the permanent/temporary split is distorted.
**Fix:** `gamma = profile.spread() / (2·ADV)`; `eta = σ·mid·√(1/ADV)` (or calibrate consistently in return space and multiply by `reference_price` once). Add a golden test pinning `cost_bps` magnitude.

#### M-5. sinh overflow in the Almgren-Chriss trajectory produces NaN schedules
**Location:** `finstack/portfolio/src/liquidity/almgren_chriss.rs:248-263`
For κT ≳ 710 (plausible with user-supplied risk aversion and small η), `sinh` overflows to inf and interior points compute inf/inf → NaN quantities, expected cost, and variance, returned as `Ok(...)`.
**Fix:** Use the stable ratio `sinh(a)/sinh(b) = exp(a−b)·(1−e^{−2a})/(1−e^{−2b})`, or fall back to the asymptotic schedule for large κT; validate `risk_aversion` finite/non-negative.

#### M-6. Liquidity scoring and LVaR divide base-currency PV by native-currency mid
**Location:** `finstack/portfolio/src/liquidity/scoring.rs:134-144`; `lvar.rs:260-264`
`position_shares = value_base / profile.mid` mixes currencies (`LiquidityProfile` documents native-currency prices). Days-to-liquidate, %ADV, tiering, and the LVaR horizon adjustment are off by the FX rate for every non-base position (≈150× for JPY in a USD book). `LvarCalculator::compute` never states which currency `position_value` must be in.
**Fix:** Convert PV to native currency explicitly, or take share/contract quantity directly from `Position`; document the expected currency on `LvarCalculator::compute`.

#### M-7. Optimizer hard-clamps existing positions to weights in [0, 1]; shorts are silently flipped or closed
**Location:** `finstack/portfolio/src/optimization/decision.rs:160-174`; `lp_solver.rs:626-665`
Existing positions get `[0, 1]` bounds and constraint refinement can only narrow (`min.max(*min)`, `max.min(*max)`), so `weight_bounds(filter, -1.0, 1.0)` silently becomes `[0, 1]`. Candidates can short (`allow_short_candidates`); existing positions cannot. For hedged books (explicitly supported by gross-normalization logic), the auto-budget Σw = 1 makes the solver flip shorts flat/long and report `Optimal`.
**Fix:** Seed existing-position bounds from the sign of the current weight or an `allow_short_existing` flag; or error when a tradeable position's current weight lies outside its effective bounds.

#### M-8. Entity-based filters never match candidate positions in objectives/metric constraints
**Location:** `finstack/portfolio/src/optimization/lp_solver.rs:96-107` (used at 153-160)
`decision_entity_id` returns `EntityId::new("")` for candidates even though `CandidatePosition` carries a real `entity_id` (and `WeightBounds` looks it up correctly). Any `ByEntityId` filter (incl. inside `And`/`Or`/`Not`) gets coefficient 0.0 for candidates; with `Not(...)` the polarity inverts and candidates are wrongly included.
**Impact:** "Entity X ≤ 20%" exposure limits don't count candidate allocations — solver can violate the intended constraint while reporting `Optimal`.
**Fix:** Resolve the candidate's entity from `problem.trade_universe.candidates` by `item.position_id`.

#### M-9. Turnover constraint slack is fictitious; only the first `MaxTurnover` is enforced
**Location:** `finstack/portfolio/src/optimization/lp_solver.rs:538-553` and `406-432`
The zero-coefficient turnover placeholder row is included in the slack loop, so `constraint_slacks["turnover"] = max_turnover` always (never reported binding). The aux-variable expansion uses `.find(...)` — a second `MaxTurnover` constraint is silently ignored.
**Fix:** Skip placeholder rows and compute slack as `max_turnover − Σ|w*−w0|`; enforce all `MaxTurnover` constraints or reject duplicates.

#### M-10. Cleared netting sets: IM computed with SIMM but labeled `ClearingHouse`
**Location:** `finstack/portfolio/src/margin/aggregator.rs:302-316`
The `is_cleared()` branch changes only the label; `calculate_simm_with_breakdown` is always used. The CCP IM calculator in `finstack/margin/src/calculators/im/clearing.rs` is never invoked from the aggregator.
**Fix:** Route cleared sets through the CCP calculator, or stamp `Simm` with an explicit proxy note — a policy-visibility violation as is.

#### M-11. `add_netting_set_with_fx` silently drops the netting set on a bad FX rate
**Location:** `finstack/portfolio/src/margin/results.rs:245-253`
Non-finite/non-positive `fx_rate` → `tracing::error!` + `return;` — the set vanishes from all totals and `by_netting_set`. `tests/margin_aggregation.rs:200-218` codifies the silent drop.
**Fix:** Return an error (`InvalidFxRate` variant); update the test to assert the error.

#### M-12. `cleared_bilateral_split` sums raw amounts across mixed currencies
**Location:** `finstack/portfolio/src/margin/results.rs:287-303`
Only the totals are FX-converted by `add_netting_set_with_fx`; the stored `NettingSetMargin` keeps its native currency. The split sums `total_margin.amount()` across all stored sets and labels it `self.base_currency` — silent cross-currency addition, acknowledged only in a test comment.
**Fix:** Store the FX-converted margin (or the rate) in `by_netting_set`, or error/skip when stored currencies differ from base.

#### M-13. CSA threshold / MTA / margin spec are carried but never applied
**Location:** `finstack/portfolio/src/margin/netting_set.rs:24, 59-62`; `aggregator.rs:249-332`
`NettingSet.margin_spec: Option<OtcMarginSpec>` (thresholds, MTA via `CsaSpec`) is never set by `from_portfolio`/`add_position` and never read. VM is raw net MTM; IM is raw SIMM; `total = IM + max(VM, 0)`.
**Impact:** Reported requirements won't match actual CSA call amounts; the dead field implies the terms are honored.
**Fix:** Populate and apply `vm_threshold`/MTA in `calculate_netting_set_margin` (the `finstack_margin` calculators already model them), or remove the field and document outputs as pre-CSA gross.

#### M-14. Replay values every snapshot at the static `portfolio.as_of`; attribution uses step dates
**Location:** `finstack/portfolio/src/replay.rs:252-279, 371-389`; `valuation.rs:482, 497, 514`
`replay_portfolio` calls `value_portfolio` per snapshot — every valuation prices at `portfolio.as_of` (instruments never age; FX lookups predate snapshot data), while attribution between steps uses the actual step dates. The factor P&L structurally cannot reconcile to `daily_pnl`.
**Fix:** Value each snapshot at its own date (thread a `value_portfolio_at` variant through, FX date included), or document replay as a frozen-as-of what-if and make attribution use `portfolio.as_of` on both legs.

#### M-15. Multi-currency IR vega silently collapsed last-write-wins in SIMM IM (cross-crate, hit from the portfolio aggregator)
**Location:** `finstack/margin/src/calculators/im/simm.rs:1073-1079`, reached from `finstack/portfolio/src/margin/aggregator.rs:367-375`
`ir_vega` is keyed `(Currency, tenor)` but flattened by tenor only; `collect()` keeps the last entry — USD 5Y and EUR 5Y vega in one set: one is discarded (which one depends on map iteration order). Latent for built-in `Marginable` impls (delta-only) but live for wire-format/externally supplied sensitivities (`tests/margin_serialization.rs` itself constructs multi-currency vega).
**Fix:** Sum on collision, or aggregate per currency like `calculate_ir_delta_multi_currency`.

#### M-16. `carino_link` silently propagates NaN despite documenting an error for non-finite returns
**Location:** `finstack/portfolio/src/brinson.rs:262-307, 383-399`
The doc promises `Error::InvalidInput` for non-finite per-period returns; no check exists. For NaN/±inf returns, `carino_coefficient` returns `Ok(NaN)` and the NaN multiplies every linked sector effect and compounds into `portfolio_return_compounded`. Rust callers and deserialized `BrinsonPeriodResult` payloads reach `carino_link` directly (bindings are compensated via `carino_link_from_sector_periods`).
**Fix:** Validate finiteness of period returns (and ideally sector effects) at the top of `carino_link`.

#### M-17. `aggregate_metrics` lets the caller pass a `base_ccy`/`as_of` that silently disagree with the valuation
**Location:** `finstack/portfolio/src/metrics.rs:253-331`; exposed at `finstack-py/src/bindings/portfolio/spec.rs:72` and `finstack-wasm/src/api/portfolio/mod.rs:227`
The implied-FX path (`value_base / value_native`) always converts into the **valuation's** base currency regardless of the `base_ccy` argument; positions whose native ccy equals the *requested* base get rate 1.0 while their `value_base` is in the valuation's base. Both bindings expose the free-form `base_ccy`/`as_of` parameters directly.
**Impact:** Mismatched arguments produce wrong-currency aggregated risk totals with no error.
**Fix:** Validate `base_ccy == valuation.total_base_ccy.currency()` and `as_of == valuation.as_of` (error otherwise), or drop the parameters and derive both from the valuation.

#### M-18. `PositionFilter.not` is a Python keyword — unusable as named; stub documents a different name
**Location:** `finstack-py/src/bindings/portfolio/optimization_spec.rs:578-582`; stub `finstack-py/finstack/portfolio/__init__.pyi:880`
The classmethod is registered as `not` (SyntaxError to call); the stub says `not_` (doesn't exist). Siblings `and_`/`or_` already use the trailing-underscore convention.
**Fix:** `#[pyo3(name = "not_")]`.

#### M-19. Same-named risk-decomposition APIs return different schemas in Python vs WASM
**Location:** `finstack-py/src/bindings/portfolio/position_risk.rs:38-91, 333-353`; `finstack-wasm/src/api/portfolio/mod.rs:384-409, 453-476, 479-521`
For `parametric_var_decomposition`, `historical_var_decomposition`, `evaluate_risk_budget`: Python hand-builds dicts, renames canonical `relative_var` → `pct_contribution`, drops `method`/`es_contributions`, and adds binding-computed fields (`breach` re-implements engine logic); WASM serializes the raw serde structs. The ES variant doesn't drift only because a canonical Rust view exists.
**Fix:** Add `parametric_var_decomposition_view` (and a budget view) in `finstack_portfolio::factor_model`, and have both bindings emit it — mirroring the ES pattern.

---

### Moderate

#### MO-1. `revalue_affected` early-return hands back `prior.clone()` verbatim
`finstack/portfolio/src/valuation.rs:600-602` — when no positions are affected, positions added/removed since `prior` are not reconciled (the slow path tolerates drift via `reuse_prior_or_value_position`; the early return doesn't). Stale totals for mutated portfolios.

#### MO-2. `PositionUnit::FaceValue` scaling contract documented two contradictory ways
`finstack/portfolio/src/position.rs:29` ("PV per one face-value unit") vs `position.rs:402` ("instrument typically returns full PV"). Following the wrong one double-counts by the face amount. Pick one convention; bonds quoting per-100 face make this a classic silent killer.

#### MO-3. Grouping/book aggregation silently drops unresolvable positions
`finstack/portfolio/src/grouping.rs:88, 137, 239-243` — positions missing from the valuation, or book `position_ids` referencing nonexistent positions, contribute zero with no warning/issue record (contrast `cashflows.rs`, which records `issues`). Book references should hard-error.

#### MO-4. Serde default for `cross_factor_pnl` hardcodes USD zero
`finstack/portfolio/src/attribution.rs:124-125, 179-181` — deserializing an older non-USD payload injects `USD 0.00` amid e.g. EUR buckets; subsequent `Money` arithmetic fails on a numerically-zero value. Re-stamp the default with `total_pnl.currency()` via a deserialization helper.

#### MO-5. `position_detail_to_csv` omits `cross_factor_pnl` — per-position rows don't close
`finstack/portfolio/src/attribution.rs:653-684` — the portfolio CSV has a `cross_factor` column; the position CSV doesn't, so factor columns won't sum to `total` under the Waterfall method.

#### MO-6. `ValueWeightedAverage` is silently identical to `WeightedSum`
`finstack/portfolio/src/optimization/lp_solver.rs:150-183` — under a filter (normalizer is the filtered weight share, not 1) or budget ≠ 1, the "average" bound is wrong by the weight-share factor; portfolios pass constraints they should fail. Lower filtered average bounds to `Σ_{i∈F} w_i(m_i − rhs) ≤ 0` or reject the unsupported combinations.

#### MO-7. `to_rebalanced_portfolio` silently drops candidate allocations
`finstack/portfolio/src/optimization/result.rs:154-171` — only existing positions get updated quantities; candidates with non-zero weight never materialize, so the "rebalanced" portfolio holds less than 100% of intended exposure.

#### MO-8. Zero-PV existing positions get implied quantity 0 — silent close-out of par swaps
`finstack/portfolio/src/optimization/lp_solver.rs:505-513` — candidates with |PV/unit| < tol are rejected with a good error; existing positions silently get quantity 0 under `ValueWeight`.

#### MO-9. Auto-added budget Σw = 1 is nonsensical under `UnitScaling`
`finstack/portfolio/src/optimization/lp_solver.rs:282-290`; `problem.rs:53-64` — Σ(quantity multipliers) = 1 on an n-position book silently mandates liquidating most of the aggregate scaling, reported `Optimal`.

#### MO-10. New short candidates misclassified as `TradeType::Existing`
`finstack/portfolio/src/optimization/result.rs:205-211` — classification tests `target_weight > WEIGHT_TOL` instead of `.abs()`; new shorts (borrow locates!) miss the `NewPosition` label.

#### MO-11. `execution_risk` overstates uniform-execution risk by √3
`finstack/portfolio/src/liquidity/almgren_chriss.rs:162-163` (same in kyle.rs:140-141) — uses full-position-held-for-T σ instead of the linear-trajectory `σ·P·|Q|·√(T/3)` (Almgren & Chriss 2001); inconsistent with the same module's `optimal_trajectory` variance.

#### MO-12. LVaR confidence handling: unvalidated config; `confidence < 0.5` is anti-conservative
`finstack/portfolio/src/liquidity/lvar.rs:208-213, 166-177` — `confidence_level: 1.0` → z = ∞; values < 0.5 give negative spread cost (`lvar > var`, violating the documented invariant); `var == 0` with `dtl == ∞` stores NaN `lvar_horizon`. Validate `(0.5, 1)`; guard the 0·∞ product.

#### MO-13. Liquidity report drops infinite-DTL positions from averages and concentration stats
`finstack/portfolio/src/liquidity/scoring.rs:213-236` — unsellable (∞ DTL) positions lower the weighted-average DTL (kept in denominator, dropped from numerator) and are excluded from `most_concentrated_position` — the headline stats understate risk exactly when an untradeable position exists.

#### MO-14. Margin position accounting contradicts its own docs
`finstack/portfolio/src/margin/aggregator.rs:163-178, 261-297`; `results.rs:143-169` — sensitivity-degraded positions still count in `total_positions`; `truly_non_marginable_count()` undercounts with `saturating_sub` masking it; positions can appear twice in `degraded_positions`.

#### MO-15. Margin aggregator silently drops tracked positions missing from the portfolio
`finstack/portfolio/src/margin/aggregator.rs:149-158, 261-296` — `filter_map` over `portfolio.get_position` skips stale registrations with no degraded record or warning; margin silently understated when aggregator and portfolio drift.

#### MO-16. `apply_scenario` mispairs instruments in release builds if the engine resizes the vector
`finstack/portfolio/src/scenarios.rs:107-116` — length invariant is `debug_assert` + `zip` (truncates silently). Promote to a runtime `Error::ScenarioError`.

#### MO-17. FX-delta currency rebase doesn't restructure calc-currency exposure (latent)
`finstack/portfolio/src/margin/aggregator.rs:230-246` with `finstack/margin/src/types/simm_types.rs:402-442` — uniform rescale + relabel; SIMM FX delta is defined against the calculation currency, so a rebase must re-map keys. Latent (no built-in instrument populates `fx_delta`) but live for external sensitivities.

#### MO-18. `CreditVolReport` mixes Euler-allocated and standalone components
`finstack/portfolio/src/factor_model/credit_vol_forecast.rs:448-455, 501-507` — `generic + Σ by_level + idiosyncratic_total ≠ total` for any tail measure; idio is double-counted relative to what's inside `total`. Only Variance reconciles.

#### MO-19. Variance-measure `marginal_risk` is half the true gradient
`finstack/portfolio/src/factor_model/parametric.rs:167, 255`; `simulation.rs:427-436` — ∂(x'Σx)/∂x = 2(Σx); reported (Σx). Optimizer gradients / FD cross-checks 2× understated. Multiply by 2 or re-document.

#### MO-20. `VolHorizon` step semantics ambiguous against annualized variances
`finstack/portfolio/src/factor_model/credit_vol_forecast.rs:61-75, 117-127` — variances are annualized; `NSteps(n)` multiplies by raw n, so "one step" = one **year**. `NSteps(10)` intending 10 days overstates variance ~252×.

#### MO-21. `RiskBudget` silently ignores un-budgeted positions; documented zero-VaR error not implemented
`finstack/portfolio/src/factor_model/risk_budget.rs:164-201` — positions absent from `targets` are invisible (no breach possible); the promised zero-portfolio-VaR error doesn't exist (yields ∞ utilization instead).

#### MO-22. Python `almgren_chriss_impact` re-implements the canonical Rust helper inline
`finstack-py/src/bindings/portfolio/liquidity.rs:222-284` — duplicates `liquidity::almgren_chriss_uniform_impact` line-for-line (incl. hard-coded δ=0.5 and the synthetic 20bp profile); WASM calls the helper. Future Rust changes silently diverge Python.

#### MO-23. Sensitivity bindings map `finstack_core::Error` via `display_to_py`, breaking the KeyError convention
`finstack-py/src/bindings/portfolio/sensitivity.rs:304, 363, 631` (also `extract.rs:216-225`) — missing-curve failures surface as `ValueError` instead of `KeyError`; same failure raises different exception classes depending on entry path. Use `core_to_py`/`portfolio_to_py`.

#### MO-24. WASM liquidity estimators return `NaN` where Python returns `None`
`finstack-wasm/src/api/portfolio/mod.rs:543, 551, 633-636` — `Option<f64>` → `f64::NAN` for `rollEffectiveSpread`/`amihudIlliquidity`/`kyleLambda`; the silent-propagation sentinel the Python design explicitly rules out. `twrrModifiedDietz` in the same file already does `Option<f64>` → `undefined`.

#### MO-25. Invented Python class `FactorRiskDecomposition` shadows canonical `RiskDecomposition`
`finstack-py/src/bindings/portfolio/sensitivity.rs:381-550` vs `factor_model.rs:278-365` — two Python classes wrap the same Rust type; the invented one renders `measure` as a Debug string (`VaR { confidence: 0.99 }`) instead of the serde wire form; WASM `decomposeFactorRisk` has the same Debug-formatted field.

#### MO-26. Python `decompose_factor_risk` rejects zero-factor input that Rust/WASM handle
`finstack-py/src/bindings/portfolio/sensitivity.rs:593-597` — guard protects the binding's own `chunks_exact` loop; canonical Rust returns a benign zero decomposition.

#### MO-27. Python historical decomposition duplicates `flatten_position_pnls`
`finstack-py/src/bindings/portfolio/position_risk.rs:215-253`; `factor_model.rs:1716-1754` — the canonical helper exists for bindings and is what WASM uses.

#### MO-28. Tier thresholds hardcoded in both bindings
`finstack-py/src/bindings/portfolio/liquidity.rs:117`; `finstack-wasm/src/api/portfolio/mod.rs:575` — `[1.0, 5.0, 20.0, 60.0]` duplicated instead of `LiquidityConfig::default().tier_thresholds` (registry-backed).

---

### Minor

1. **Percentage validation asymmetric** — `position.rs:218` rejects quantity > 100 but allows < −100 (short percentage beyond −100%).
2. **`Portfolio` derives Serialize/Deserialize but `#[serde(skip)]`s positions** — `portfolio.rs:47`; direct serde silently drops all positions (PortfolioSpec is the wire format); deserialization yields an unusable empty portfolio.
3. **`validate()` doesn't check `position.book_id ∈ books`** nor that `book.position_ids` reference existing positions — `portfolio.rs:325-351` (compounds MO-3).
4. **`PositionMetrics` doc says "raw metric values" but summable metrics are scaled** by `metric_scale` — `metrics.rs:71-72` vs `272-274`.
5. **`book.rs:32` doc claims parent/child consistency "is not validated"** — `validate_book_hierarchy` does validate it; stale doc.
6. **Modified Dietz silently clamps out-of-range flow weights** — `performance.rs:104`; reject `w ∉ [0,1]`/non-finite instead.
7. **`twrr_linked` annualizes sub-one-year horizons** — `performance.rs:166-170`; GIPS 2020 prohibits it; treat `0 < horizon < 1` as cumulative.
8. **`carino_coefficient` catastrophic cancellation just above the 1e-12 cutoff** — `brinson.rs:393-398`; use `(diff/(1+r_b)).ln_1p()/diff`.
9. **`_untagged` group key collides with a genuine attribute value** — `grouping.rs:16, 47-52`; numeric tags also silently route to `_untagged`.
10. **`neumaier_sum` misused (2-element calls in a loop = naive summation)** — `lp_solver.rs:527-531`.
11. **Absolute `SLACK_TOL`, unlabeled/duplicate constraint names in diagnostics** — `result.rs:277-285`; `lp_solver.rs:539-552`.
12. **Liquidity inbound types don't `deny_unknown_fields`; `mid: 0.0` bypasses validation via serde** — `liquidity/types.rs:48-90, 258-307`; `impact.rs:12-40`. Same for margin wire structs, `replay.rs:64-68` (`JsonSnapshot`), `sensitivity/positions.rs:16-21` — contrary to the strict-serde invariant.
13. **`turnover()` documented "one-way" but computes Σ|Δw|** — `result.rs:287-295`; off by 2× vs the standard convention.
14. **`NotionalWeight` uses raw `position.quantity` across mixed `PositionUnit`s** — `decision.rs:112-114`; shares vs face amounts compared as "notional".
15. **NaN/Inf PVs unvalidated in both sensitivity engines** — `delta_engine.rs:53-60`; `repricing_engine.rs:134-163`.
16. **Replay `max_drawdown_pct` sign flips for negative peak values** — `replay.rs:442-446`.
17. **Margin wire deserialization trusts serialized totals** (doesn't re-establish `total = IM + max(VM,0)`) — `wire.rs:281-295, 362-389`.
18. **`Position::scale_value` warns-only on notional-currency mismatch feeding VM** — `position.rs:448-462` via `aggregator.rs:338-353`.
19. **Inconsistent tail-count conventions** (`ceil` vs `floor`, FP-noise off-by-one at 99%/100k) — `simulation.rs:483` vs `position_risk.rs:784`.
20. **Assignment report records only the last matched factor per dependency** — `assignment.rs:44-49`; diagnostic disagrees with actual multi-factor overlay.
21. **Absolute (scale-blind) tolerances in covariance validation** — `simulation.rs:8`, `parametric.rs:34`.
22. **Position-level decomposer accepts confidence in (0,1) instead of (0.5,1)** — `position_risk.rs:366-371`; sign-inconsistent VaR/ES instead of an error.
23. **NaN confidence passes the crate check** (`<= 0.0 || >= 1.0` both false for NaN) — `position_risk.rs:366, 720`; easiest to hit from raw JS (`undefined → NaN`).
24. **`__all__` on the raw Rust module missing the six sensitivity symbols** — `finstack-py/src/bindings/portfolio/mod.rs:63-144`.
25. **`optimize_portfolio` compact vs pretty JSON across bindings** — `optimization.rs:36` vs WASM `mod.rs:345`.
26. **`evaluate_risk_budget` echoed `target_pct` misaligns under duplicate position ids** — `position_risk.rs:340` (zip truncation).
27. **Portfolio totals exposed only as lossy f64; no `total_value_decimal`** — `types.rs:147-149, 221-224`; `core.Money` already has the lossless pattern.
28. **`days_to_liquidate` first parameter renamed/re-unitized in both bindings** — `liquidity.rs:96-98`; WASM `mod.rs:555-566`.

---

## Open Questions or Assumptions

1. **`FaceValue` scaling convention** (MO-2): which contract is intended — PV per 1 face unit (table) or full PV (scale_value doc)? Existing tests use it both ways with quantity-as-face; the answer determines whether any current usage double-counts.
2. **VaR/ES sign convention** (M-3): the module-level docs declare loss-negative, the position-level engines are positive. This needs a single decision before fixing `RiskBudget`; it also affects the binding schemas (M-19).
3. **Margin output semantics** (M-13): is the aggregator intentionally a pre-CSA gross-requirement engine? If so, removing `margin_spec` and documenting it is the fix; if not, threshold/MTA application is required work.
4. **Replay design intent** (M-14): frozen-as-of market-move what-if vs true historical replay. Either is defensible; the current half-and-half (static valuation date, stepped attribution dates) is not.
5. **`VolHorizon` step unit** (MO-20): is a "step" meant to be a year? If daily forecasting is intended, a `periods_per_year` is missing from the design.
6. **Strict-serde scope**: portfolio inbound types broadly do not `deny_unknown_fields` (crate-level choice, also noted by the bindings review). Confirm whether the workspace invariant is meant to apply here; if yes this is one sweep across wire/spec/config types.

## Brief Summary

The crate's **core aggregation spine is in good shape**: deterministic Neumaier aggregation with positional parallel collection (parallel ≡ serial by construction), currency-safe FX collapse through one shared helper with correct rate direction and policy stamping, robust degraded-position tracking, sound Brinson-Fachler/Carino mathematics with exact closure tests, correct parametric Euler decomposition/ES multipliers, a correct discrete Almgren-Chriss trajectory, textbook incremental VaR, and a well-built LP encoding (offset transforms, turnover linearization) over a deterministic pure-Rust solver. Binding coverage is broad, GIL handling is right, and runtime `__all__`/stub/facade parity all verify.

The defects cluster at **integration seams rather than core formulas**: unit-system boundaries (Percentage scale factor missed in factor risk; Amihud/AC calibrations mixing return- and price-space; native-vs-base currency divisions in liquidity), **state lifecycle** (margin accumulator never reset; selective-repricing index blind to the FX-to-base edge), **overlay consistency** (what-if missing the credit residual that its own baseline includes), and **silent-failure paths** (NaN→zero-risk, dropped netting sets, dropped candidate allocations, flipped shorts, cross-currency sums in the cleared/bilateral split). Five Blockers and nineteen Majors warrant remediation before this surface is trusted for production margin, risk, or execution-cost numbers; none look architecturally hard — most are localized fixes plus the missing regression tests called out per finding.

Residual risk: the `finstack_margin` SIMM internals (risk weights, correlations, curvature), the `finstack_scenarios` engine shock semantics, the `finstack-factor-model` matcher/calibration internals, and the `finstack_attribution` instrument-level decomposition were reviewed only at their portfolio-facing call surfaces. No golden/parity fixtures exist for Almgren-Chriss costs, LVaR, or SIMM-from-portfolio against external references — worth adding alongside the fixes.

## Quant Notes

- **Almgren & Chriss (2001)**: trajectory and κ²=λσ²/η are correctly implemented; the linear-trajectory IS variance is Q²T/3 (hence the √3 in MO-11). The omitted η̃ = η(1−γτ/2η) correction is an acceptable approximation and documented.
- **Kyle (1985) / Amihud (2002)**: λ has units price-per-share per share; ILLIQ is |return| per volume. The bridge between them is the mid price — the missing factor in B-4.
- **Bangia et al. (1999)**: the exogenous-spread LVaR core is correctly implemented (half of mean-plus-quantile of relative spread); the confidence-domain and 0·∞ issues (MO-12) are guards, not formula errors.
- **Carino (1999)**: linking coefficients and the equal-returns limit are right; prefer `ln_1p` form for the near-equal-returns regime (index trackers).
- **GIPS 2020**: no annualization below one year (Minor 7); Modified Dietz weight domain should be enforced, not clamped (Minor 6).
- **ISDA SIMM**: FX delta is defined relative to the calculation currency — a calc-currency rebase must restructure keys (MO-17); IR vega aggregates per (currency, tenor) (M-15).
- **Euler/Tasche allocation**: component decompositions are exact and well-tested; the Variance-measure "marginal" is a component-per-unit, not a gradient, unless doubled (MO-19).
