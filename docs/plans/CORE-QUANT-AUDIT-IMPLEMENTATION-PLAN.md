# Core quantitative audit implementation plan

## Scope and sequence

This plan covers the five Blocker and fifteen High findings in the November 17 core audit. The 18 slices below are in delivery order. Each dependency lists an earlier slice whose conventions or interfaces the later slice should consume. Independent slices can proceed separately. The dividend and tranche slices each resolve two findings through a shared data contract. Slice numbers refer to this plan. The finding titles preserve the original audit severity and step.

The existing [core conventions reconciliation plan](CORE-CONVENTIONS-RECONCILIATION-PLAN.md) and [status](CORE-CONVENTIONS-RECONCILIATION-STATUS.md) track earlier work. Use their established owners and the current code when implementing. Each slice delivers a working Rust path through its existing consumers and public hosts in one commit.

## Verification for every slice

1. Encode the stated counterexample in the owning regression suite and exercise it against the current implementation. Use the expected contractual amount, formula or error as the oracle. Cross-host agreement alone is insufficient.
2. Implement in the owning Rust crate. The Python and WASM paths listed below are entry points to update or exercise. Keep computation in Rust. For host-only transport of a Rust calculation, verify restoration in each host and the numerical result at the Rust consumer.
3. For signature or wire changes, replace obsolete state and update all callers, constructors, serialized fixtures, schema, examples, Python stubs under `finstack-quant-py/finstack_quant/`, the parity ledger, and affected JS facades under `finstack-quant-wasm/exports/`. Run `mise run gen-write` before the gate for generator, wire, binding or facade changes. Regenerate declarations and affected UI fixtures together.
4. Run `mise run rust-lint-crate -- <package>` and `mise run rust-test-filter -- <package> <filter>` or `mise run rust-test-integration -- <package> <test>` for each touched financial owner. Build the current Python extension with `mise run python-build`, then run the affected files via `mise run python-test-file -- <file-or-node-id>`. Use `mise run wasm-test` for binding changes and assert the example through the public JS facade. Run `mise run slice-gate` before each commit.
5. After all slices pass, run `mise run all` once. Record the counterexample results for each slice, distinguishing Rust, Python and WASM checks and any Rust-only checks. This document records planned checks.

## 1. Calendars and session rolls

**Depends on** no earlier slice.

- [High] Step 2 — Trading calendars misclassify both closures and early closes

Apply amended NSE, NYSE and SIFMA dates in their data owners, retaining early closes as sessions. Keep amendments and qualified historical coverage with the datasets.

- **Rust** — [finstack-quant/core/data/calendars/nse.json](../../finstack-quant/core/data/calendars/nse.json) · [finstack-quant/core/data/calendars/sifma.json](../../finstack-quant/core/data/calendars/sifma.json) · [finstack-quant/core/data/calendars/nyse.json](../../finstack-quant/core/data/calendars/nyse.json)
- **Python** — [finstack-quant-py/src/bindings/core/dates/calendar.rs](../../finstack-quant-py/src/bindings/core/dates/calendar.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/calendar.rs](../../finstack-quant-wasm/src/api/core/calendar.rs)

**Acceptance.** Assert next NSE session from Jan 14 2026 is Jan 16, NYSE Following from Oct 29 2012 is Oct 31, and SIFMA Following from Dec 31 2021 stays Dec 31. Run all three through Rust and each host.

## 2. Contractual ICMA roll

**Depends on** slice 1.

- [Blocker] Step 4 — Long ACT/ACT ICMA coupons can pay the wrong amount

Pass the original anchor and explicit EOM flag through fixed-coupon emission and reuse the schedule-aware slice calculation. Emit single-reference accrual metadata only for a period contained by that reference.

- **Rust** — [finstack-quant/cashflows/src/builder/emission/coupons.rs](../../finstack-quant/cashflows/src/builder/emission/coupons.rs) · [finstack-quant/cashflows/src/builder/periods.rs](../../finstack-quant/cashflows/src/builder/periods.rs) · [finstack-quant/cashflows/src/builder/date_generation.rs](../../finstack-quant/cashflows/src/builder/date_generation.rs) · [finstack-quant/core/src/cashflow/primitives.rs](../../finstack-quant/core/src/cashflow/primitives.rs)
- **Python** — [finstack-quant-py/src/bindings/cashflows/builder/schedule.rs](../../finstack-quant-py/src/bindings/cashflows/builder/schedule.rs) · [finstack-quant-py/src/bindings/core/cashflow.rs](../../finstack-quant-py/src/bindings/core/cashflow.rs)
- **WASM** — [finstack-quant-wasm/src/api/cashflows/schedule.rs](../../finstack-quant-wasm/src/api/cashflows/schedule.rs) · [finstack-quant-wasm/src/api/core/cashflow.rs](../../finstack-quant-wasm/src/api/core/cashflow.rs)

**Acceptance.** Build the audited unadjusted annual Feb 28 schedule, Jun 30 2024 to Feb 28 2027, long front, EOM false. Compare the first cash amount with USD 5m × (1 + 243/366) = 8,319,672.13 in Rust, Python and WASM. Also exercise EOM true to catch unconditional fixed-day rolling.

## 3. Historical fixing identity

**Depends on** no earlier slice.

- [Blocker] Step 5 — Changing a projection curve can change an already-fixed coupon

Require an observed index on floating specifications independently of the projection curve. Thread it through historical lookup, schedule generation, waterfall, wire types and builders. Preserve the future-reset projection route.

- **Rust** — [finstack-quant/core/src/market_data/fixings.rs](../../finstack-quant/core/src/market_data/fixings.rs) · [finstack-quant/cashflows/src/builder/specs/coupon.rs](../../finstack-quant/cashflows/src/builder/specs/coupon.rs) · [finstack-quant/cashflows/src/builder/specs/rate.rs](../../finstack-quant/cashflows/src/builder/specs/rate.rs) · [finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs](../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs)
- **Python** — [finstack-quant-py/src/bindings/cashflows/builder/specs/coupon.rs](../../finstack-quant-py/src/bindings/cashflows/builder/specs/coupon.rs) · [finstack-quant-py/src/bindings/core/market_data/context.rs](../../finstack-quant-py/src/bindings/core/market_data/context.rs)
- **WASM** — [finstack-quant-wasm/src/api/cashflows/specs.rs](../../finstack-quant-wasm/src/api/cashflows/specs.rs) · [finstack-quant-wasm/src/api/core/market_context.rs](../../finstack-quant-wasm/src/api/core/market_context.rs)

**Acceptance.** Load 4% contractual and 5% proxy fixings for the same reset. On USD 100m, 90/360, unit gearing and zero spread, require USD 1m after changing the projection curve. Remove the contractual fixing under strict policy and require an error. Exercise host-built terms against Rust.

## 4. Dated and numeric price queries

**Depends on** no earlier slice.

- [Blocker] Step 6 — A volatility-index curve returns a negative futures fair level

Make price evaluation fallible. Use it for dates, numeric queries and the listed-future fair price. Enforce nonnegative evaluated VolIndex levels and retain signed commodity prices.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/price_curve.rs](../../finstack-quant/core/src/market_data/term_structures/price_curve.rs) · [finstack-quant/valuations/equity/src/vol_index_future/types.rs](../../finstack-quant/valuations/equity/src/vol_index_future/types.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/price.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/price.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_curves.rs](../../finstack-quant-wasm/src/api/core/market_curves.rs)

**Acceptance.** With levels 30 at spot and 20 at 0.25 years, require the one-year linear extension that gives −10 to error through both host query forms and Rust fair price. Require an error under the Error tail policy as well. Check both pillar levels and a valid negative commodity price.

## 5. Dividend identity, entitlement and payment

**Depends on** no earlier slice.

- [Blocker] Step 1 — A foreign-currency dividend is subtracted from the equity spot at face value
- [High] Step 7 — Equity forwards discount cash dividends to the ex-date

Replace the single-date event with identified cash per share, underlying/share basis, currency, ex-date and pay date. Validate at construction and snapshot restoration. Preserve spot currency through equity calibration, require an explicitly qualified same-currency amount, select entitlement on ex-date and discount to pay date.

- **Rust** — [finstack-quant/core/src/market_data/dividends.rs](../../finstack-quant/core/src/market_data/dividends.rs) · [finstack-quant/calibration/src/targets/util.rs](../../finstack-quant/calibration/src/targets/util.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/context.rs](../../finstack-quant-py/src/bindings/core/market_data/context.rs) · [finstack-quant-py/src/bindings/calibration/mod.rs](../../finstack-quant-py/src/bindings/calibration/mod.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_context.rs](../../finstack-quant-wasm/src/api/core/market_context.rs) · [finstack-quant-wasm/src/api/calibration/mod.rs](../../finstack-quant-wasm/src/api/calibration/mod.rs)

**Acceptance.** Reject the USD 100 share / JPY 10 dividend as supplied. At JPY 100 per USD, explicitly converted USD 0.10 must give 99.90 at unit factors. Separately use a USD 10 dividend with ex/pay factors .99/.98 and expiry after ex but before pay. Require a 9.80 deduction. Round-trip the event through both host market loaders and repeat the equity-forward check.

## 6. Tranche detachment and stress magnitude

**Depends on** no earlier slice.

- [Blocker] Step 9 — A 700 bp tranche shock moves the wrong loss layer
- [High] Step 9 — Correlation clipping reduces the requested stress without failing

Convert detachment bp to percent once at the scenario boundary. Resolve every requested bucket and prevalidate every resulting correlation. Remove clipping from both parallel and bucketed core paths. Publish the bump only after all checks pass.

- **Rust** — [finstack-quant/core/src/market_data/bumps.rs](../../finstack-quant/core/src/market_data/bumps.rs) · [finstack-quant/scenarios/src/adapters/basecorr.rs](../../finstack-quant/scenarios/src/adapters/basecorr.rs) · [finstack-quant/core/src/market_data/term_structures/base_correlation.rs](../../finstack-quant/core/src/market_data/term_structures/base_correlation.rs) · [finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs](../../finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs) · [finstack-quant/core/src/market_data/context/ops_bump.rs](../../finstack-quant/core/src/market_data/context/ops_bump.rs)
- **Python** — [finstack-quant-py/src/bindings/scenarios/operation_spec/spec.rs](../../finstack-quant-py/src/bindings/scenarios/operation_spec/spec.rs) · [finstack-quant-py/src/bindings/core/market_data/curves/credit.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/credit.rs)
- **WASM** — [finstack-quant-wasm/src/api/scenarios/operation_spec.rs](../../finstack-quant-wasm/src/api/scenarios/operation_spec.rs) · [finstack-quant-wasm/src/api/core/market_curves.rs](../../finstack-quant-wasm/src/api/core/market_curves.rs)

**Acceptance.** For [0.07,3,7,10] percent and correlations [.20,.25,.45,.60], apply +.05 at 700 bp. Require [.20,.25,.50,.60] through both hosts. Reject an unmatched target, including one in a partially matching request. At .98 reject +.10 through direct and scenario paths, verifying every original knot survives the failure.

## 7. Annualized curve clocks

**Depends on** no earlier slice.

- [High] Step 4 — A contractual 1/1 day count collapses all dated maturities

Reject OneOne in annualized term-structure and volatility clocks on all construction and restore paths. Retain its contractual coupon semantics.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/common/clock.rs](../../finstack-quant/core/src/market_data/term_structures/common/clock.rs) · [finstack-quant/core/src/dates/volatility_clock.rs](../../finstack-quant/core/src/dates/volatility_clock.rs) · [finstack-quant/core/src/dates/daycount/mod.rs](../../finstack-quant/core/src/dates/daycount/mod.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/discount.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/discount.rs) · [finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_discount.rs](../../finstack-quant-wasm/src/api/core/market_discount.rs) · [finstack-quant-wasm/src/api/core/surfaces.rs](../../finstack-quant-wasm/src/api/core/surfaces.rs)

**Acceptance.** The 5% OneOne curve must fail construction on each host before it can return .9512294245 for tomorrow, one year and ten years. Reject a serialized OneOne volatility clock. Check dated factors on an ACT/365F curve against exp(−.05t), and verify a OneOne coupon still gives one unit.

## 8. Named credit pool membership

**Depends on** no earlier slice.

- [High] Step 5 — An unknown issuer receives a weight and hazard curve in a named credit pool

Resolve named-member curve, recovery and weight through a checked roster lookup. Keep homogeneous pool assumptions explicit. Propagate missing-member errors to callers and hosts.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/credit_index.rs](../../finstack-quant/core/src/market_data/term_structures/credit_index.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/credit.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/credit.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_curves.rs](../../finstack-quant-wasm/src/api/core/market_curves.rs)

**Acceptance.** Build the A/B 90%/10% pool. All three C lookups must fail through both hosts. Check the known-member weights, curves and applicable recoveries, and separately verify the homogeneous pool API.

## 9. Reporting period restoration

**Depends on** no earlier slice.

- [High] Step 3 — A duplicate daily period can hide a missing day in trailing EBITDA

Validate canonical bounds, order, uniqueness and period kind at construction and restore. Protect validated state and count distinct expected daily slots in trailing windows.

- **Rust** — [finstack-quant/core/src/dates/periods/mod.rs](../../finstack-quant/core/src/dates/periods/mod.rs) · [finstack-quant/core/src/dates/periods/ltm.rs](../../finstack-quant/core/src/dates/periods/ltm.rs) · [finstack-quant/statements-analytics/src/analysis/reports.rs](../../finstack-quant/statements-analytics/src/analysis/reports.rs)
- **Python** — [finstack-quant-py/src/bindings/core/dates/periods.rs](../../finstack-quant-py/src/bindings/core/dates/periods.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/periods.rs](../../finstack-quant-wasm/src/api/core/periods.rs)

**Acceptance.** Replace Jan 2 2025 with a second Jan 1 in the 365-row plan. Reject restoration at the public loading paths. A sparse valid plan missing Jan 2 must never yield complete trailing EBITDA. Check a complete plan with distinct daily EBITDA against its direct sum and leverage.

## 10. CPI series comparability

**Depends on** no earlier slice.

- [High] Step 5 — CPI comparison labels a change of index basis as a revision

Carry authoritative series, reference-base and seasonal-adjustment identity with observations and projected sources at the comparison boundary. Validate compatibility before subtraction and revision classification.

- **Rust** — [finstack-quant/core/src/market_data/diff.rs](../../finstack-quant/core/src/market_data/diff.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/inflation.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/inflation.rs) · [finstack-quant-py/src/bindings/core/market_data/scalars.rs](../../finstack-quant-py/src/bindings/core/market_data/scalars.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_curves.rs](../../finstack-quant-wasm/src/api/core/market_curves.rs) · [finstack-quant-wasm/src/api/core/market_scalars.rs](../../finstack-quant-wasm/src/api/core/market_scalars.rs)

**Acceptance.** Rust must reject 105 and 210 for the same month on different bases, even when currency matches. A qualified common-basis comparison of the unchanged level returns zero. Verify a genuine same-basis revision and reject mixed seasonal status. Check metadata round-trip through host market inputs. The comparison itself is Rust-only.

## 11. SABR cube interpolation

**Depends on** no earlier slice.

- [High] Step 6 — Mixed-beta SABR interpolation creates a 48-point volatility jump

Preserve exact nodes. Reject off-grid interpolation when contributing nodes have different betas. Retain interpolation on supported common-beta cells.

- **Rust** — [finstack-quant/models/src/volatility/source.rs](../../finstack-quant/models/src/volatility/source.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs) · [finstack-quant-py/src/bindings/models/volatility.rs](../../finstack-quant-py/src/bindings/models/volatility.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/surfaces.rs](../../finstack-quant-wasm/src/api/core/surfaces.rs) · [finstack-quant-wasm/src/api/models/volatility.rs](../../finstack-quant-wasm/src/api/models/volatility.rs)

**Acceptance.** Use the audit’s one-year expiry, F=K=.04, zero rho and nu, with 5Y (.04,.5) and 10Y (.20,1) alpha/beta. Both hosts must preserve endpoint vols .2000833333 and .20, and reject 7.5 and 7.50000001. Verify continuity on a common-beta control.

## 12. NDF settlement classification

**Depends on** no earlier slice.

- [High] Step 7 — An NDF settlement creates outstanding principal

Add DerivativeSettlement as cash-only in core, wire and hosts. Use it in the NDF producer and update exhaustive classifications, cash exports and principal consumers.

- **Rust** — [finstack-quant/valuations/fx/src/ndf/types.rs](../../finstack-quant/valuations/fx/src/ndf/types.rs) · [finstack-quant/core/src/cashflow/primitives.rs](../../finstack-quant/core/src/cashflow/primitives.rs)
- **Python** — [finstack-quant-py/src/bindings/core/cashflow.rs](../../finstack-quant-py/src/bindings/core/cashflow.rs) · [finstack-quant-py/src/bindings/valuations/pricing.rs](../../finstack-quant-py/src/bindings/valuations/pricing.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/cashflow.rs](../../finstack-quant-wasm/src/api/core/cashflow.rs) · [finstack-quant-wasm/src/api/valuations/pricing.rs](../../finstack-quant-wasm/src/api/valuations/pricing.rs)

**Acceptance.** Export the CNY 10m NDF, contract 7.25 and fixing 7.30. Expect signed USD −9,447.33, derivative_settlement, absent principal delta/date, zero outstanding principal and no repayment contribution to weighted-average life. Verify cash PV reconciliation and exported host rows.

## 13. Asset-swap floating accrual

**Depends on** slice 3 and slice 7.

- [High] Step 7 — Bond asset-swap spreads inherit the forward curve’s day count

Resolve the ASW contract’s floating index and accrual basis. Pass contractual accrual into both forward-components and ordinary quote paths, using the curve clock solely to evaluate dated factors. Use the observed index for seasoned resets.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs](../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs) · [finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs](../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs)
- **Python** — [finstack-quant-py/src/bindings/valuations/bond_quotes.rs](../../finstack-quant-py/src/bindings/valuations/bond_quotes.rs) · [finstack-quant-py/src/bindings/valuations/pricing.rs](../../finstack-quant-py/src/bindings/valuations/pricing.rs)
- **WASM** — [finstack-quant-wasm/src/api/valuations/pricing.rs](../../finstack-quant-wasm/src/api/valuations/pricing.rs) · [finstack-quant-wasm/src/api/valuations/typed](../../finstack-quant-wasm/src/api/valuations/typed)

**Acceptance.** Use ACT/360 coupons projected on ACT/365F factors. Independently sum contractual fractions × payment discount factors. With fixed numerator equal to .05 times that annuity require 500 bp, not 506.94 bp. Exercise Rust quote and spread metrics plus published host routes, then check price/spread inversion.

## 14. Expression errors in composite quantities

**Depends on** no earlier slice.

- [High] Step 8 — An undefined formula can initialize a short position

Implement the accepted divide-by-zero and missing-comparison rules in the core expression evaluator, including three-valued boolean operations. Propagate errors through initialization and rebalancing before applying positions.

- **Rust** — [finstack-quant/core/src/expr/eval.rs](../../finstack-quant/core/src/expr/eval.rs) · [finstack-quant/valuations/src/instruments/composite/spec.rs](../../finstack-quant/valuations/src/instruments/composite/spec.rs)
- **Python** — [finstack-quant-py/src/bindings/valuations/composite.rs](../../finstack-quant-py/src/bindings/valuations/composite.rs)
- **WASM** — [finstack-quant-wasm/src/api/valuations/composite.rs](../../finstack-quant-wasm/src/api/valuations/composite.rs)

**Acceptance.** With two USD 100 equities and quantity if(spread/abs(spread)>0,1,-1), both hosts must reject initialization and preserve existing holdings on a failed rebalance. Verify positive/negative spreads choose the intended quantities and missing comparisons remain unknown.

## 15. Full-window realized variance

**Depends on** no earlier slice.

- [High] Step 8 — OHLC realized variance drops the first target period

Add a separate preceding close to the OHLC estimator for close-to-close and Yang–Zhang. Use all target bars, represent insufficient history explicitly, and propagate window and availability semantics through swap settlement and both hosts.

- **Rust** — [finstack-quant/core/src/math/stats/realized.rs](../../finstack-quant/core/src/math/stats/realized.rs) · [finstack-quant/valuations/equity/src/variance_swap/pricer.rs](../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs)
- **Python** — [finstack-quant-py/src/bindings/core/math/stats.rs](../../finstack-quant-py/src/bindings/core/math/stats.rs) · [finstack-quant-py/src/bindings/valuations/pricing.rs](../../finstack-quant-py/src/bindings/valuations/pricing.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/math.rs](../../finstack-quant-wasm/src/api/core/math.rs) · [finstack-quant-wasm/src/api/valuations/pricing.rs](../../finstack-quant-wasm/src/api/valuations/pricing.rs)

**Acceptance.** Use preceding close 100 and three target bars flat at 110. Both hosts and Rust must return 252 × ln(1.1)² / 3 ≈ .76305855 for Yang–Zhang and OHLC close-to-close. Settle a swap from that estimate. Check absent anchor and an insufficient Yang–Zhang sample cannot settle as zero.

## 16. Joint-event probabilities

**Depends on** no earlier slice.

- [High] Step 8 — The joint-default model replaces feasible perfect dependence with independence

Reserve deterministic handling for exact zero or one marginals, preserve feasible endpoint dependence and reject infeasible correlations.

- **Rust** — [finstack-quant/core/src/math/probability.rs](../../finstack-quant/core/src/math/probability.rs)
- **Python** — [finstack-quant-py/src/bindings/models/correlation/mod.rs](../../finstack-quant-py/src/bindings/models/correlation/mod.rs)
- **WASM** — [finstack-quant-wasm/src/api/models/correlation.rs](../../finstack-quant-wasm/src/api/models/correlation.rs)

**Acceptance.** For p=2⁻³⁵ and p=2⁻⁴⁰, with equal marginals and rho=1, require joint=p and conditional=1 in both hosts. Check rho=0 gives joint=p² and infeasible rho fails.

## 17. Rate-quote calibration replay

**Depends on** slice 3 and slice 7.

- [High] Step 9 — Rate-quote replay can attach bumped quotes to a curve that does not fit them

Persist executed settings, resolved quote and dependency bindings, original evaluator and fit acceptance. Verify an unshocked reconstruction before replay. Return the accepted recalibration with its new record. Remove the ratio and rate-delta overlays.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/rate_calibration.rs](../../finstack-quant/core/src/market_data/term_structures/rate_calibration.rs) · [finstack-quant/calibration/src/recalibration/rates.rs](../../finstack-quant/calibration/src/recalibration/rates.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/discount.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/discount.rs) · [finstack-quant-py/src/bindings/core/market_data/curves/forward.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/forward.rs) · [finstack-quant-py/src/bindings/calibration/mod.rs](../../finstack-quant-py/src/bindings/calibration/mod.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_discount.rs](../../finstack-quant-wasm/src/api/core/market_discount.rs) · [finstack-quant-wasm/src/api/core/market_forward.rs](../../finstack-quant-wasm/src/api/core/market_forward.rs) · [finstack-quant-wasm/src/api/calibration/mod.rs](../../finstack-quant-wasm/src/api/calibration/mod.rs)

**Acceptance.** Reject a source factor .90 whose recorded quotes re-fit to .95 before any bump is published. With a matching .95 source and quotes that re-fit to .9499 after the bump, require .9499, reprice the inputs and verify the unit-cashflow change is −.0001. Repeat after host snapshot restoration and cover projection replay.

## 18. CDS risk-quote identity

**Depends on** no earlier slice.

- [High] Step 9 — A restored CDS risk recipe can shock the wrong maturity

Bind each original and spread-risk quote to one validated CDS contract. Verify maturity, entity, currency, recovery and convention across the pair at attachment and restore. Keep the explicitly allowed quote-form conversion.

- **Rust** — [finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs](../../finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs) · [finstack-quant/calibration/src/targets/hazard.rs](../../finstack-quant/calibration/src/targets/hazard.rs) · [finstack-quant/calibration/src/recalibration/hazard.rs](../../finstack-quant/calibration/src/recalibration/hazard.rs)
- **Python** — [finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs](../../finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs) · [finstack-quant-py/src/bindings/calibration/mod.rs](../../finstack-quant-py/src/bindings/calibration/mod.rs)
- **WASM** — [finstack-quant-wasm/src/api/core/market_hazard.rs](../../finstack-quant-wasm/src/api/core/market_hazard.rs) · [finstack-quant-wasm/src/api/calibration/mod.rs](../../finstack-quant-wasm/src/api/calibration/mod.rs)

**Acceptance.** Use a valid 5Y source and a 10Y risk quote with the same ID and internally consistent pillar dates. Both hosts’ market restore paths and Rust attachment must reject it. On a valid 5Y pair, a targeted bump must move the 5Y input and produce an accepted CDS reprice.
