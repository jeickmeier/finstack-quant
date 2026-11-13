# MODELS-AND-VALUATIONS-CREDIT-11: Liquidity estimators, execution impact and liquidity VaR

- Decision status: **Accepted — complete checked liquidity and execution estimates package, 2026-11-12**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, host and example reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent arithmetic examples evaluated. Production, host and empirical qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked liquidity and execution estimates

1. **Freeze the quote profile and distinguish quote mid from arrival price.**
   Require a nonempty, already-trimmed, case-sensitive instrument ID and
   finite `0<bid<=ask` in currency per traded unit. Derive the arithmetic
   quote mid from these quotes. A locked quote is valid and has zero spread.
   Report full absolute spread `ask−bid`, half absolute spread, and
   full relative spread `(ask−bid)/mid`. Use the mathematical midpoint
   in relative spread and fixed-mid SD conversion before rounding.
   Require finite nonnegative ADV in units per trading day and a positive explicit observation
   window in trading days for the supplied historical statistics. Retire
   unused average-trade-size state. Spread volatility is explicitly
   absent or a tagged finite nonnegative standard deviation of full
   spread in relative or absolute units. Normalize absolute SD by the
   quote mid if requested, identifying it as a fixed-reference conversion
   rather than the SD of historical relative spreads when mids vary.
   Remove the 20-day and relative-kind defaults and the unavailable-to-zero
   substitution. Freeze and validate the same source fields in construction
   and restoration; spread and mid are derived. Quotes, data selection and
   lookback alignment are caller-owned.

2. **Use a checked Roll relative full-spread estimate.** Take chronological
   contiguous simple decimal returns of positive transaction prices (each
   `r_i>−1`) at one caller-selected observation cadence. Never infer or
   annualize the clock, reorder observations or bridge gaps. For `N>=2`,
   use the common full-sample mean `r̄` and
   `C=Σ_{i=1}^{N−1}(r_i−r̄)(r_{i−1}−r̄)/(N−1)`. Return `2sqrt(−C)`
   in return units for negative covariance. A valid sample with fewer
   than two observations, zero covariance or positive covariance is
   unavailable. Validate the whole sample before deciding availability.
   Use scaled centered products and checked compensated accumulation;
   take the square root before undoing scale if needed. Unresolved
   arithmetic, including uncertainty about covariance sign due to
   numerical loss, is an error rather than unavailability. This is a
   selected finite-sample estimator under bid–ask bounce assumptions.
   One pair is calculable and weak evidence. It neither establishes a
   tradable spread nor implies invariance across observation cadences.
   [Roll](../../../REFERENCES.md#roll-1984).

3. **Separate Amihud turnover from the Kyle volume proxy.** Amihud uses
   aligned chronological daily simple decimal returns and actual daily
   cash turnover in one currency and scale. Admit finite returns
   `r_i>=−1` and finite strictly positive turnover. The arithmetic
   mean of daily `|r_i|/turnover_i` has inverse currency units. No
   ratio of totals, implicit turnover construction, row dropping,
   volume floor or arbitrary rescaling. A valid empty aligned sample
   is unavailable; a mismatch or malformed row errors. All zero returns
   give zero. Retain a separately named Kyle **volume proxy** from
   the same shape of daily returns and positive share or contract
   volumes: `λ_proxy=P_ref mean(|r_i|/V_i)`, with explicit
   `P_ref>0` in currency per the same traded unit as the volumes.
   Its units are currency per traded unit squared. The Amihud
   currency-turnover output cannot be used as its unit-volume input.
   This proxy uses unsigned volume; it is neither a signed-flow
   regression nor a market-depth calibration. Retain one proxy
   calculation and one direct constructor taking `λ`, with no
   duplicate conversion path. Use checked scaled ratio and mean
   arithmetic so a representable requested result survives avoidable
   intermediate overflow or underflow.
   [Amihud](../../../REFERENCES.md#amihud-2002),
   [Kyle](../../../REFERENCES.md#kyle-1985).

4. **Require explicit execution inputs and actual impact coefficients.**
   Use signed finite trade quantity `Q` (positive buys, negative sells),
   finite horizon `T>0` in trading days, finite nonnegative daily
   decimal return volatility `σ_r` on that clock, and explicit finite
   arrival price `P_ref>0` in currency per traded unit. The absolute
   arithmetic price volatility is `σ=P_ref σ_r` per square-root
   trading day. No required profile field or inferred arrival price.
   Select finite `λ>=0` for Kyle. Select finite `γ>=0`, `η>0`
   and `0<δ<=1` for Almgren–Chriss. `λ` and `γ` are currency
   per traded unit squared; `η` is (currency per unit) divided by
   (units per trading day) raised to `δ`. Remove the profile
   coefficient recipe and synthetic 20 bp spread helper, including
   its unit-price fallback and fixed square-root exponent. Calibration,
   monetary denomination, ADV limits, price selection and venue
   assumptions stay with callers. Quantities and prices must change
   units together when a caller redenominates a trade into lots.

5. **Report uniform model costs and timing risk in explicit units.**
   For `q=|Q|`, Kyle has permanent cost `λq²/2` and temporary cost
   zero. Almgren–Chriss has permanent cost `γq²/2` and temporary
   cost `η(q/T)^δ q`. Total cost adds both in the common currency;
   they are integrated amounts rather than price displacements.
   Report cost bps as `10000 total_cost/(q P_ref)` and timing risk
   as the currency standard deviation `σ q sqrt(T/3)`. A validated
   zero trade has zero amount costs and timing risk with unavailable
   bps. Zero volatility gives zero timing risk. Buy and sell costs
   agree in magnitude for otherwise equal inputs. Neither quoted
   spread, fees nor execution risk is included in expected impact
   cost. These are fixed-coefficient, zero-drift scenario estimates
   with no claim about fills or market reaction.

6. **Name and score the trajectory that is actually returned.** Kyle
   returns a uniform schedule with no risk aversion input. Require
   exact `δ=1` and an explicit finite `ρ>=0` in inverse currency
   for the Almgren–Chriss trajectory. Nonlinear `δ` is available
   only for the uniform cost estimate. With
   `κ²=ρσ²/η`, sample the continuous fixed-coefficient minimizer
   of expected cost plus `ρ` times variance:
   `x(t)=Q sinh(κ(T−t))/sinh(κT)`. Exact zero risk aversion or
   volatility gives the uniform line `Q(T−t)/T`. Use stable
   small-argument ratios and large-argument scaled exponentials;
   no tolerance for switching impact laws or flattening a nonzero
   risk case. Require an explicit positive bucket count. Use
   [CORE-NUMERICS-20](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)
   uniform knots in trading days with exact `0,T`, checked counts
   and allocations, and actual positive adjacent widths `h_j`.
   Return signed monotone inventory from `Q` to +0 and bucket
   trades as adjacent stored inventory differences. Equal rounded
   adjacent inventories can give zero bucket trades. Use scaling
   before rounding inventory and check all generated quantities.
   Score the returned piecewise-linear inventory. Permanent cost
   is unchanged, linear temporary cost is `ηΣ q_j²/h_j`, and
   variance is `σ²Σ h_j(a_j²+a_j b_j+b_j²)/3` for starting and
   ending inventory `a_j,b_j`. Kyle uses its permanent cost and
   the same variance rule, with no temporary cost. Use checked
   sums and actual bucket widths. The Almgren–Chriss result is a
   bucketed view of a continuous optimum with the bucketed schedule's
   own scores. It does not solve the original discrete execution
   problem or optimize those piecewise-linear scores on the grid.
   [Almgren and Chriss](../../../REFERENCES.md#almgren-chriss-2000).

7. **Apply an explicit spread stress to an existing VaR.** Use one
   scalar operation taking finite loss-negative base VaR `v<=0`,
   finite signed position value `V` and a finite nonnegative
   stressed **full relative** spread `s*`. The caller aligns
   the position, monetary unit, price basis, horizon and coverage
   of the scenario. The one-way spread cost is `C=s*|V|/2`
   and adjusted VaR is `v−C`. A zero position has zero spread
   cost. Return the checked ratio `(v−C)/v` for nonzero `v`
   using scaled arithmetic before rounding either displayed amount, and
   an unavailable ratio only at exact zero. Remove the epsilon,
   NaN sentinel, hardwired normal quantile and implicit calibration.
   A caller can supply an empirical stressed spread or explicitly
   build one from a mean, volatility and selected multiplier.
   Treat the result as a concurrent exogenous spread scenario,
   without an implied probability law, joint VaR quantile, market
   impact, horizon scaling or portfolio diversification. The base
   VaR comes from its own owner. [Bangia et al.](../../../REFERENCES.md#bangia-1999-lvar).

8. **Separate finite liquidation time from no capacity and from tiers.**
   Use signed finite `Q`, finite `ADV>=0` in the same units
   per trading day and finite participation `p∈[0,1]`. Validate
   all inputs first. Zero quantity returns +0 days. A nonzero
   quantity with exact zero ADV or participation returns explicit
   `NoCapacity`. Otherwise return checked positive finite
   `|Q|/(p ADV)` in fractional trading days, using scaling so an
   underflowed capacity product does not mean no capacity. Missing
   capacity data stays with the caller and differs from known zero.
   No infinity or NaN sentinel, calendar conversion or implied
   fill forecast. Classify finite nonnegative days using four
   required finite positive strictly increasing thresholds. Use
   successive half-open intervals starting at zero, with equality
   entering the next tier and values at or above the fourth
   threshold in Tier 5. No default cutoffs or classification of
   `NoCapacity` as a finite tier. These cutoffs are user policy.

9. **Use checked Rust state and carry the contracts through existing hosts.**
   Distinguish domain errors, unavailable estimates, no capacity and
   computation errors. Validate all selected inputs before zero and
   unavailable branches. Use checked scaled arithmetic for requested
   finite results and avoidable intermediate range problems. Preserve
   exact zeros and reject positive requested results lost to underflow
   unless a rounded stored trajectory coordinate is explicitly allowed.
   No silent infinity, NaN, repaired output or partial schedule.
   Use immutable validated source state with strict restore and
   read-only computed outputs. Remove independent restoration of
   unverified derived results. Python and WASM convert shapes and map
   Rust errors; carry explicit nullable fields and a tagged no-capacity
   result through declarations and schemas. Register retained profile
   state directly when removing it from trade parameters. Reconcile
   exports, parity, generated schemas, examples and the liquidity
   lesson and notebook. Portfolio execution decisions, contractual
   maturity, balance-sheet liquidity, valuation adjustments and
   regulatory classification retain their existing owners. Qualify
   market data and empirical calibration separately from the math.

## Observed behavior

### Choice and rationale

Roll/Amihud estimators, Kyle linear impact, Almgren–Chriss cost/linear-impact schedules and Bangia-style LVaR are separate explicit calculations. Profile-based impact coefficients are heuristic calibrations.

### Scope and input domain

Positive mid/bid/ask with bid≤mid≤ask, nonnegative volume/spread volatility; finite quantity and positive horizon/daily volatility/reference price. AC gamma≥0, eta>0 and exponent in (0,1].

### Algorithm

Roll spread=2√(−lag-one covariance), using the overall sample mean and divisor N−1; nonnegative covariance or invalid samples returns None. Amihud=mean(|return|/volume), failing on any nonpositive volume. Kyle lambda=Amihud×reference price; cost=λQ²/2 and trajectory is uniform.
AC permanent cost=γQ²/2, temporary cost=η|Q/T|^δ|Q|. Profile calibration uses gamma=spread/(2ADV), eta=dailyVol×mid/√ADV, delta=.5. The trajectory admits delta within 1e−12 of one: remaining inventory is Q sinh(κ(T−t))/sinh(κT), κ²=riskAversion×priceVol²/eta, default risk aversion 1e−6. Near-zero κT uses uniform execution; large sinh ratios use exponentials. Piecewise-linear inventory variance integrates sigma²×(a²+ab+b²)Δt/3. Reported execution risk for simple estimates is priceVol×|Q|√(T/3).
LVaR adds half expected plus normal-quantile stressed relative spread times absolute position value to loss magnitude. VaR inputs/outputs use negative-loss signs; base VaR with magnitude at most 1e−15 yields a NaN ratio. Days to liquidate=|Q|/(participation×ADV); zero quantity is zero after the finite and nonnegative-capacity checks; negative ADV or participation yields infinity before the zero-quantity branch; for a nonzero trade, a zero or underflowed capacity product also yields infinity. Nonfinite inputs give NaN. Tiers use strict thresholds [1,5,20,60] days; equality enters the next tier.

### Selection and controls

Caller selects model; tier thresholds default to [1,5,20,60]. The public almgren_chriss_impact helper synthesizes bid/ask at .999/1.001 times mid (20bp full spread), uses unit mid when absent, scales ADV-derived gamma/eta by caller multipliers, and fixes the temporary-impact exponent at .5. Liquidity spread volatility defaults relative, observation window 20 days; absolute volatility divides by mid. AC nonlinear cost is supported; its closed-form trajectory rejects exponents outside the near-one tolerance.

### Conventions

Quantity/ADV share units; prices are per unit, time in days, daily volatility decimal. LVaR confidence is (.5,1). Participation is not capped at one.

### Limitations, failure and fallback

Kyle uniform schedule is a selected schedule, not the risk-optimal instantaneous liquidation solution of pure permanent impact. Profile fields are public and can bypass construction validation. No volume/market-state evolution is modeled.

### Alternatives and tradeoffs

Nonlinear execution optimization and signed-order-flow Kyle regression are unimplemented; the Amihud conversion is a proxy. Alternative explicit impact coefficients are supported.

## Evidence

Source and located tests: [quote profiles and capacity](../../../../finstack-quant/models/src/liquidity/types.rs),
[estimators](../../../../finstack-quant/models/src/liquidity/estimators.rs),
[execution inputs and shared scores](../../../../finstack-quant/models/src/liquidity/impact.rs),
[Kyle](../../../../finstack-quant/models/src/liquidity/kyle.rs),
[Almgren–Chriss](../../../../finstack-quant/models/src/liquidity/almgren_chriss.rs),
[synthetic wrapper](../../../../finstack-quant/models/src/liquidity/mod.rs)
and [spread adjustment](../../../../finstack-quant/models/src/liquidity/lvar.rs).
Existing host paths include [Python](../../../../finstack-quant-py/src/bindings/models/liquidity.rs),
[WASM](../../../../finstack-quant-wasm/src/api/models/liquidity.rs),
[models schemas](../../../../finstack-quant/models/src/schema.rs),
[Python tests](../../../../finstack-quant-py/tests/test_models_liquidity.py),
[WASM tests](../../../../finstack-quant-wasm/tests/wasm_models_liquidity.rs),
the [liquidity lesson](../../../../docs-site/content/learn/4/4.4-portfolio-construction-and-liquidity.mdx)
and the [liquidity notebook](../../../../finstack-quant-py/examples/notebooks/05_portfolio/liquidity_risk.ipynb).
The lesson's lot example changes only volume units; the full cost model
requires changing the price and trade quantity units as well. The
notebook's synthetic-wrapper coefficients are multipliers, despite its
gamma and eta labels. These source checks did not execute product tests.

### Primary references and evidence limits

- [Roll (1984)](https://onlinelibrary.wiley.com/doi/10.1111/j.1540-6261.1984.tb03897.x)
  derives the negative-serial-covariance result under bid–ask bounce.
  This record specifies its own finite-sample centering and denominator.
- [Amihud (2002), equation (1)](https://www.cis.upenn.edu/~mkearns/finread/amihud.pdf#page=3)
  uses monetary volume. [Kyle (1985)](https://doi.org/10.2307/1913210)
  models the price response to signed order flow. Neither derives
  the unsigned unit-volume proxy as a signed-flow estimate.
- [Almgren and Chriss](https://doi.org/10.21314/JOR.2001.041)
  distinguish permanent impact, temporary impact and inventory risk.
  The selected schedule samples a continuous solution, so the reported
  bucket costs must be checked on their own geometry.
- [Bangia et al., section III.B](https://archive.nyu.edu/bitstream/2451/27135/3/wpa99062.pdf)
  stress relative spreads alongside adverse price changes. The paper
  motivates the half-spread add-on and an empirically selected stress,
  not a mandatory normal spread quantile.

### Independent review evidence

- Bid `u=2^-1074` and ask `2u` give an exact relative full spread
  of `2/3` even though standalone half spread `u/2` rounds to zero.
- Roll returns `[1/128,−1/128,2/128]` give common-mean lag covariance
  `−25/294912`. Alternating `±1/128` over four observations gives a
  relative full spread `1/64`. Returns `[−1e−200,1e−200]` give
  `2e−200` despite covariance underflow in unscaled binary64.
- Amihud returns `[.01,−.03]` and turnover `[10000,5000]` give
  `3.5e−6` inverse currency. The Kyle proxy with `[.01,−.02]`,
  volumes `[100,200]` and price `50` gives `.005`. Full
  redenomination into 100-share lots gives `50` in currency per lot
  squared. In both units the permanent cost of 1,000 shares is `2500`.
- At `Q=1000,T=4,P_ref=50,σ_r=.02,γ=.001,η=.02,δ=1,ρ=.005`,
  the continuous-derived two-bucket midpoint inventory is about
  `324.0271368`. Its piecewise-linear cost is about `6119.328971`
  and variance `1022676.205093`. Minimizing the two-bucket score
  directly instead gives `312.5` at the midpoint.
- A base VaR of `−1000`, signed position value `−100000` and
  stressed full spread `.02` give cost `1000`, adjusted VaR
  `−2000` and ratio `2`. Zero base VaR has an unavailable ratio.
- With quantity and ADV both `2^-1074` and participation `.5`,
  the exact liquidation time is two days, despite the unscaled
  capacity product rounding to zero. If two days is an explicit
  threshold, the next tier applies. Known zero capacity is distinct.
- These are independent formula and boundary examples. Implementation
  accuracy, host parity, empirical fit and scenario usefulness remain
  to be qualified.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
