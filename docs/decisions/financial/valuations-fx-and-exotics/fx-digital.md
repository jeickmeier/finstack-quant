# VALUATIONS-FX-AND-EXOTICS-FX-DIGITAL: FX cash-or-nothing and asset-or-nothing digitals

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-DIGITAL`
- Decision status: **Accepted 2026-11-14** — complete FX digital payout, lifecycle and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation remains pending.
- Validation status: Independent dated cash and base payout values, strict tie and retained claim examples checked. Runtime valuation, benchmark data, settlement and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX digital pricing and settlement

1. **Specify one native payout.** A cash-or-nothing digital
   pays one fixed positive Money amount in quote currency.
   An asset-or-nothing digital delivers one fixed positive
   Money amount in base currency. Require the ordered pair,
   positive strike, call or put, and long or short direction.
   Apply the position sign once. Preserve the native base
   amount as the contractual claim after fixing.

2. **Use the trigger and payout dates on their own bases.**
   Bind the expiry observation, benchmark value date `L`,
   actual market spot value date and payout date `T`.
   Use the accepted spot-to-cash bridge and compute Black
   `d1,d2` from the qualified benchmark forward `F_L`,
   strike and expiry variance. Under deterministic rates
   the unsigned quote PV of payout `A_Q` is
   `A_Q * D_Q(V,T) * Phi(phi * d2)`. A native base payout
   `A_B` has unsigned quote PV
   `A_B * X_V * D_B(V,T) * Phi(phi * d1)`. Use phi +1
   for a call and −1 for a put.
   [Rubinstein and Reiner](https://digicoll.lib.berkeley.edu/record/86304/files/b120984374_C044481811.pdf) ·
   [Deliverable FX basis](fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity).

3. **Use the strict payoff at the fixing and deterministic
   boundary.** A call pays above strike and a put below
   strike. Equality pays zero. Resolve the exact authorized
   fixing at the information cut. A due missing fixing
   requires authorized disruption resolution. At exact live
   zero volatility, compare the deterministic forward
   `F_L` with strike on the same strict basis. An exact
   deterministic tie gives zero. The positive-volatility
   ATM limit is half the discounted payout. At the jump,
   trigger-sensitive Greeks are unavailable.

4. **Retain the fixed native claim through payout.** Use
   the eligible fixing once and retain the signed native
   amount until discharge. A quote claim uses quote
   discounting. A base claim uses the base curve and
   current qualified cash FX for its quote PV. The trigger
   stays fixed. Explicitly entitled same-day payment has
   factor one. Ordinary clean close excludes same-day cash.
   Failed or partial settlement requires a qualified
   residual claim.

5. **Differentiate the dated price on the selected risk
   basis.** Use analytic delta and gamma per unit and
   squared unit of the active spot quote with volatility
   held fixed. Model vega is per volatility point.
   Local base and quote rhos are per positive basis
   point on their own curve clocks with the actual spot,
   benchmark and payment dates. At exact zero volatility
   away from strike, use the sensitivities of the fixed
   local payout branch. A finite move across the trigger
   is scenario P&L. Use dated horizon theta with eligible
   cash. For market reprices keep the selected scalar
   volatility or absolute-strike grid fixed, or rebuild
   a selected native smile from held quotes on the
   shocked forward. Call-spread replication and
   smile-density pricing retain their own model choices.
   [Shared option risk](../valuations-framework/option-sensitivities.md) ·
   [Dated theta](../valuations-framework/carry-theta-breakeven.md).

6. **Export the known payment as `DerivativeSettlement`.**
   Preserve its native currency, payment date and fixing
   provenance. The payment has no principal or coupon
   metadata. A live contingent payout's complete
   contractual export is unavailable. A confirmed no-payoff
   or discharged state has no residual. Rust owns
   validation through the existing Python envelope and
   typed WASM route.
   [Core cashflow classification](../core-conventions/015-cashflow-semantics.md).

## Observed behavior

### Choice, rationale and scope

European FX digitals use analytical lognormal distribution probabilities and separately derived sensitivities. Payout currency and base notional have different roles for cash and asset digitals.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

Using Garman–Kohlhagen `d1,d2`, a cash call pays `A D_d Φ(d2)` and an asset call `N S D_f Φ(d1)`; puts use negative arguments. On expiry only strict in-the-money conditions pay: equality pays zero. After expiry value and Greeks are zero.

Cash delta is `±A D_d n(d2)/(S σ√t)`, gamma `∓A D_d n(d2)d1/(S²σ²t)`, and vega `∓A D_d n(d2)d1/(100σ)`. Asset delta differentiates `SΦ(d1)`, gamma uses `d2`, and vega is `∓N S D_f n(d1)d2/(100σ)`. Cash theta differentiates `d2` in remaining time; nondegenerate asset theta differentiates `d1`. Domestic/foreign rhos differentiate both discounting and distribution drift and divide by 100.

At zero volatility away from the deterministic boundary, cash Greeks reduce to discount-only theta/rho and asset Greeks to discounted-asset delta/foreign rho. At deterministic ATM the Greek function errors as undefined. The deterministic asset theta branch uses a one-day shorter remaining maturity, floored at zero, and returns the price difference.

### Selection and controls

Default digital pricing delegates shared FX inputs to the local analytic kernel. Greek registration chooses the local `OptionGreeksProvider` for delta/gamma/vega/theta/domestic and foreign rho.

Inputs share date-based DF recovery and active surface/override selection with vanilla FX. Cash payout is a quote-currency money amount; asset payout converts the base notional at terminal spot. Time/day-count and theta-year denominator are explicit controls.

### Limitations, failure and fallback

The expiry equality rule and the zero-volatility distribution limit are discontinuous conventions. A smooth finite bump through a digital jump is not an independent derivative. No smile-gradient term is added to the analytic Greeks.

### Alternatives and tradeoffs

Tight call-spread replication would introduce a finite width and potentially smile-sensitive digital price. It is not the chosen kernel here. Full smile-density or local-volatility methods are not implemented locally.

### Evidence

- [finstack-quant/valuations/fx/src/fx_digital_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_digital_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_digital_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

References for the original audit are [Garman and Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983) and [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas).

## Independent numerical checks

- With spot 1.20 USD per EUR and quote/base factor pairs
  `(.98,.99)` at market spot, `(.94,.96)` at the
  benchmark date and `(.93,.95)` at payment, the
  benchmark forward is `3136/2585`. With strike 1.21,
  expiry time .5 and volatility .20, the USD 10,000
  cash call and put are USD 4,456.009472 and 4,843.990528.
  The EUR 10,000 base call and put are USD 6,043.073092
  and 5,241.775393. Each sum matches the corresponding
  discounted native payout on the continuous model.
- At exact deterministic equality both calls and puts
  have zero payout. After an eligible 1.22 fixing against
  strike 1.21, the asset call retains the EUR 10,000
  claim. With cash FX 1.18 and base discount factor
  .999, that claim is worth USD 11,788.20.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
