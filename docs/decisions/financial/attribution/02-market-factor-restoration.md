# ATTRIBUTION-002: Economic factor families and exact market restoration

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

Repricing attribution isolates economic families by replacing selected market stores. A snapshot covers all items in a selected family, while declared dependencies determine whether a family is active and classify scalar volatility quotes. This is not per-instrument filtering of every snapshot entry.

## Algorithm and controls

- Rates combine discount and forward families. Forward includes forward, basis-spread and parametric curves plus fixing series whose IDs have the canonical fixing prefix. Hazard curves form credit; inflation includes curves and index fixings; base-correlation curves form correlations.
- Volatility includes surfaces, SABR cubes, FX-delta surfaces, volatility-index price curves and scalar quotes identified by volatility dependencies. A dependency whose spot ID equals its volatility ID is excluded from scalar-vol classification. Market scalars hold remaining price scalars, ordinary price curves, non-fixing time series and dividend schedules. FX replaces the entire matrix.
- Restore by cloning the current market, installing replacement curves before removing obsolete members of selected families. This avoids transient missing dependencies. Invalidated credit-index metadata produces a tracing warning. Replace volatility stores and FX exactly; an absent snapshot matrix clears FX. Rebuild scalar stores while preserving unselected economic families; restore rate fixings separately with forward rates.
- `InstrumentFactorUse` uses declared discount/forward, hazard, inflation, FX, vol and scalar dependencies. Correlation always remains eligible because no correlation dependency is declared in this contract. An entirely empty dependency declaration or dependency-query error permits every family, avoiding silent exclusion of undeclared exposures. Carry/model parameters are always eligible.

## Conventions, failure and alternatives

New/deleted market members belong to their economic family; this is replacement, not an overlay preserving obsolete quotes. Parallel skips a family empty at opening and notes when closing data exists; waterfall can install closing data. Rates fixing changes therefore belong to rates, while carry retains the opening fixing environment. Snapshot extraction itself does not calibrate or roll curves. Complete dependency declarations improve isolation/cost; interpreting empty declarations as no exposure would be a different policy.

## Rationale and evidence

Economic grouping and exact replacement explain the implementation's factor interpretation; adoption of these group boundaries remains pending. Dependency-query fallback keeps exposures visible but cannot guarantee correct family classification for custom instruments.

Located tests `scalar_stores_restore_only_their_economic_factors` and `missing_opening_quotes_are_removed_only_by_their_own_factor` are family-isolation invariants; `discount_only_bond_does_not_use_unused_book_families` exercises dependency selection.

## Source and evidence

- [factors.rs](../../../../finstack-quant/attribution/src/factors.rs) — owning implementation.
- [helpers.rs](../../../../finstack-quant/attribution/src/helpers.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
