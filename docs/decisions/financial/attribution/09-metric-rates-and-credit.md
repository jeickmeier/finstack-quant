# ATTRIBUTION-009: Metric rates and credit first- and second-order P&L

- Decision status: Accepted 2026-11-16 for checked metric rate and credit P&L.
- Implementation status: Observed for the source scope listed below. Runtime, producer, arithmetic and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked credit twists, matched spread and rate curvature, native intensity risk and checked product scaling. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked metric rate and credit P&L

1. Calculate signed first-order P&L as `Σ s_i m_i` over the complete ordered buckets, with `s_i` in holder currency per positive native bp and `m_i` in the matching bp. An elected aggregate sensitivity consumes one amplitude only when the full move matches its declared direction. Retire average-move fallbacks. Count each covered exposure once and preserve the producer's direct or replay basis, including its treatment of dependent hazards under rate shocks. [Accepted input and pairing contract](08-metric-inputs-and-move-pairing.md#accepted-decision-for-checked-metric-selection-horizon-matching-and-market-move-pairing).
2. Keep credit measures on their own bases. CDS CS01 uses matched par-risk quotes and the qualified replay center. `Hazard01` uses native intensity bp. Credit-role discount risk uses its own zero-rate basis. An eligible bond's instrument-level Z-spread CS01 consumes the actual matched Z move on the same fixed cash path, compounding and clock, with a qualified settlement-to-holder bridge. Scaling its one-sided one-bp price change is a finite-step approximation. [Accepted credit sensitivities](../valuations-framework/credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities) · [Accepted bond spread risk](../valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk).
3. Elect additive curvature explicitly. Use `0.5 K a²` only when the full realized move matches the producer's declared direction at amplitude `a`, in the units of `K`. For curvature in currency per decimal-rate or decimal-spread squared, `a = move_bp / 10000`. A zero first-order amount leaves matched curvature eligible. Bucketed first order can coexist with qualified parallel curvature when the entire curvature scope moves in that direction. Retire weighted and arithmetic mean proxies and the tiny-weight fallback. A missing or mismatched required producer or direction fails. Elected advisory detail may report typed unavailability and stays outside the additive total.
4. Use the actual rate curvature units and center. Raw `IrConvexity` is monetary and can contribute at zero opening PV. Convert a bond's unscaled `effective_convexity` to monetary curvature on its matching raw claim center and apply signed quantity once. Quoted-yield `convexity` belongs to the separate settlement dirty-price yield experiment. Retire the `×100` conversion, generic holder-PV multiplier and magnitude-based precedence. Combined curvature includes interactions within its selected direction. Exclude overlapping mixed terms from the additive total. [Accepted IRS risk](../valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk) · [Accepted bond curve risk](../valuations-bonds/15-curve-risk.md#accepted-decision-for-checked-bond-curve-and-effective-risk) · [Accepted bond yield risk](../valuations-bonds/14-yield-risk.md#accepted-decision-for-checked-quoted-yield-bond-risk).
5. Apply `CsGamma` on its qualified par-spread direction. The accepted single-name producer measures parallel par-spread curvature at its zero-shock replay center. Intensity, Z-spread and credit-role zero curvature require their own qualified producers. A nonparallel move needs a matching directional metric for additive curvature within MetricsBased. Taylor owns the separate directional repricing route. [Accepted CDS risk](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk) · [Accepted Taylor curve risk](06-taylor-curve-risk.md#accepted-decision-for-checked-taylor-curve-sensitivities-and-directional-gamma).
6. Publish checked amounts and factual diagnostics. Validate signed products and use producer-ordered checked Neumaier summation for binary64 bucket terms. Scale products to preserve representable results, combine monetary components with checked Money and reconcile detail to factor totals. Distinguish proven inapplicability, evaluated zero, advisory unavailability and required failure. Retire the fixed `100 bp`, `50 bp` and `1%` twist heuristics for these factors. Report actual move magnitudes, covered scope and direction-match status. Residual tolerance establishes neither coverage nor approximation accuracy. Rust owns the calculation and evidence projected through Python and WASM. [Accepted sums](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty) · [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

Exact examples verified a zero-mean `(+2, −2) bp` credit twist with `−6` first-order P&L and `+20` of quadratic P&L. A separate rate example gave `+2000` from unscaled effective convexity of `40` on a raw claim of `1000000` at `+100 bp`.

## Observed choice and formulas

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

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
