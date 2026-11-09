# ATTRIBUTION-009: Metric rates and credit first- and second-order P&L

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and formulas

Rate and credit buckets pair opening sensitivities in money/bp with actual per-bucket bp moves. Sum products with Neumaier accumulation. If there are no buckets, use aggregate DV01 or CS01 times the unweighted average measurable curve move; missing move produces zero with a note. Missing sensitivity while a measurable aggregate move exists also leaves zero and notes residual exposure. Bucket parsing and completeness are in [record 8](08-metric-inputs-and-move-pairing.md).

Rate convexity uses a scalar representative shift. With keyrates it is `sum(abs(DV01_i)*move_i)/sum(abs(DV01_i))`, falling back to the arithmetic bucket mean if absolute sensitivity weight is at most 1e-12. Without buckets it is the aggregate mean. Street `Convexity` takes precedence over `IrConvexity` when materially nonzero under a default rounding context: contribution=`0.5*PV0*Convexity*100*(move_bp/10000)²`. The factor 100 reverses the bond producer's street-convexity scale. Otherwise raw dollar `IrConvexity` contributes `0.5*IrConvexity*(move_bp/10000)²`; no PV normalization is applied to a near-par swap.

Credit gamma is conditional on first-order credit data and a measurable representative shift. Bucketed credit uses an unweighted arithmetic mean of bucket shifts (unlike rate convexity's absolute-DV01 weighting); aggregate credit uses mean curve move. Contribution=`0.5*CsGamma*(mean_bp/10000)²`, because CsGamma is money per decimal-spread squared. It does not reconstruct a key-rate Hessian.

## Diagnostics and controls

Rate mean moves beyond 100 bp and credit means beyond 50 bp add large-move notes. A twist diagnostic compares absolute signed mean with 1% of the mean absolute tenor move; cancellation below that threshold adds an unreliability note. The notes do not invalidate, bound approximation error or change calculated P&L. Missing measurements in the diagnostic can become zero. Nonfinite factor Money conversion sets invalid and uses zero sentinel. Debug assertions for convexity finiteness can panic on direct debug-build calls before this conversion.

## Rationale and alternatives

Buckets retain first-order nonparallel exposure; the scalar curvature terms are cheaper approximations. Source comments about specific percentage accuracy thresholds are not an independently established error bound. Taylor directional gamma and full repricing are supported alternatives. Located tests cover rate deduplication, keyrate steepeners and DV01-weighted convexity; these are targeted synthetic and implementation checks.

## Source and evidence

- [QuantLib attribution parity](../../../../finstack-quant/attribution/tests/attribution/quantlib_parity.rs) — existing stored external-reference expectations for bond/IRS/FX-forward cases, with per-component and residual bounds; test source sampled but not executed or fixture-regenerated. This evidence has a different role from same-library analytical consistency tests and does not qualify all curve shapes or products.
- [metrics_based/rates.rs](../../../../finstack-quant/attribution/src/metrics_based/rates.rs) — owning implementation.
- [metrics_based/credit.rs](../../../../finstack-quant/attribution/src/metrics_based/credit.rs) — owning implementation.
- [metrics_based/shifts.rs](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs) — owning implementation.

## Relationship to the accepted comparable-measurement package

[CORE-MARKET-22's accepted package](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
was accepted on **2026-11-08**. It distinguishes a homogeneous equal-weight diagnostic mean from
producer-defined risk coordinates. Retain complete per-bucket movements and
explicit parameter-versus-quote basis; a zero mean can conceal a twist, and
aggregate sensitivity times a mean needs separately qualified mapping or an
explicit approximation. Missing required rows are errors at measurement, not
unchanged inputs. This does not adopt this record's curvature/gamma proxies,
warning thresholds, P&L allocation or accuracy claims.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
