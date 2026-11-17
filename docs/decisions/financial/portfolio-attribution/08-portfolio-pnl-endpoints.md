# PORTFOLIO-ATTRIBUTION-008: Portfolio P&L endpoint preparation, primitive reduction and aggregation

- ID: PORTFOLIO-ATTRIBUTION-008
- Decision status: Accepted 2026-11-16 for checked portfolio P&L attribution.
- Implementation status: Observed for the stated owning source scope. Runtime, shared translator, caller and host reconciliation with the accepted contract are pending.
- Validation status: Independent endpoint, nested FX, cash, signed scaling, reconciliation and percentage examples were checked. Production, integration and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked portfolio P&L attribution

1. Require an explicit method, one ordered frozen holding and composition state, and immutable opening and closing market states with valuation cuts. Bind effective pricing dates, model, settings, registry, provider and FX basis. Reuse a prepared endpoint only when its required profile and provenance match. Reject reversed dates before pricing. Admit an explicit ordered same-date market comparison with zero elapsed carry and period cash. A validated empty book yields base-currency zero. [Portfolio execution and NAV reduction](../portfolio-execution/04-execution-order-and-reduction.md).
2. Parallel, Waterfall and Taylor use canonical PV-only endpoints. MetricsBased requires strict applicable opening metrics, with carry measured over the actual interval, and closing PV. Confirmed structural inapplicability needs no substitute. Missing or failed applicable metrics fail the request. Use the same shared method dispatch and settings for plain positions and composite leaves. Remove the adapter's hard-coded `strict_validation=false` override. [Shared endpoint and method owner](../attribution/01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).
3. Use the actual prepared valuation tree with every ordered leaf occurrence and child-owned pricing controls. Apply signed holding scale and each frozen leg quantity once along the path. Follow the authored child-to-parent currency route and verify opening and closing reductions independently against their prepared marks. [Accepted primitive exposure rules](../portfolio-execution/11-primitive-exposures.md).
4. At each conversion edge use the qualified dated cash rates `q0,q1` for the corresponding marks. Carry reporting FX forward as `q1*inherited_reporting_FX + opening_value*(q1-q0)`. Translate signed factors and existing residual at closing FX. Keep embedded pricing FX separate. Use the shared translator at every edge. Preserve cash-owner eligibility and the native cash basis. Label the closing-rate translation of admitted cash as closing-equivalent attribution cash. Replay MTM and settlement-date performance have their own bases. [Portfolio FX rules](../portfolio-execution/08-spot-and-forward-fx.md). [Shared translation requirement](../attribution/13-target-currency-and-scaling.md#accepted-portfolio-requirement-for-composable-reporting-fx).
5. Retain ordered position reports in their own native or composite reporting currencies. Aggregate opening and closing marks, MTM, eligible cash and all twelve P&L buckets in checked base-currency Money. Keep traceable conversion rounding separate from economic residual. Compare monetary reconciliation difference to an explicit nonnegative base-currency Money tolerance. Give required failures endpoint, position and leaf-path context. Preserve constituent invalid flags and diagnostics through cancellation. Withhold reconciled aggregate qualification and percentage shares for invalid results.
6. Compute signed bucket shares from the Money ratio. For a valid report an exact zero headline makes shares unavailable. Nonzero amounts below display precision remain eligible. Use the same availability in text and structured outputs. Retain endpoint, method, profile, FX and coverage evidence in an ordered Rust result. Python, WASM and UI restoration use checked source evidence.

Independent checks reproduced two currency routes with marks `USD 100 → 120` and `USD 130 → 150` that share `USD 20` P&L. Each endpoint must reconcile independently. A flat `EUR 100` leaf with EUR/USD `1.10 → 1.20` and USD/CAD `1.30 → 1.40` yields `CAD 143 → 168`. The accepted reporting FX is `CAD 25`, including `14` from the inner edge and `11` from the outer edge. The observed shared translation sequence places the inner `14` in residual.

## Observed choice and ownership boundary

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

Implementation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
