# VALUATIONS-FX-AND-EXOTICS-TARN-TARGET: TARN target truncation, early redemption and HW1F term rates

- Record: `VALUATIONS-FX-AND-EXOTICS-TARN-TARGET`
- Decision status: **Accepted 2026-11-15** — complete target entitlement, redemption, lifecycle, dated Hull–White pricing, risk and export package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent Decimal cash, target boundary and correlated Gaussian early-redemption calculations checked. Production pricing, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for TARN target and redemption

1. **Book the target as cumulative coupon entitlement.** Require a
   noncallable single-currency note, positive Money face, explicit long
   or short and an exact nonnegative Decimal target `T` as a fraction
   of original face. For each coupon form the candidate fraction
   `q=max(fixed_rate-index_fixing,annual_floor)*coupon_accrual_fraction`.
   Use finite annual decimal rates, nonnegative floor and unit index
   gearing. Earlier earned coupon credits count even while unpaid.
   Principal and payment status do not enter the target ledger.
   [ICE inverse-floater TARN conventions](https://idd.ice.com/IRHelp/Content/FM/Inverse_Floater_TARN.htm).

2. **Truncate the coupon and trigger on equality.** Start with remaining
   fraction `R=T`. Credit `min(q,R)`. At `q>=R` set the remaining
   amount to zero and trigger. Otherwise subtract the credited fraction
   exactly, or fail if the result is unrepresentable. Apply the annual
   floor before truncation, so the truncated effective rate can fall
   below it. Admit Decimal inputs exactly and convert needed binary64
   model and day-count operands at the checked Decimal boundary.
   Compute the candidate with the accepted bounded Decimal transform
   rules and compare admitted fractions without tolerance. Reject
   unrepresentable inputs or unresolved arithmetic. Multiply credited
   fraction by face using accepted Money arithmetic and apply any
   elected monetary rounding only after the entitlement calculation.
   The cap is exact on the admitted coupon ledger.
   [Decimal and Money rules](../core-conventions/012-money-decimal-precision.md).

3. **Pay par once on the designated payment date.** The triggering fixing
   schedules the truncated coupon and par for its actual payment date,
   extinguishes later coupon slots and preserves all earlier earned
   claims. A target shortfall pays the ordinary last coupon and par
   at maturity with no target top-up. Zero target schedules par at the
   first contractual coupon payment with zero interest and requires
   no fixing or simulated work for that branch.
   [Issuer terms illustrating target-based redemption and a cumulative cap](https://www.rbinternational.com/content/dam/rbi/ho/investors/debt-investors-ratings/prospectus-issuances/eur-25-dip/2023/Securities%20note%2017072020.pdf.coredownload.pdf).

4. **Separate fixing, entitlement, payment and discharge.** Require
   ordered in-advance fixing slots, actual index periods, qualified
   coupon accrual intervals and strictly ordered payment dates. Preserve
   explicit phases on a co-dated node. For seasoned state use authorized
   history or a validated snapshot of the exact processed prefix,
   remaining target, any triggering slot and outstanding claims. Bind
   it to the contract, observation version, authority and information
   cut. Keep every retained claim at its own payment event, including
   when several fixed claims are pending at once. Preserve principal
   outstanding through its contractual retirement and settle cash
   separately. Eligible same-day unpaid claims use factor one.
   Overdue or partial cash requires a qualified residual and actual
   eligible payment date. A passed due date alone does not imply
   discharge.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [Dated discounting](../core-conventions/016-discounting.md).

5. **Use dated Hull–White projection and actual payment discounting.**
   Use the accepted multiplicative basis for the actual index period
   on expressly selected curve roles. Recover the signed simple rate
   with the index accrual fraction. Keep discount and projection
   native clocks, coupon day count and model ACT/365F distinct.
   Require coverage through each needed index end, even beyond the
   payment horizon. Resolve a complete finite scalar
   `kappa,sigma >= 0` pair or an explicitly selected qualified cap/floor
   fit. Use exact joint rate and log-bank stepping over the fixing and
   payment events. Discount each cashflow at its actual payment.
   Require explicit simulation controls and report actual work and
   95% sampling uncertainty. Fixed cash and the zero-volatility
   case use deterministic valuation and report zero simulated work.
   Live stochastic pricing uses Hull–White with no forward-only
   approximation.
   [Accepted term projection](../models-analytic/rate-clock-and-projection.md),
   [Accepted Hull–White simulation](../models-analytic/hw1f-monte-carlo-discounting.md).

6. **Rerun future target decisions for rate risk.** Report signed
   parallel and triangular bucketed DV01 in native currency per positive
   bp. Shock each distinct active curve once, freeze contractual terms,
   the selected scalar parameters and authorized history, and recompute
   future target decisions. Use matched simulation groups across
   scenarios and paired uncertainty. A deterministic bump that jumps
   the redemption date is a finite-step scenario response. Retire the
   inactive surface field. This profile has no local surface vega or
   target sensitivity metric.
   [Accepted rate risk](../valuations-framework/finite-difference-rates.md).

7. **Export resolved signed cash through canonical Rust state.** Export
   known interest as `Floating`, early par as `PrePayment` and maturity
   par as `Notional`. Record the actual retired face and its contractual
   date separately from settlement cash. A complete fixed schedule is
   unavailable while later cash is contingent. Restore and validate
   the same Rust terms and state in the existing Python and WASM JSON
   routes and generated schemas.
   [Cashflow semantics](../core-conventions/015-cashflow-semantics.md).

## Independent calculations

- With face USD 1 million, target 7.5%, fixed 10.5%, zero floor,
  coupon fractions 0.5 and fixings 0.5% and 2.5%, the candidate coupon
  fractions are 5% and 4%. Fixings 2026-01-02 and 2026-06-29 pay
  2026-08-03 and 2027-01-04 respectively. As of 2026-07-01 both
  coupons are unpaid and the target is already reached. The later
  coupon is capped at USD 25,000 and the earlier USD 50,000 survives.
  Principal is due with the capped coupon. Discount factors 0.98
  and 0.94 give PV USD 1,012,500. Exact equality also triggers,
  target zero gives first-payment par, and target shortfall at
  maturity leaves the last coupon unchanged.
- With flat 3% continuous discount and 4.5% continuous projection,
  `kappa=.08`, `sigma=.03`, face 100, fixed 8.5%, floor 0.5% and
  target 1.2%, use fixing/start/end/payment days 90/92/183/192 and
  181/183/274/285 on ACT/365F. Each index and coupon fraction is
  91/360. Payment-measure Gaussian integration gives PV contributions
  31.78064446 and 67.13788090, for total 98.91852536 per 100 face.
  Independent conditional-option and bivariate integration checks
  agree. The second reset precedes the first payment and paths
  exercise the early redemption branch.

## Observed implementation and scope

The TARN is an inverse-floating coupon note whose cumulative accrual, measured as a fraction of notional, is capped at a target. It redeems principal at the payment of the target-reaching coupon.

The following sections describe the audited implementation and qualification
limits. The accepted contract above governs reconciliation.

## Algorithm and conventions

For each accrual period compute `c=max(fixed_rate-term_rate,coupon_floor)×accrual_fraction`. With cumulative paid fraction `C` and target `T`, actual coupon is `min(c,max(T-C,0))`. When `c>=T-C`, set knocked-out and stop future coupons. Add actual coupon×notional to pending payment and redeem notional once at that coupon payment; otherwise redeem at final maturity. A zero target therefore triggers at the first coupon, not an immediate valuation-date redemption.

Term rates use the same HW affine bond-to-simple-forward calculation and deterministic additive projection basis as snowballs: `basis=market_forward(reset)-discount_model_forward(reset)`. The model process fits the discount curve and starts at its short rate. Reset events are at period starts; pending coupon/redemption is discounted when flushed at the next payment event using reciprocal simulated bank account. Maturity is appended to settle the final pending amount. If all remaining resets are deterministic, use contract DFs and report zero standard error.

Target state starts at zero on every path and is reset between paths. The pricer rejects seasoned valuation because cumulative coupon, past fixing and prior redemption state are absent. Contractual accrual day count differs from shared model time. Local risk registration is combined DV01 and triangular bucketed DV01; no target-risk or short-rate-volatility metric is added here.

## Selection and controls

Default/registered method is strict-parameter `MonteCarloHullWhite1F` via the shared note adapter. A deterministic branch is used only when no future stochastic reset remains; no independent discounting model is registered.

Target and floor finite/nonnegative; nonempty strictly increasing payment periods; index tenor must match the forward curve. Scalar HW parameters must be complete, positive and previously supplied/fitted; sigma schedules error. Shared MC count/antithetic/event-substep and seed controls apply. At or beyond final payment return zero.

## Limitations, failure and fallback

Target units, exact coupon truncation, first-coupon zero-target redemption and same-day settlement follow the observed contract above. Deterministic basis is not a multi-factor forward-rate model. Seasoned contracts are unsupported. Internal parity does not establish independent target-boundary, event-timing or stochastic-discounting accuracy.

## Alternatives and tradeoffs

A tree with cumulative-target state is a possible independent benchmark; production local pricing remains MC. Overshooting target, paying full final coupon, or postponing principal to final maturity are different contractual conventions and are not implemented alternatives.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/tarn/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/tarn/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

- [finstack-quant/valuations/exotics/tests/exotic_harness/tarn_tree_parity.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/tarn_tree_parity.rs): Tree-versus-MC comparison uses an unreachably high target and nonbinding floor, so validates the linear non-knockout term-rate/discounting limit, not target truncation or path-dependent knockout. It is an internal alternate-engine comparison.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). Production qualification remains pending.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
