# MODELS-ANALYTIC-21: Discrete variance replication, log-contract anchor and wing extension

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Annualized forward variance from≥3 strictly increasing positive strikes straddling positive forward, positive T and supplied vol/discounted-option callbacks.

## Choice and rationale

Carr–Madan/Demeterfi-labelled OTM replication uses trapezoids, an exact log-contract anchor and finite flat-vol wing extension. No automatic switch to ATM variance occurs on failure.

## Algorithm

Choose K0=largest strike≤F. Use puts below K0,calls above,mean(call,put) at K0. Sum ΔK Q(K)/K² with half neighboring widths, including half-width endpoints, using Neumaier accumulation. Query vols at nodes; require>1e−6. Extend lower tail to Kmin/3 using boundary put vol and upper to3Kmax using boundary call vol, each64 trapezoid subdivisions. Return (2 exp(rT)/T)integral−(2/T) × (F/K0−1−ln(F/K0)) if finite positive. Warn if either wing log-moneyness<.35.

## Selection and controls

Fixed wing factor3 and64 subdivisions; user controls input grid and callbacks but not those constants. Invalid grid/vol, nonpositive/invalid variance yields None. Forward/K0 must exceed1e−9. Invalid tail endpoint arithmetic returns zero tail contribution rather than a distinct error.

## Conventions

Callbacks must use compatible forward, discount, option type and annual volatility. The exp(rT) factor presumes prices already discounted at supplied continuous r. Returned quantity is variance, not volatility.

## Limitations, failure and fallback

Finite flat-vol wings omit farther tails and assume no wing slope; narrow-grid warning does not reject. Callback option prices are not individually checked for sign/upper bounds; only final aggregate is tested. The mean-at-K0/discrete anchor is an approximation to continuum replication and needs grid convergence.

## Alternatives and tradeoffs

ATM σ², adaptive continuum integration or extrapolated smile tails are alternative methods, not fallbacks here. Fixed tail extension buys limited truncation correction at bounded cost.

## Implementation and validation evidence

[variance_replication.rs](../../../../finstack-quant/models/src/volatility/variance_replication.rs). Exact-anchor test is a direct formula identity; dense-grid flat-vol and wing-bias tests reuse production BS prices and test numerical integration consistency, not real-market replication. Reference: [Demeterfi](../../../REFERENCES.md#demeterfi-1999-volatility-swaps).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
