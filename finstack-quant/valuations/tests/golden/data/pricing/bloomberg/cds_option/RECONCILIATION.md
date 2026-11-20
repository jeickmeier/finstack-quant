# CDX.NA.IG.46 payer option reconciliation

On the May 7, 2026 capture, CDSO shows **USD 118,781.76** for the USD 100 million payer. Finstack gives **USD 118,373.49**, or **USD 408.27 below Bloomberg**. The external USD 6 tolerance is still open. The captured spread volatility is 36.63%, the strike is 55.2848 bp, expiry is June 17 and exercise settles June 23. [CDSO capture](screenshots/cdx_ig_46_payer_atm_jun26__main.png)

## Rebuilding the market

The linked CDSW screen shows one traded spread of 53.6264 bp for June 20, 2031. The fixture previously copied it onto eight maturities and bootstrapped eight different hazard rates. Calibrating one constant hazard to the observed maturity raises NPV by USD 3.52 from USD 118,369.97. This implements the fixture's flat-index assumption. The archived forward matrix is set to *Ask* and *Knock Out Yes*, while CDSO specifies *CBBT Mid*. The exact curve used internally by CDSO remains unverified. [Linked CDSW](screenshots/cdx_ig_46_payer_atm_jun26__underlying_cds.png) · [CDSD matrix](screenshots/cdx_ig_46_payer_atm_jun26__swap_curve.png)

The linked spot CDS provides a separate check on the market before pricing the option. The amounts below are signed buyer cash at May 12 settlement. The Finstack calculation carries dirty PV to that date and uses the contractual 49-day ACT/360 accrual to recover clean principal. The former diagnostic used the Ask curve, reported zero accrued and compared a valuation-date PV to settlement cash.

| USD | Bloomberg CDSW | Finstack | QuantLib ISDA |
| --- | ---: | ---: | ---: |
| Clean principal | −2,137,736 | −2,137,841.36 | −2,137,733.65 |
| Accrued premium | −136,111 | −136,111.11 | −136,111.11 |
| Settlement cash | −2,273,847 | −2,273,952.47 | −2,273,844.76 |

QuantLib 1.43 independently converts the quoted spread to a flat hazard and prices the 100 bp coupon using its ISDA CDS engine. It uses Finstack's S531 discount-factor knots with log-linear interpolation, US settlement business days and inclusive final coupon accrual. Both engines round to the screen's maturity default probability of 0.0452. Using CDSW's clean upfront of −2.13773585 points as the sole hazard calibration input in Finstack gives an option value of USD 118,427.35. That experiment still leaves USD 354.41.

## Option diagnostics

- The production exercise integral agrees within one cent with independent integration split at the exercise threshold. Halving the calibration integration step also changes the expected underlying mark by less than one cent at this notional.
- Varying volatility over the rounding interval for 36.63% gives USD 118,356.66 to USD 118,390.33. The model needs 36.75127% to reproduce the screen premium.
- Substituting the displayed ATM forward of 55.2848 bp for the model's 55.02583 bp in the forward calibration, with the original strike conversion, raises NPV to USD 124,387.28. Changing the forward alone overshoots the target by USD 5,605.52.
- At the screen's 57.8824 bp breakeven, the difference between the clean exercise amounts for breakeven and strike, discounted from June 23 to May 7, is USD 118,764.38 in Finstack and USD 118,786.09 with QuantLib's spread conversion. This compares the *change* in exercise amounts. Breakeven is itself a Bloomberg output and cannot independently establish the option premium or either absolute exercise amount.

## Isolating the forward and strike with QuantLib

QuantLib's strike conversion moves the payer toward Bloomberg and then overshoots. Its forward moves the payer farther away. The table replaces each cash amount inside Finstack's option pricer while keeping the spread volatility at 36.63%, the exercise clock at 41 days, the same S531 discounting and the same function for valuing the delivered CDS at each possible expiry spread. All amounts are USD at May 7. The forward and strike columns use the protection buyer's sign, so a more negative strike makes exercise more valuable.

| Forward source | Strike source | Forward value | Strike cash | Payer premium | Change from Finstack |
| --- | --- | ---: | ---: | ---: | ---: |
| Finstack | Finstack | −2,019,252.35 | −2,008,683.02 | 118,373.49 | 0.00 |
| QuantLib | Finstack | −2,024,099.59 | −2,008,683.02 | 115,917.82 | −2,455.67 |
| Finstack | QuantLib | −2,019,252.35 | −2,010,527.26 | 119,229.74 | +856.25 |
| QuantLib | QuantLib | −2,024,099.59 | −2,010,527.26 | 116,762.58 | −1,610.91 |

Averaging each substitution's effect over both orders assigns **−2,461.42 to the forward** and **+850.51 to the strike conversion** in the combined premium change. Using both QuantLib cash amounts leaves the payer USD 2,019.18 below Bloomberg.

The reference forward carries the flat hazard fitted to the captured 53.6264 bp spot spread to expiry. It weights the June 23 clean settlement principal by survival to June 17 and adds the expected pre-expiry loss, with 40% recovery, payable at exercise settlement. For the strike, QuantLib independently fits the 55.2848 bp par spread at expiry and values the 100 bp coupon. Both amounts are discounted from June 23 to May 7. These are explicit assumptions about the delivered CDS and front-end protection. [QuantLib ISDA CDS engine](https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/credit/isdacdsengine.hpp)

On the Finstack side, payer minus receiver at the coupon recovers the forward. Parity at the contractual strike then recovers its cash amount. To substitute the reference forward through the public pricer, the test varies only the single credit quote on a copy of the market until coupon-strike parity reproduces that forward. For this loss-free, unit-factor, non-knockout index with fixed volatility, that curve enters the option price only through the forward used to fit the spread distribution. The strike substitution uses the equivalent clean-price strike. The test checks both the parity identities and the equivalence of the spread and cash strikes to one cent. [Revaluation test](../../../../../../../../scripts/tests/test_bloomberg_cdso_fixture.py)

Holding the other Finstack input fixed, a **USD 800.12 increase in discounted forward value** or a **USD 880.52 decrease in discounted strike cash** would each reproduce the captured USD 118,781.76 payer premium. The QuantLib strike adjustment is USD 1,844.24 downward, about twice that required move. Its forward adjustment is USD 4,847.24 downward, opposite the required forward-only move. The screen premium still permits combinations of the two and does not establish a unique Bloomberg attribution.

## What delta, vega and breakeven imply

Each row fits the USD 118,781.76 screen premium and one additional observation at 36.63% volatility. The two contribution columns allocate the USD 408.27 premium difference by averaging both orders of substitution in Finstack. The delta is the spread-strike Black delta at the original contractual strike. Vega is the production reprice at 37.63% less the price at 36.63%, with the distribution mean refitted to the same forward in each valuation. The screen shows **50.70%** and **USD 3,411.78** respectively. Components are rounded independently. [Archived CDSO screen](screenshots/cdx_ig_46_payer_atm_jun26__main.png) · [Revaluation test](../../../../../../../../scripts/tests/test_bloomberg_cdso_fixture.py)

| Second observation and assumption | Forward contribution | Strike contribution | Delta | Vega (USD per vol point) |
| --- | ---: | ---: | ---: | ---: |
| 57.8824 bp breakeven, Finstack CDS value there | +400.21 | +8.06 | 51.0242% | 3,367.11 |
| 57.8824 bp breakeven, QuantLib ISDA CDS value there | −444.94 | +853.20 | 50.8105% | 3,364.95 |
| 50.70% delta, Finstack spread-delta convention | −882.14 | +1,290.40 | 50.7000% | 3,363.84 |
| USD 3,411.78 vega, Finstack one-point bump | +17,909.88 | −17,501.62 | 55.4234% | 3,411.78 |

**Breakeven constrains the difference between two cash amounts.** If the quoted premium is its May 7 PV, discounted strike cash equals the discounted CDS value at 57.8824 bp less USD 118,781.76. The two CDS valuations at that spread differ by USD 1,822.52. The second row uses QuantLib only to supply that external breakeven mark and retains Finstack's pricing function to measure both contributions. The screen gives the spread to four decimal places, worth about ±USD 2.28 at this point. The premium date also matters at this precision. Treating the quoted amount as cash payable May 12 would reduce its May 7 value by USD 59.78. The benchmark and table use the captured market value as quoted.

**Delta and vega reject the same two-input explanation under Finstack's conventions.** Inverting Black N(d₁) at the contractual strike gives a forward spread of 54.988075 bp, or 54.987229–54.988921 bp allowing for delta display rounding. The screen's 55.2848 bp ATM forward would instead give 52.4473% delta. At the four corners of the delta rounding interval and the ±USD 6 premium band, vega is USD 3,363.78–3,363.89. Conversely the exact premium-and-vega fit misses delta by 4.72 percentage points and misses the discounted breakeven payoff by USD 37,937.18 using Finstack's CDS mark. The Greeks constrain which model conventions can work together. They do not establish an absolute Bloomberg forward or strike cash amount.

## Comparison with the QuantLib source

QuantLib 1.43's core `BlackCdsOptionEngine` puts the underlying CDS running coupon in Black's strike argument, uses the CDS fair spread as forward, and scales by the absolute coupon-leg PV divided by the running coupon. A non-knockout payer then receives discounted expected pre-expiry loss outside the option payoff. The executable comparison using the fixture's curve, dates and volatility requires a proxy underlying paying 55.2848 bp. The knockout proxy gives USD 91,605.59 and the non-knockout proxy gives USD 152,107.50. The entire USD 60,501.91 difference reconciles to the separate default claim. The CDX contract delivers a 100 bp coupon and uses 55.2848 bp to set the exercise cash. [QuantLib 1.43 option engine](https://raw.githubusercontent.com/lballabio/QuantLib/v1.43/ql/experimental/credit/blackcdsoptionengine.cpp)

The QuantExt index-specific Black engine in Open Source Risk Engine separates strike and running coupon, puts expected front-end protection in the adjusted forward, and converts the quoted strike through a strike-specific risky annuity. Its annuity code solves a flat hazard at the strike and includes the accrual rebate in the clean coupon leg. Finstack also separates the coupon and strike and includes expected loss in the index forward, but values each expiry spread using a scheduled clean annuity with hazard equal to spread divided by loss given default. These differences identify the expiry CDS valuation and forward annuity as comparable pieces of code. [QuantExt index option](https://raw.githubusercontent.com/OpenSourceRisk/Engine/master/QuantExt/qle/pricingengines/blackindexcdsoptionengine.cpp) · [QuantExt strike annuity](https://raw.githubusercontent.com/OpenSourceRisk/Engine/master/QuantExt/qle/pricingengines/forwardriskyannuitystrike.cpp) · [Finstack option engine](../../../../../../credit/src/cds_option/bloomberg_quadrature.rs)

**Changing only how the delivered CDS is valued at each expiry spread** raises the payer by **USD 103.65** to USD 118,477.15, leaving USD 304.61. The test fits an ISDA flat hazard to each spread, values the 100 bp CDS in QuantLib, and refits the lognormal mean to the original Finstack cash forward. Strike cash, discounting and volatility stay fixed. Vega rises only USD 2.88 to USD 3,369.22, still USD 42.56 below Bloomberg. Two quadrature orders agree within one cent for both volatility levels. This isolates the size of that model difference. It supplies no evidence for replacing the index payoff with core QuantLib's Black engine or fitting a production cash offset to the screen. [QuantLib ISDA legs](https://raw.githubusercontent.com/lballabio/QuantLib/v1.43/ql/pricingengines/credit/isdacdsengine.cpp)

## What would distinguish the remaining causes

For this loss-free index, payer minus receiver at the same strike gives the discounted forward index value less the discounted exercise amount. At the 100 bp running coupon the spread-strike exercise amount is zero. Bloomberg payer and receiver premiums there would isolate the forward. The receiver premium at the archived 55.2848 bp strike would then isolate the exercise amount at that strike. The same valuation snapshot, fixed spread volatility and CDSO curve settings are needed for the comparison. The displayed ATM spread alone does not establish the forward cash value. [White, sections 9.4.1–9.4.4](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf)

## Focused reproduction

```sh
mise run rust-test-filter -- finstack-quant-valuations cdx_ig_46 --integration valuations
NEXTEST_SUCCESS_OUTPUT=immediate mise run rust-test-filter -- finstack-quant-valuations cdx_ig_46_convention_matrix --integration valuations --slow
uv run --no-sync pytest -q -s scripts/tests/test_bloomberg_cdso_fixture.py
GOLDEN_IGNORE_NON_EXECUTABLE=1 GOLDEN_FIXTURE_FILTER=cdx_ig_46_payer_atm_jun26 mise run rust-test-filter -- finstack-quant-valuations golden_pricing_fixtures_from_existing_json_files --integration valuations --slow
```

The last command shows the raw external mismatches, including the NPV failure at USD 6.
