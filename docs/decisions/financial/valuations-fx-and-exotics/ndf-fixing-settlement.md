# VALUATIONS-FX-AND-EXOTICS-NDF-FIXING-SETTLEMENT: NDF quote direction, fixing transition and settlement value

- Record: `VALUATIONS-FX-AND-EXOTICS-NDF-FIXING-SETTLEMENT`
- Decision status: **Accepted 2026-11-14** — complete NDF fixing and settlement package.
- Implementation status: **Observed** in the owning paths below. Reconciliation remains pending.
- Validation status: Independent reciprocal cash, date-basis, inverse-expectation and risk examples checked. Production fixing data, valuation, cash classification and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked NDF fixing and settlement

1. **Book a fixed strike and one net settlement.** Require
   positive base-currency Money notional `N`, explicit long
   or short side and positive finite booked strike `K`.
   Preserve the ordered quote and use `g` to express rates in
   settlement currency per base unit, using a checked
   reciprocal for the opposite orientation. For an observed
   fixing `R`, the signed cash is
   `side * N * (g(R) - g(K))`. Check arithmetic and apply
   agreed settlement rounding once when the cash becomes
   known. Use an at-market quote only to establish terms at
   booking. Remove the omitted strike and local rate floor.
   [BIS NDF definition](https://data.bis.org/help/glossary?item=Non-deliverable+forwards).

2. **Bind the fixing event, benchmark value date and payment
   date.** Use the actual booked dates or derive them once
   under explicit fixing and settlement calendars and rules.
   Same-day fixing and payment require ordered phases. Resolve
   the exact benchmark, quote orientation, observation date,
   publication time and eligible revision at the declared
   information cut. Project only while the fixing is expressly
   pending. Missing or invalid due observations fail unless
   the contract owner supplies an authorized disruption
   resolution. A later publication cannot enter an earlier
   cut.
   [EMTA BRL/USD template](https://www.emta.org/media/ancpombt/brl-ndf-template-rev-2021-final-draft.pdf) ·
   [Shared fixing rules](../core-market/scalar-series-and-fixings.md).

3. **Elect the pending projection explicitly.** Bind a matched
   NDF par outright to the benchmark, dates, payoff, settlement
   currency, collateral basis and information cut. An
   explicitly elected deterministic covered-interest-parity
   proxy is also admissible when the curves and benchmark
   reference value date are qualified. Apply the accepted
   spot-to-cash bridge and project to the benchmark reference
   value date. Discount settlement to the payment date.
   Disclose the proxy's NDF-basis, stochastic reciprocal and
   rounding limits. Normalize a true inverse par quote by
   reciprocal. For stochastic projection require the expected
   settlement-per-base fixing under the payment-date pricing
   measure. Selecting a source confers no automatic fallback.
   [Deliverable FX basis](fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity) ·
   [BIS on NDF market segmentation](https://www.bis.org/publications/non-deliverable-forwards-2013-and-beyond) ·
   [FX index observation dates](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/FxIndexObservation.html).

4. **Value the fixed claim through settlement.** Discount
   the one known cash amount on the qualified settlement
   curve. Ordinary clean close excludes same-day cash.
   An explicitly entitled cut includes known same-day cash
   at factor one. Failed or partial settlement requires a
   qualified residual claim. A fully settled trade has zero
   residual value without inactive market lookups.

5. **Use the active risk sources for each state.** Pending
   FX01 uses matched relative +1% and −1% moves on the
   selected source in settlement-per-base economics. Divide
   an inverse raw quote by the corresponding `1 + move`.
   Fix the booked strike, dates and eligible observed
   fixings. Preserve source, date and direction for matching
   attribution. A projection without a qualified shock
   response cannot supply FX01. Apply shared DV01 to the
   consumed curves. Once fixed, native FX and projection
   risk are zero. The live fixed claim has settlement-curve
   risk through its nonzero discount interval.
   [FX01](../valuations-framework/option-sensitivities.md#8-keep-fx01-a-fixed-directional-scenario) ·
   [DV01](../valuations-framework/finite-difference-rates.md).

6. **Export cash-only derivative settlement.** Publish the
   signed settlement cash with fixing and payment provenance.
   Use projected status while pending, fixed contractual
   when known and no residual after settlement.
   `DerivativeSettlement` is cash-capable and has no
   principal movement or coupon accrual. Use Rust validation
   in the existing Python instrument envelope and typed
   WASM route.
   [Core cashflow classification](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).

## Observed behavior

### Choice, rationale and scope

A long-base NDF is represented by its base-currency notional and settled in the convertible currency. Both base-per-settlement and settlement-per-base quote conventions map to the same economic exposure.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

Convert rates to settlement-per-base: `g(x)=1/x` for base-per-settlement quotes and `g(x)=x` otherwise. Settlement cash is `N[g(fixing)-g(contract)]`, discounted to maturity on the settlement curve. Missing contract rate gives zero at-market settlement after the fixing/projection requirements have been resolved.

Before fixing, use the selected spot quote direction and `F=S D_settlement/D_base` for base-per-settlement, reciprocal DF ratio for settlement-per-base. The implementation projects to **settlement maturity**, not the earlier fixing date. If a direct matrix query fails, the reverse quote is queried and inverted. After fixing date, missing observed fixing errors; on fixing date projection is still allowed. The settlement remains live on maturity and becomes zero afterward.

Trade construction rolls spot by the pair-specific FX calendar convention, adds tenor, then steps fixing backward over joint business days and applies Preceding. Quote FX01 bumps economic settlement-per-base exposure, so an inverse quoted rate is divided by `1+b` instead of multiplied; observed fixing is not bumped. Parallel/key-rate DV01 delegates to the shared registry.

### Selection and controls

The default discounting implementation prioritizes observed fixing, then (only before or on fixing date) a projected rate. Projection prioritizes quoted forward, then quoted spot/FX matrix plus both discount curves. Missing foreign curve is rejected unless quoted forward is present.

Notional must be positive and in base currency; currencies differ; fixing cannot follow maturity. Supplied rates must be finite and greater than the local minimum positive rate. The optional fixing source is checked against its typical currency. Settlement schedule carries one projected net cashflow and no residual after maturity.

### Limitations, failure and fallback

Pre-fixing projection uses settlement rather than fixing maturity. Inverse quotation makes substituting the reciprocal of a projected rate a different operation from modeling a stochastic reciprocal fixing; no explicit convexity adjustment is implemented. Observed fixing input may determine settlement even before its scheduled date.

### Alternatives and tradeoffs

Possible alternatives are fixing-date projection followed by settlement discounting and an explicit settlement-measure payoff model. Neither is a silent replacement authorized by this audit. Mandatory trade terms would avoid the missing-strike template convention.

### Evidence

- [finstack-quant/valuations/fx/src/ndf/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/ndf/metrics/tests.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/tests.rs)
- [finstack-quant/valuations/fx/src/ndf/types.rs](../../../../finstack-quant/valuations/fx/src/ndf/types.rs)

- [finstack-quant/valuations/fx/src/ndf/metrics/tests.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/tests.rs): FX01 quotation-direction and fixing-state regression assertions.
- [finstack-quant/valuations/fx/tests/ndf/test_ndf_types.rs](../../../../finstack-quant/valuations/fx/tests/ndf/test_ndf_types.rs): Fixing-source, date-construction and type/dependency assertions; not an independent fixing-measure benchmark.

References for the original audit are [Clark FX Options](../../../REFERENCES.md#clark-fx-options) and [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options).

## Independent numerical checks

- A long BRL 100,000 trade booked at 5 BRL per USD
  and fixed at 4 gives USD +5,000. The reciprocal
  terms .20 and .25 USD per BRL give the same cash.
  Discount factor .997 gives USD 4,985.
- With cash FX .20 settlement units per base, notional
  1,000 and strike .19, use base/settlement factor pairs
  `(.98,.99)` at fixing, `(.96,.98)` at the benchmark
  value date and `(.90,.95)` at payment. With cash
  discounted to payment in each case, projecting to
  benchmark value gives +5.622448979592, projecting to
  fixing gives +7.580808080808 and projecting to payment
  gives −.50.
- Equally likely inverse fixings 4 and 8 give expected
  normalized fixing `3/16`. The reciprocal of their
  mean is `1/6`. At strike .18, notional 1,000 and
  discount .95, the two calculations give +7.125 and
  `−38/3`.
- At CNY 700,000 notional, strike 7.25 CNY per USD
  and normalized fixing `1/7`, cash is USD `100000/29`.
  At discount .99 the pending-source relative FX01 is
  USD +990 in either quotation direction. Once fixed,
  FX01 is zero and centered settlement DV01 with a 1-bp
  step and .25 source years is USD −.085344827595.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
