# ATTRIBUTION-003: Period cash, gross carry and flat-yield partition

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

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
