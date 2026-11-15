# VALUATIONS-FX-AND-EXOTICS-ASSET-RANGE-ACCRUAL: Asset range-accrual digital-spread replication, GBM and quanto carry

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-RANGE-ACCRUAL`
- Decision status: **Accepted 2026-11-15** — complete asset range-accrual coupon, fixing, static and GBM pricing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent observation, static collar, quanto marginal, coupon and settlement calculations checked. Runtime, market data, numerical and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for asset range accrual pricing and settlement

1. **Book one conditional cash coupon.** Require explicit receive or pay,
   positive Money notional in payout currency, a finite nonnegative annual
   coupon rate and an exact nonempty observation schedule. Use the qualified
   day-count context from contractual start through the final scheduled
   observation. Book determination and actual payment separately and
   preserve ordered phases on a shared civil date. This coupon does not
   repay principal. Rate-index instruments retain their callable rate
   range-accrual owner.
   [Issuer coupon precedent](https://sp.morganstanley.com/download/prospectus/9d29aea9-679e-48ed-963b-3dbe6e52769b/).

2. **Freeze the contractual band and count exact slots.** Use finite
   ordered absolute bounds or derive them once from positive booked
   multiples of a qualified inception fixing. Preserve the resulting band
   through ordinary reprices. Both endpoints are inclusive. Bind each slot
   to its exact benchmark, phase, authorized version and publication cut.
   A certified tally identifies the exact eligible prefix and effective
   band. Missing eligible observations or unresolved authority errors.
   Project a pending observation only on a qualified positive model
   horizon. Count each slot once using the full scheduled denominator.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [range-accrual observations](../models-monte-carlo/22-range-accrual.md).

3. **Bind dated forwards and volatility to the currency basis.** Equity
   and index observations use qualified dated funding and explicit dividend
   treatment. An FX observation uses the ordered pair, actual reference
   value dates and quote-currency payout. For equity quanto, orient FX as
   payout currency per asset currency and apply the signed covariance
   correction to observation carry. Static replication can use qualified
   per-fixing ATM covariance inputs. Use one prepared quanto forward for
   all calls at that fixing and a smile qualified on its payout basis.
   GBM freezes the asset and FX volatility pair at the last live observation
   horizon. Preserve total variance when mapping quote clocks. The coupon
   is already denominated in payout currency.
   [Dated FX carry](fx-delivery-cip.md),
   [quanto pricing basis](quanto-drift.md).

4. **Use checked outer call spreads for static replication.** For positive
   bounds `L,U`, choose explicit relative bandwidth `0<b<1` and set
   `K_0=L(1−b)` and `K_3=U(1+b)`. Use undiscounted calls on the same
   fixing forward. Subtract the decreasing call slope magnitude on
   `[U,K_3]` from the one on `[K_0,L]`. The finite payoff is one in the
   closed band and tapers outside it. Require representable widths,
   supported quotes and sampled call bounds, monotonicity and convexity.
   Reject violations and unresolved arithmetic. Apply exact positive-asset
   support for nonpositive bounds, and exact inclusive membership for a
   qualified deterministic law. Sampled consistency and finite-band
   approximation accuracy require separate qualification.
   [Breeden and Litzenberger](https://origin.secure.website/wscfus/8149792/uploads/Breeden_1978_Litzenberger__J_Business_CCAPM_and_Options.pdf).

5. **Use exact nodes and dated carry for explicitly selected GBM.** Use
   one qualified constant asset volatility through the last pending
   observation. Preserve tied slots with their individual dated forward
   scales. Require explicit sampling controls and report actual work and
   uncertainty. Each complete path supplies one coupon observation, with
   the accepted pairing and replicate rules where applicable. The sum of
   same-law marginal in-band probabilities is an independent price check.
   Discount once to actual coupon payment.
   [Checked Monte Carlo range accrual](../models-monte-carlo/22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual).

6. **Value and export the actual coupon.** Apply holder side once to
   notional times annual coupon rate times accrual fraction times expected
   in-band fraction. Once determined, retain signed cash through discharge
   and avoid inactive market inputs. Eligible same-day unpaid cash uses
   factor one. Partial, failed or overdue payment needs a qualified
   residual. Export a known `Floating` conditional coupon with its
   effective rate, applicable accrual context and observation provenance.
   Complete contractual cash export is unavailable while contingent.
   Use the same Rust state in existing Python and WASM envelopes.
   [Cashflow classification](../core-conventions/015-cashflow-semantics.md),
   [dated discounting](../core-conventions/016-discounting.md).

7. **Keep risk on the selected route and booked band.** Hold authorized
   history, effective bounds and static bandwidth fixed. Pure spot Greeks
   hold sampled volatility fixed. Asset volatility shocks reprepare quanto
   covariance where it is active. Use actual represented bump widths and
   qualified paired draws for simulation risk. Deterministic membership
   jumps are scenarios. Retire the generic discount-only `rho`. Use dated
   DV01 across each actual curve's active roles. Horizon theta uses qualified
   closing observations and eligible signed cash.
   [Option risk](../valuations-framework/option-sensitivities.md),
   [dated risk](../valuations-framework/finite-difference-rates.md),
   [horizon theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- With forward `100`, time `0.5`, volatility `0.20` and band `[95,105]`,
  exact flat-law probability is `0.27604058955343`. Outer collar weights
  are `0.30226723114124` at width `0.01` and `0.28921165634651` at
  `0.005`. At width `0.01`, changing only the lower-limit volatility
  to `0.21` gives successive decreasing-call slope magnitudes
  `0.35717832`, `0.49927858` and `0.32604728`. The difference of the
  outer slopes is in `[0,1]`, but the sampled convexity check fails.
- On USD `1,000,000` at annual coupon `0.08` and accrual fraction `0.25`,
  observations `95`, `105`, `105.0001` and `100` against the booked
  band `[95,105]` give three hits and cash USD `15,000`. At the
  remaining payment factor `0.992`, PV is USD `14,880`.
- With spot `100`, asset volatility `0.22`, FX volatility `0.16`,
  correlation `0.55` versus payout currency per asset currency, yield
  `0.02`, live times `0.25,0.75`, funding factors `0.996,0.956` and
  the band `[100,120]`, the checked marginal framework gives quanto
  forwards `99.4184994373` and `101.5597760688`. On USD `1,000,000`,
  coupon `0.12`, accrual fraction `0.80`, one known hit in two known
  observations and payment factor `0.955`, GBM marginal PV is
  USD `40,038.29407668`.

## Observed behavior

### Choice, rationale and scope

The inspected standalone asset coupon uses static digital replication or
explicit GBM. The callable rate owner uses its own dynamics and principal.

### Algorithm and conventions

Validate sorted observations, positive notional/accrual interval, coupon≥0, lower<upper, paired historical counts and payment no earlier than last observation. Observations on/before valuation require exact count totals; future dates are strictly later. Payment date defaults to last observation. Value is zero on/after payment. Fully observed but unpaid value is `DF×N×coupon×year_fraction(start,last_obs)×in_range/total`.

For each future observation, compute forward `S exp(-q t)/DF_asset`; without quanto asset/payoff curve coincide. With quanto use `S exp[-(q+ρ σ_asset σ_fx)t]/DF_asset`, FX vol sampled at the currency CIP forward and payoff DF used only for the final coupon discount. Static replication queries volatility separately at each call-spread strike. Approximate `P(S>K)` by `[C(K_lo)-C(K_hi)]/(K_hi-K_lo)` using undiscounted Black calls, fixed absolute width 0.0025 (half-width 0.00125) and lower strike floor `1e-6`. Clamp each digital probability and their lower-minus-upper difference to [0,1]. Fully deterministic surfaces/overrides use inclusive forward-in-band membership instead. Sum future probabilities plus realized in-range count, divide by total counts, multiply coupon accrual/notional and payment DF.

GBM samples one volatility at payment horizon/current spot, creates drift nodes at observation/payment dates (including quanto carry), merges observation times into the grid, applies fixing multipliers, and computes the same historical-plus-future observation fraction. Generic spot/vol finite-difference Greeks reprice this method. Rho registers discount-only DV01, while DV01 registers combined curve risk; these are distinct registry settings.

### Selection and controls

Default `StaticReplication` uses dated smile call spreads; `MonteCarloGBM` uses a constant volatility plus datewise carry schedule. Complete rate-index fields may be representable in shared terms, but require the separate callable rate-linked instrument/model. Bounds are absolute or multiplied by the spot passed at valuation.

Shared GBM path defaults and instrument overrides apply. Static digital-spread width and strike floor are internal constants without a market-unit scaling control. `RelativeToInitialSpot` computes barriers from the current supplied valuation spot; there is no separately stored inception reference spot in these terms.

### Limitations, failure and fallback

The fixed 0.0025 spread is absolute price units despite its code comment saying 25bp. Probability clamping can conceal smile arbitrage. Relative bounds rebase with current spot and spot risk, rather than a locked inception reference. MC constant volatility and static smile replication are different distributions, especially across maturities/quanto terms.

### Alternatives and tradeoffs

An analytic digital at frozen volatility is cheaper but omits smile-slope effects. Density-consistent smile differentiation, adaptive spread widths or local-volatility paths could align price/risk more closely; none is implemented here. Rate range accruals require rate dynamics, not renaming the asset spot.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/exotics/range_accrual/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated cases. Production qualification is pending.

Canonical reference index. [Black 1976](../../../REFERENCES.md#black-1976), [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
