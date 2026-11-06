# MODELS-AND-VALUATIONS-CREDIT-11: Liquidity estimators, execution impact and liquidity VaR

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Roll/Amihud estimators, Kyle linear impact, Almgren–Chriss cost/linear-impact schedules and Bangia-style LVaR are separate explicit calculations. Profile-based impact coefficients are heuristic calibrations.

## Scope and input domain

Positive mid/bid/ask with bid≤mid≤ask, nonnegative volume/spread volatility; finite quantity and positive horizon/daily volatility/reference price. AC gamma≥0, eta>0 and exponent in (0,1].

## Algorithm

Roll spread=2√(−lag-one covariance), using the overall sample mean and divisor N−1; nonnegative covariance or invalid samples returns None. Amihud=mean(|return|/volume), failing on any nonpositive volume. Kyle lambda=Amihud×reference price; cost=λQ²/2 and trajectory is uniform.
AC permanent cost=γQ²/2, temporary cost=η|Q/T|^δ|Q|. Profile calibration uses gamma=spread/(2ADV), eta=dailyVol×mid/√ADV, delta=.5. Optimal trajectory requires delta=1: remaining inventory is Q sinh(κ(T−t))/sinh(κT), κ²=riskAversion×priceVol²/eta, default risk aversion 1e−6. Near-zero κT uses uniform execution; large sinh ratios use exponentials. Piecewise-linear inventory variance integrates sigma²×(a²+ab+b²)Δt/3. Reported execution risk for simple estimates is priceVol×|Q|√(T/3).
LVaR adds half expected plus normal-quantile stressed relative spread times absolute position value to loss magnitude. VaR inputs/outputs use negative-loss signs; zero base VaR yields NaN ratio. Days to liquidate=|Q|/(participation×ADV); zero quantity is zero, no capacity infinity, nonfinite inputs NaN. Tiers use strict thresholds [1,5,20,60] days; equality enters the next tier.

## Selection and controls

Caller selects model and thresholds. The public almgren_chriss_impact helper synthesizes bid/ask at .999/1.001 times mid (20bp full spread), uses unit mid when absent, scales ADV-derived gamma/eta by caller multipliers, and fixes the temporary-impact exponent at .5. Liquidity spread volatility defaults relative, observation window 20 days; absolute volatility divides by mid. AC nonlinear cost is supported but its closed-form trajectory rejects nonlinear exponent.

## Conventions

Quantity/ADV share units; prices are per unit, time in days, daily volatility decimal. LVaR confidence is (.5,1). Participation is not capped at one.

## Limitations, failure and fallback

Kyle uniform schedule is a selected schedule, not the risk-optimal instantaneous liquidation solution of pure permanent impact. Profile fields are public and can bypass construction validation. No volume/market-state evolution is modeled.

## Alternatives and tradeoffs

Nonlinear execution optimization and signed-order-flow Kyle regression are unimplemented; the Amihud conversion is a proxy. Alternative explicit impact coefficients are supported.

## Evidence

Owners: [Liquidity source files](../../../../finstack-quant/models/src/liquidity). Inline tests locate formula/domain/trajectory invariants. References: [Roll](../../../REFERENCES.md#roll-1984), [Amihud](../../../REFERENCES.md#amihud-2002), [Almgren–Chriss](../../../REFERENCES.md#almgren-chriss-2000), [Kyle](../../../REFERENCES.md#kyle-1985), [Bangia](../../../REFERENCES.md#bangia-1999-lvar).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
