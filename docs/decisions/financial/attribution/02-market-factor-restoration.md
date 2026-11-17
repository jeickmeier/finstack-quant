# ATTRIBUTION-002: Economic factor families and exact market restoration

- Decision status: Accepted 2026-11-16 for checked economic factor families and market restoration.
- Implementation status: Observed for the source scope listed below. Runtime, source coverage, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent examples checked symmetric insertion and removal, same-label typed isolation, qualified closing-only inputs and changes in FX route. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked economic factor families and market restoration

1. Resolve the selected pricer's actual source bindings at both endpoints, including inputs controlling source eligibility and precedence. Recheck each staged counterfactual because it can activate different sources. Skip a family only with proof of irrelevance or an exact no-op. If fresh dependency discovery leaves required coverage unresolved, fail the affected experiment. [Accepted dependency rules](../portfolio-execution/05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing).
2. Use namespace, kind, ID and applicable coordinate and qualify each source's economic role. Rates cover selected discount and projection sources and rate fixings. Credit covers selected hazards and qualified index inputs. Correlation covers consumed market correlation sources, including base correlation. Inflation covers CPI curves and releases. Volatility covers surfaces, cubes, FX-delta surfaces, scalar volatility and volatility-index curves. MarketScalars covers ordinary price curves, spot and yield observations, dividends and other pricing series. Retire classification by fixing prefix or matching spot and volatility labels. Apply a shared source once and require explicit ownership for conflicting roles. Carry and model parameters have their own operations. [Typed market identity](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).
3. Bind authority to the captured scope and distinguish present, qualified absent and uncaptured inputs. Restore each selected source exactly, including insertions and removals in either direction. A captured empty scope authorizes clearing it. Uncaptured scope fails restoration. Parallel includes qualified closing-only sources even when the whole family is empty at opening. A qualified optional absence follows only authorized source resolution for the selected product.
4. Stage the full candidate atomically. Select donor values for scoped sources and recipient values elsewhere. Rebind and validate registered dependencies before pricing. Preserve exact credit-index membership, aggregate timing and named hazard roles, recovery, weights and base-correlation roles. An incompatible hybrid fails with its exact binding. A source-kind or index-composition change requires an explicitly owned operation. [Atomic context lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle) · [Credit-index composition](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts).
5. Retain the valuation cut and source clocks, exact observations, vintages and publication eligibility. Require qualified information at each stage. Conditioning, fixing realization and quote replay require explicit elections through their owners. [Scenario time rolls](../scenarios/13-market-time-roll.md#accepted-decision-for-checked-scenario-time-rolls-and-horizon-replay).
6. Restore FX through the qualified frozen quote authority. Preserve ordered queries, date policies, eligible quotes and route precedence, including newly eligible quotes and pivot legs. Bridge curves retain their rate roles. [FX resolution](../core-conventions/013-fx-resolution-and-shocks.md) owns rate delivery and [record 013](13-target-currency-and-scaling.md) owns reporting-only translation.
7. Use the same checked Rust restoration path for Waterfall, Parallel and Taylor restore-and-reprice stages. Their method owners retain stage order and effect calculation. Unsupported required comparisons fail with typed context. Publish selected scope, actual substitutions, state and cut identities, and evidence for skipped work. Host views project the Rust result. Retain source evidence for economic replay. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

Independent examples checked symmetric source replacement and same-label isolation. An unchanged EUR `100` native mark moved from CAD `150` to CAD `160` when the opening pivot rate `1.20 × 1.25` gave way to a qualified direct rate of `1.60`. This yielded CAD `10` of reporting FX.

## Observed choice and input domain

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
