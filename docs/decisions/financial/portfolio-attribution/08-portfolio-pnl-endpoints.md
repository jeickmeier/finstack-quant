# PORTFOLIO-ATTRIBUTION-008: Portfolio P&L endpoint preparation, primitive reduction and aggregation

- ID: PORTFOLIO-ATTRIBUTION-008
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and ownership boundary

The portfolio adapter composes the separate attribution crate's methods. It owns endpoint profiles, position scaling, composite primitive reduction, portfolio currency aggregation and reporting. Waterfall ordering, parallel factor isolation, Taylor/metric kernels and shared translation internals belong to the [attribution scope](../attribution/README.md); this record does not claim ownership of those kernels.

## Ordered algorithm and controls

1. Prepare the same portfolio at T0 and T1 using the evaluation plan. MetricsBased requests `default_attribution_metrics` at both endpoints with strict risk collection. Repricing methods request an empty metrics-only set with non-strict risk collection. Preparation errors propagate in input order.
2. Before reuse, validate endpoint stamps against position ID, instrument ID and date; MetricsBased also requires the complete profile. This is an input-identity check, not a fresh repricing.
3. For a plain repricing position, invoke the selected attribution method with prepared endpoints and caller configuration using `strict_validation=false`. MetricsBased calls its dedicated kernel with prepared metric results; caller configuration has already controlled endpoint preparation. Preserve the returned invalid flag rather than hiding it behind a portfolio reconciliation total.
4. For a composite, flatten the frozen primitive exposures, reconstruct each primitive instrument from its JSON representation, compute its endpoint values and attribute each primitive separately. Scale signed results by leaf quantity, translate to composite reporting currency, accumulate the components and compute composite residual from the aggregate total. This freezes composition for the interval; it does not infer trading or changing constituents.
5. Scale each position report by `Position::scale_factor()` rather than raw quantity. Keep the by-position report in its own reporting currency. Translate a copy to portfolio base currency for aggregate components, with native embedded FX separate from translation FX.
6. Shared translation uses T0 and T1 FX for opening/closing endpoints, converts signed factor/detail components at T1 FX and adds opening-value FX translation. The detailed contract is [target currency and scaling](../attribution/13-target-currency-and-scaling.md); no extra portfolio translation formula is introduced.
7. Reconcile all twelve monetary buckets, including cross-factor, translation and residual, using compensated accumulation. The result is reconciled only when no constituent invalid flag is set and absolute difference is within caller tolerance. Percentage shares use the default currency rounding zero tolerance; structured shares are `None` for zero total, while explanation text displays zero percent.

## Chained composite translation discrepancy

A composite whose reporting currency differs from portfolio base reaches translation twice: foreign primitive to composite, then composite to portfolio. The shared translator replaces an existing `fx_translation_pnl` rather than carrying it forward. Static algebra therefore indicates that the inner opening-principal translation moves into residual at the second conversion. For example, a flat EUR leaf with moving EUR/USD inside a USD composite produces a USD translation effect; converting the composite to CAD at constant USD/CAD replaces that effect with zero outer FX translation and derives it as residual. Headline P&L still reconciles. The existing composite FX fixture uses the same USD reporting currency for composite and portfolio and does not qualify this branch. This impact is derived from source, not reproduced by execution.

## Alternatives and evidence

Portfolio-wide factor shocks with changing holdings or cashflow-aware trading attribution differ from summing frozen primitive/position reports. Native position reports preserve detail but cannot be summed across currencies without the documented translation.

[Owning source and inline composite/prepared-endpoint tests](../../../../finstack-quant/portfolio/src/attribution.rs) cover quantity/FX aggregation, endpoint reuse, cross-factor reconciliation and invalid propagation. [Golden fixtures](../../../../finstack-quant/portfolio/tests/attribution_golden.rs) supply regression evidence; shared kernels and FX translation are separately audited. No tests ran.

Reconciliation proves arithmetic closure only, not causal correctness of component labels.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
