# STATEMENTS-EVALUATION-014 — Flow, balance and formula rollup to coarser periods

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, domain and actual selection

Rollup transforms evaluated statement results to a strictly coarser nested monthly/quarterly/semiannual/annual grid. It does not support daily/weekly conversion. The effective method is explicit rollup-spec override, then the node's declared rollup method, then **Sum**. Tags do not infer a stock/flow/ratio method. Unknown override node IDs error. This is an observed default requiring deliberate use; it is not an assertion that every financial statement line should sum.

## Algorithm and conventions

1. Prepare/validate the model, group source periods using the source ID's position in the target frequency and preserve fiscal-year labels. Require the exact number of source periods per target bucket; incomplete buckets error. Target bounds run from the first source start to the last source end; target is actual only if all its source periods are actual.
2. **Sum:** require all source values to be present, then add with ordinary `f64` summation. Any absent source omits the target cell; present nonfinite values propagate. This branch does not use the compensated sum used by some formula statistics.
3. **PeriodEnd:** copy only the final source period's value. Earlier missing values do not by themselves prevent the final balance, unlike Sum.
4. **Formula:** require a node formula and reject capital references in either its formula or mask. Evaluate in target DAG order and target historical context. Apply `where` again only for Formula. Temporal expressions now operate in target periods: a lag of one in an annual rollup is one year. Do not regenerate original forecasts or reapply explicit source cell overrides.
5. Rebuild units/Money views using result metadata with prepared-model fallback. Result periods/metadata describe the coarser grid. Do not carry original capital snapshots, check report, cell sources or original warnings; new rollup calculations can generate fresh warnings.

## Rationale, limitations and failure behavior

Sum supports period flows, PeriodEnd supports balances and Formula supports ratios derived from aggregated inputs. This is a usage rationale, not automatic semantic classification: default Sum also applies to undeclared stocks, ratios or trailing measures. Caller/node selection is financially material. A Formula rollup can differ from summing source formula results by design. A target with any forecast source remains forecast even when some source periods are actual.

Ordinary accumulation in Sum is a concrete reconciliation concern under the accepted [numerical accumulation policy](../../standards/0042-numerical-accumulation-and-comparisons.md). No new numerical failure or independent magnitude/cancellation fixture was executed. A policy inferred from tags, weighted-average ratios, partial-period prorating or capital-aware recomputation would be different methods, none silently selected here.

## Evidence

Owning source: [rollup](../../../../finstack-quant/statements/src/evaluator/rollup.rs). Inspected [rollup tests](../../../../finstack-quant/statements/tests/evaluator_rollup.rs), especially `quarterly_results_roll_up_to_annual_flows_balances_and_ratios`: hand expectations include revenue 460/620, period-end balances, annual margin/leverage and annual-frequency lag growth, and explicit assertions that sources/capital snapshots are empty. The same file locates incomplete-year, fiscal-grid, declared-method and default-Sum cases. This is independent small-formula/contract evidence, not a general accounting certification. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
