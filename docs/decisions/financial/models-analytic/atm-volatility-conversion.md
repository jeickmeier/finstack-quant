# MODELS-ANALYTIC-22: Price-equivalent ATM volatility convention conversion

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Positive finite volatility and finite forward, Normal/Lognormal/ShiftedLognormal conventions. Lognormal effective forwards must be positive; shifts finite.

## Choice and rationale

Price equivalence at K=F is solved with core Brent rather than applying σN≈FσLN as the answer. That approximation only initializes the solver.

## Algorithm

Validate vol, negative time, forward and shifts; exactly zero time returns input vol before convention-domain checks. Identical conventions (or shifts within1e−12) return unchanged. Price an ATM call under source model; solve target ATM call−source price=0 with σ floor1e−16. Seed scales vol by F or F+shift, using ratio for changing displacements; bad guess falls back to max(|vol|+1e−6,1e−6). Use default Brent settings and return finite positive solution; solver failure becomes VolatilityConversionFailed with residual at guess.

## Selection and controls

Explicit source/target convention; no market-data lookup or OTM conversion. Core Brent owns bracketing/tolerance controls (not caller-configurable here). Same-convention shortcut bypasses numerical pricing.

## Conventions

Normal annual volatility uses forward units/√year; lognormal is dimensionless/√year. Shared ATM price is undiscounted; common annuity would cancel. Shift unit matches forward.

## Limitations, failure and fallback

Time validation only checks time<0, not finiteness: NaN/infinite time can pass same-convention shortcut. Zero time returns unchanged even when cross-convention units differ, because price alone cannot identify vol. No final independent residual threshold is applied beyond the solver’s success contract.

## Alternatives and tradeoffs

First-order scale conversion is cheaper but not price-equivalent at larger variance; explicit Black/normal IV solvers exist elsewhere but this API chooses Brent.

## Implementation and validation evidence

[convert.rs](../../../../finstack-quant/models/src/volatility/convert.rs). Inline round-trip and representative-ATM-price tests compare the same pricing kernels, with negative-forward/insufficient-shift rejection. References: [Brent](../../../REFERENCES.md#brent-1973), [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
