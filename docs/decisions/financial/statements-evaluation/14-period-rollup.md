# STATEMENTS-EVALUATION-014 — Flow, balance and formula rollup to coarser periods

Decision status: **Accepted for the full period rollup contract (2026-11-15)**. Implementation status: **Reconciled (2026-11-17), with checks on rolled results outstanding**. Validation status: **Qualified by the half-year example and its absent-flow variant, unresolved methods, partial periods, foreign results, compensated and exact sums, overflow, final-cell status, re-evaluated masks and target cadence**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

Every node resolves to an explicit method, the request's entry and then the node's declaration; a node with neither fails the rollup with every such node named, and nothing is summed by default. The bundled flow metrics declare `sum`. The source result must be a verified run of the model: the same content, source snapshot, period plan and run identity. `Sum` adds typed cells, money exactly and scalars with a compensated sum, and a source cell that is missing, masked or unavailable leaves the target unavailable; a sum that mixes units or cannot be represented is an error. `PeriodEnd` copies the final source cell with its status. `Formula` re-evaluates the mask and formula over typed rolled cells and target-period history: a false mask is a masked cell, an input without a value leaves the result unavailable, and a function that is invalid at the target cadence is an error. The rolled result records its source run, the method used for each node and the source periods of each target period (`rollup_provenance`); it is not itself a run and carries no run provenance, cell sources, capital snapshots or check report. A check suite accepts only a verified run, so checks against a rolled result are not yet available. The sections below the accepted decision describe the audited implementation.

## Accepted decision for period rollup

Require an explicit rollup method for every rolled node and its required inputs. A request-level method overrides the node declaration. Reject unknown nodes and unresolved methods, and remove automatic `Sum` selection.

1. Verify that the source result matches the model and its period plan. Roll months, quarters or half-years into strictly coarser periods on the same Gregorian or fiscal calendar. Require exact canonical coverage and reject partial periods. A target period is actual only when all of its source periods are actual.
2. Use `Sum` for additive flows with checked Money arithmetic or checked compensated scalar accumulation. A required missing, masked or otherwise unavailable source cell makes the target cell unavailable with a reason. Use `PeriodEnd` for balances or trailing measures reported at close, copying the final source cell and its status. [Numerical accumulation policy](../../standards/0042-numerical-accumulation-and-comparisons.md).
3. Use `Formula` to reevaluate the formula and its mask on rolled inputs and target-period history. A lag of one reads one target period. Validate the target cadence and reject direct capital snapshot reads in target formulas and masks. Previously rolled capital-derived statement values can be used when their rollup method is explicit.
4. Link each result to the source run, effective method and contributing periods. Keep source diagnostics, check reports and capital snapshots traceable to their original periods. Target checks require their own evaluation against the rolled result. [Result provenance](13-result-units-and-exports.md#accepted-decision-for-typed-results-and-exports) · [Check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, Q1 and Q2 revenue of USD 100 and USD 300, profit of USD 20 and USD 30, and closing cash of USD 40 and USD 50 give H1 revenue of USD 400, profit of USD 50 and closing cash of USD 50. The H1 margin is 12.5%. If Q2 revenue is absent, H1 revenue and margin are unavailable and closing cash is still USD 50.

## Audited choice, domain and actual selection

Rollup transforms evaluated statement results to a strictly coarser nested monthly/quarterly/semiannual/annual grid. It does not support daily/weekly conversion. The effective method is explicit rollup-spec override, then the node's declared rollup method, then **Sum**. Tags do not infer a stock/flow/ratio method. Unknown override node IDs error. This is an observed default requiring deliberate use; it is not an assertion that every financial statement line should sum.

## Audited algorithm and conventions

1. Prepare/validate the model, group source periods using the source ID's position in the target frequency and preserve fiscal-year labels. Require the exact number of source periods per target bucket; incomplete buckets error. Target bounds run from the first source start to the last source end; target is actual only if all its source periods are actual.
2. **Sum:** require all source values to be present, then add with ordinary `f64` summation. Any absent source omits the target cell; present nonfinite values propagate. This branch does not use the compensated sum used by some formula statistics.
3. **PeriodEnd:** copy only the final source period's value. Earlier missing values do not by themselves prevent the final balance, unlike Sum.
4. **Formula:** require a node formula and reject capital references in either its formula or mask. Evaluate in target DAG order and target historical context. Apply `where` again only for Formula. Temporal expressions now operate in target periods: a lag of one in an annual rollup is one year. Do not regenerate original forecasts or reapply explicit source cell overrides.
5. Rebuild units/Money views using result metadata with prepared-model fallback. Result periods/metadata describe the coarser grid. Do not carry original capital snapshots, check report, cell sources or original warnings; new rollup calculations can generate fresh warnings.

## Audited rationale, limitations and failure behavior

Sum supports period flows, PeriodEnd supports balances and Formula supports ratios derived from aggregated inputs. This is a usage rationale, not automatic semantic classification: default Sum also applies to undeclared stocks, ratios or trailing measures. Caller/node selection is financially material. A Formula rollup can differ from summing source formula results by design. A target with any forecast source remains forecast even when some source periods are actual.

Ordinary accumulation in Sum is a concrete reconciliation concern under the accepted [numerical accumulation policy](../../standards/0042-numerical-accumulation-and-comparisons.md). No new numerical failure or independent magnitude/cancellation fixture was executed. A policy inferred from tags, weighted-average ratios, partial-period prorating or capital-aware recomputation would be different methods, none silently selected here.

## Evidence

Owning source: [rollup](../../../../finstack-quant/statements/src/evaluator/rollup.rs). Inspected [rollup tests](../../../../finstack-quant/statements/tests/evaluator_rollup.rs), especially `quarterly_results_roll_up_to_annual_flows_balances_and_ratios`: hand expectations include revenue 460/620, period-end balances, annual margin/leverage and annual-frequency lag growth, and explicit assertions that sources/capital snapshots are empty. The same file locates incomplete-year, fiscal-grid, declared-method and default-Sum cases. This is independent small-formula/contract evidence, not a general accounting certification. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
