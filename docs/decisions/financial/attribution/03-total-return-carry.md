# ATTRIBUTION-003: Period cash, gross carry and flat-yield partition

- Decision status: Accepted 2026-11-16 for checked period cash, gross carry and funding.
- Implementation status: Observed for the source scope listed below. Runtime, cash lineage, pricing and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked the four cash-phase combinations, fixed-yield partition, cash realization differences, signed funding and closing-equivalent FX. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked period cash, gross carry and funding

1. Measure carry on held dated opening sources at the closing cut. Keep the same instrument, holder entitlement, selected model and valuation basis. Use one qualified held-market scenario for its remaining claim and period cash. Label authorized projected fixings as hypothetical. Retain detached unpaid claims in the mark or a qualified bridge. An admitted same-date market comparison on the same settlement basis has zero elapsed carry and period cash. [Accepted horizon valuation](../valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).
2. Count signed economic settlement cash once. Use `[open, close)` when both marks include eligible same-day cash, and `(open, close]` when both exclude it. Resolve mixed phases independently. Preserve payment date, event identity, entitlement and native currency. Distinguish income from other economic cash, including applicable principal and derivative settlement. Exclude an opening acquisition by verified event identity. PIK and write-downs are noncash. Convert foreign cash rows at qualified payment-date FX to the mark currency. Further reporting translation uses the closing-equivalent cash basis under [record 013](13-target-currency-and-scaling.md). [Accepted cashflow semantics](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).
3. On a common holder-currency basis compute gross carry as `V_H − V0 + C_held` and headline P&L as `V1 − V0 + C_close`. Use eligible observed or explicitly elected scenario cash for the closing path. If the qualified ledgers differ, expose `C_close − C_held` and require the selected method to attribute it once to a qualified effect. Fail complete decomposition when the difference is unsupported. [Scenario headline contract](../scenarios/15-horizon-attribution-and-returns.md#accepted-decision-for-checked-scenario-horizon-attribution-and-returns).
4. Elect unpartitioned carry or a qualified fixed-yield split. Require the product's opening yield law and buyer-to-holder bridge to reproduce the opening holder mark for a split. Hold that yield fixed to obtain the closing benchmark mark `V_Y`. With settled income `I` and other held cash `O = C_held − I`, compute `pull = V_Y − V0 + O` and `roll = V_H − V_Y`, and verify `I + pull + roll = gross carry`. Keep accrued change as a distinct diagnostic. Retire the first-curve synthetic grid and generic compounding assumptions. Unpartitioned carry leaves pull and roll unavailable. A failed requested split reports its cause. [Accepted bond carry](../valuations-bonds/19-carry-and-life.md#accepted-decision-for-checked-bond-carry-and-repayment-life).
5. Elect unfunded carry or require a signed funded amount `B` and a qualified positive same-currency dated funding factor `D_f`. Compute `F = B × (1/D_f − 1)` and net carry as gross carry less `F`. Unfunded carry has zero charge. Full opening-PV funding requires an explicit election. Retain signed funding credits and count contractual financing and any modeled overlay once on their qualified bases. [Accepted funding basis](../valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).
6. MetricsBased admits qualified gross carry or recovers it from net CarryTotal and the matching verified funding charge once. Match dates, cuts, holder basis and cash evidence. Retire the one-day stamp exception, generic Theta fallback and missing-to-zero behavior.
7. Publish checked Money, held and closing cash evidence, valuation cuts, gross carry, elected funding and net carry, and partition availability through the shared Rust result and source-backed restoration path. Native, Python and WASM consumers share that contract. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

An independent fixture reconciled USD `2` of gross carry to USD `3` of income, USD `1` of pull-to-par and USD `−2` of roll-down. It produced USD `1` of net carry after a USD `1` funding charge.

## Observed choice and scope

Repricing methods define theta as opening-market PV at the closing date minus opening PV, using opening model parameters. Carry includes period economic cash; headline total P&L also adds that cash to MTM. Funding is a diagnostic deduction for presentation and is not subtracted from gross total return. This applies through the instrument cashflow/metric contracts; absence of a YTM/accrual capability limits the partition, not headline carry.

## Algorithm

1. Delegate period cash selection to valuations `collect_period_cash`; retain total cash and income-only cash separately. Instruments including valuation-date cash receive payments in `[T0,T1)`; instruments excluding settled same-day cash receive payments in `(T0,T1]`. Eligible receipts are converted at payment-date FX in the supplied market snapshot. PIK, write-downs, collateral transfers and an instrument's identified opening notional draw are excluded. Query opening Accrued and YTM together; if that request fails, recover opening Accrued independently. Query closing Accrued using the same frozen market. Discard nonfinite optional metric values.
2. Set `delta_accrued = accrued1 - accrued0` only when both exist. If YTM is available, replace the first declared discount curve with a flat-yield curve and price both dates. Bonds use Street compounding and actual coupon frequency; other instruments use annual periodic compounding. Preserve original curve base date/day count and use log-linear DF interpolation. The knot horizon is `max(1, maturity-minus-base days)/365.25 + 1`, clamped to 1–100 years, or 100 years without expiry. Round up to half-year sampling steps and append a 100-year endpoint when needed; this grid sizing does not use the preserved curve day count.
3. Let `theta` be frozen-market price change, `C` total period cash, `I` income cash, `A` accrued change and `F` flat-yield price change. Set carry=`theta+C`; headline=`MTM+C`; coupon income=`I+A` when A exists, otherwise I. Principal cash=`C-I`.
4. When F exists, pull-to-par=`F+(C-I)-A`, substituting zero for missing A, and roll-down=`theta-F`. Without F, omit pull-to-par and assign carry minus coupon income to roll-down; issue a note that the price-carry residual is unpartitioned.
5. If a repo curve is declared, funding cost=`PV0*(1/DF(T0,T1)-1)`. Same-day cost is zero. Missing curve/PV, nonpositive or nonfinite DF and nonfinite cost omit funding with notes. No repo curve means no funding detail.

## Conventions and failure

Positive cash is instrument holder income according to the upstream cashflow contract; principal and coupon are deliberately distinct. The flat-yield partition is an approximation tied to the first discount curve, not a separate full risk-free/credit decomposition. Optional accrual/flat-yield queries can fail softly; period cash errors propagate. Funding uses the signed PV and frozen market. Attempted helper metric/price calls increment diagnostic repricing counts.

## Rationale and alternatives

The identities keep total-return cash separate from price MTM and avoid treating redemption as coupon income. This algebra is observed; adopting the first-curve flat-yield convention and generic annual compounding is pending. Alternatives include a product-specific carry decomposition or financing-net return; neither is selected universally.

Located tests cover unavailable-YTM/accrual recovery, principal/coupon partition, coupon-frequency compounding and repo funding. These are implementation/identity tests, not an independent economic carry benchmark. The upstream cash-selection kernel is documented in the existing valuation-framework catalogue.

## Source and evidence

- [helpers.rs](../../../../finstack-quant/attribution/src/helpers.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
