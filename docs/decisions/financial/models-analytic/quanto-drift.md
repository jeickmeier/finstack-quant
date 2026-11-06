# MODELS-ANALYTIC-07: Quanto drift and domestic discounting

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Fixed-conversion European quanto calls/puts with asset/FX volatility, correlation and domestic/foreign continuous rates. Checked dispatch requires asset/FX vols≥0 and correlation in [−1,1].

## Choice and rationale

Apply the domestic-measure quanto drift correction −ρ σasset σFX to foreign asset carry and discount domestically. Source documents the drift sign; approval of FX quotation orientation and contractual mapping is not established here.

## Algorithm

Build adjusted forward Fq=S exp[(rf−q−ρσassetσFX)T]. Use Black call/put on Fq,K with asset σ and multiply exp(−rdT). At zero asset vol calculate discounted adjusted-forward intrinsic; at expiry spot intrinsic. Nonfinite results return NaN; finite prices floor at zero.

## Selection and controls

`quanto_option_price` validates and calls the raw `quanto_price`; the latter verifies only finiteness before branches. FX conversion multiplier is absent: the caller must apply any fixed contract conversion/notional.

## Conventions

Spot and strike are asset-price units; rd is payout-currency discount, rf is underlying-currency rate, σFX is relative annual FX volatility and ρ correlates asset return with the FX return orientation assumed by the correction.

## Limitations, failure and fallback

Constant rates, vols and correlation; no quanto smile, stochastic FX/asset covariance or stochastic discounting. Raw API accepts out-of-range correlation or negative volatility if finite. Contract orientation cannot be deduced from scalar arguments.

## Alternatives and tradeoffs

Joint asset/FX simulation can represent richer covariance and path features but is not this method. Zero correlation removes only the drift adjustment and retains the two-currency discount/carry split.

## Implementation and validation evidence

[quanto.rs](../../../../finstack-quant/models/src/closed_form/quanto.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). `test_quanto_zero_correlation_vs_vanilla`, parity and correlation-direction tests establish invariants/comparison to the same vanilla kernel. [Haug](../../../REFERENCES.md#haug-2007-option-formulas) is the available formula reference; no independently generated quanto fixture was located in this inspection.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
