# VALUATIONS-FX-AND-EXOTICS-FX-TOUCH: FX one-touch/no-touch probabilities and payout timing

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-TOUCH`
- Decision status: **Accepted 2026-11-14** — complete FX touch pricing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent first-passage, deterministic crossing and residual-claim calculations checked. Runtime, benchmark data, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX touch pricing and settlement

1. **Bind the contractual payout.** Require an explicitly ordered FX pair,
   up or down direction, a finite positive barrier in quote per base, explicit
   holder long or short, and a total nonnegative quote-currency Money payout.
   Apply side once and validate the contract before short-circuiting zero
   payout. A one-touch pays at the first hit or on an explicit fixed date at
   or after window close. A no-touch pays on that fixed date after confirmed
   survival. Live `AtHit` means immediate payment at first passage. A lag
   requires its own qualified payment law. Honor a known actual payment date
   on the fixed claim after a recognized hit.

2. **Resolve the authorized monitoring state.** Continuous monitoring includes
   the contractual opening and closing observations. Bind the exact benchmark,
   event phase, and information cut. Equality at an eligible observation is a
   hit. Require complete elapsed no-hit coverage or an immutable first
   recognized hit. A required missing observation needs authorized resolution.
   Admit live analytics only for a started continuous window that covers the
   entire remaining model horizon and has a qualified eligible origin and
   elapsed state. Future-start and discrete monitoring require a separately
   qualified model.
   [Barrier classification](../core-conventions/020-barrier-classification.md),
   [information cut](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [FX barrier monitoring](fx-barriers.md#accepted-decision-for-checked-fx-barrier-monitoring-and-settlement).

3. **Use the shared constant-coefficient first-passage law.** Prepare the exact
   monitoring coordinate and a compatible lognormal law. For a later-value FX
   quote, use the actual value-date bridge. A rolling value date can change
   barrier geometry and requires qualification for a constant barrier. Use one
   checked scalar volatility at the contractual barrier on the ordered source,
   forward and clock. Any flat proxy derived from terminal factors requires
   explicit selection. Map payout to `barrier_cash` with `HitAtHit`,
   `HitAtExpiry` or `NoHitAtExpiry`. `barrier_touch_probability` stays a
   diagnostic. Discount immediate cash at first passage. Discount deferred hit
   or survival cash on the actual payment date, using an equivalent
   deterministic expiry-to-payment factor on the same valuation basis when
   needed. Apply each factor once. Use the real closed form for a resolved
   nonnegative discriminant and the checked real integral for the negative
   domain. Evaluate the survival tail directly. Literal zero volatility uses
   exact deterministic crossing, including terminal equality. Preserve
   checked errors and exact zero without floors or clamps.
   [Continuous barrier cash](../models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash),
   [dated FX carry](fx-delivery-cip.md).

4. **Resolve entitlement and discharge independently.** An observed hit
   establishes entitlement under the contract. Retain fixed signed native
   quote cash on its actual due date through expiry until payment or discharge
   is established. Include eligible unpaid same-day cash at factor one.
   Clean close excludes discharged cash and requires resolved state. Partial,
   failed or overdue payment requires a qualified residual amount and valuation
   basis. Resolve state before querying markets that the claim no longer uses.
   Export a known payment as one dated `DerivativeSettlement` without principal
   or coupon flows. Contingent complete cash export is unavailable. Confirmed
   no-payoff and discharged states have `NoResidual`. Keep canonical Rust
   economics in the existing Python envelope and typed WASM routes.
   [Dated discounting](../core-conventions/016-discounting.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

5. **Reprice on the active basis with observed state held fixed.** Use the
   same price branch and actual selected quote or volatility source for each
   bump. Delta and gamma are per numerical unit and unit squared of the active
   FX quote. Vega is per `+0.01` annual model volatility. Use the full specified
   widths, with a full upward one-sided stencil only at an explicitly admitted
   smooth boundary. A shock across the live trigger or a deterministic
   terminal-hit discontinuity is a scenario. Report local flat-model quote
   and base rho per `+1 bp`, holding prepared level, volatility and other
   external factors fixed. Actual dated curve DV01 reprices all active roles
   of the bumped source. Date theta uses qualified observations and eligible
   signed cash over the exact roll, without synthesizing a hit. Fixed quote
   cash has zero FX, volatility and base sensitivity and retains its active
   discount and cash timing exposure.
   [Option sensitivities](../valuations-framework/option-sensitivities.md),
   [dated curve risk](../valuations-framework/finite-difference-rates.md).

## Independent calculations

- An up barrier of `1.29` on `1.15` with expiry time `0.75`, quote rate
  `−0.08`, base rate `−0.10645` and volatility `0.23` gives zero log drift
  and discriminant `−0.008464` in the oriented first-passage law. For a
  USD 125,000 payout the hit probability is `0.564108889419`. Direct
  first-passage integration gives a PV of USD `72,053.29461904` for
  at-hit payment and USD `74,873.92937702` for payment at expiry.
  Moving payment `0.125`
  years later on the same flat law gives hit and survival values of
  USD `75,626.42487750` and `58,437.09777927`.
- With spot `1.20`, up barrier `1.225`, expiry time `0.75`, quote rate
  `0.025`, base rate `−0.015` and zero volatility, deterministic crossing
  occurs at `0.515482180068` years. For USD 10,000, immediate and expiry
  payment are worth USD `9,871.956280` and `9,814.246877`.
- A January 7–12 window with up barrier `1.25` ignores the January 6
  observation of `1.26`. January 10 equality fixes the first hit, and a
  later `1.20` preserves that entitlement. USD 20,000 due January 14 is
  worth USD 19,940 at factor `0.997` and USD 20,000 on the eligible unpaid
  due day. A qualified USD 15,000 residual at factor `0.999` is worth
  USD 14,985.

References for the benchmark formula and payout terms are the
[Wystup FX formulas](https://www.mathfinance.com/wp-content/uploads/2024/08/Pricing-formulae-for-Foreign-Exchange-Options_eqf.pdf)
and [FpML touch payout terms](https://cdn.fpml.org/spec/fpml-5-13-5-lcwd-1/html/confirmation/schemaDocumentation/schemas/fpml-fx-5-13_xsd/complexTypes/FxOptionPayout.html).

## Observed behavior

### Choice, rationale and scope

The inspected implementation uses a constant-coefficient lognormal hitting-time formula with separate at-hit and at-expiry payout treatment.

### Algorithm and conventions

For an unbreached contract, define `μ=(r_d-r_f-σ²/2)/σ²`. Set `λ²=μ²+2r*/σ²`, with `r*=0` for expiry touch probabilities and `r*=r_d` for one-touch paid at hit. Let `z=ln(H/S)/(σ√t)+λσ√t`, `z′=ln(H/S)/(σ√t)-λσ√t` and `η=+1` down, `-1` up. The hit expression is `(S/H)^[-(μ+λ)]Φ(ηz)+(S/H)^[-(μ-λ)]Φ(ηz′)`. Multiply by payout. Expiry payment adds `D_d`. No-touch expiry value is `D_d A(1-P_hit)`.

Current spot beyond the inclusive barrier is immediately hit: at-hit pays now, at-expiry discounts the payout. An already recorded breach instead means an at-hit payment has already occurred and contributes zero residual. At expiry explicit observed state decides the remaining payout; after expiry zero. Zero volatility delegates deterministic crossing probability rather than dividing by volatility.

Spot delta/gamma use shared central differences against FX-matrix bumps; vega normally bumps the active quote or surface symmetrically, capping a positive quote-override half-bump at half its volatility. At an exact zero-volatility quote override it uses the one-sided upward difference `(PV(h)-PV(0))/(100 h)`. Domestic/foreign rhos use curve bump repricing in basis-point units. This normalization differs from the vanilla/digital analytic one-percent rho and must be interpreted via the metric contract.

### Selection and controls

The touch analytic kernel is the local selected method. Continuous monitoring must have started; future monitoring windows error. Barrier-level volatility is selected through the active surface/override route.

Require positive spot/barrier and a quote-currency payout; explicit monitoring start and observed state are mandatory once seasoned. NoTouch with AtHit payout is rejected. The negative-rate at-hit formula rejects `λ²<0` rather than silently taking a complex/invalid square root. Shared finite-difference settings control spot/volatility/rate bumps.

### Limitations, failure and fallback

The formula distinguishes a settled recorded hit from a newly hit payout and rejects the stated negative-rate domain. It is constant-volatility/constant-effective-rate; surface lookup at the barrier is a frozen-volatility choice. No independent hitting-time numerical benchmark was established for the full negative-rate domain.

### Alternatives and tradeoffs

Numerical first-passage/PDE valuation could extend the domain and allow time-dependent coefficients. Such a replacement is not implemented. MC would require explicit hit-time discount handling and convergence checks.

### Implementation evidence

- [finstack-quant/valuations/fx/src/fx_touch_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_touch_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/types.rs)

The original audit inspected inline assertions and did not execute the runtime suite. The independent calculations above cover the stated examples.

Canonical reference index. [Reiner Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991), [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
