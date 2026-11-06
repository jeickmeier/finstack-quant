# PORTFOLIO-ATTRIBUTION-004: Modified Dietz, geometric linking and money-weighted return

- ID: PORTFOLIO-ATTRIBUTION-004
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and conventions

These helpers distinguish an approximate flow-adjusted period return, a geometric link of caller-supplied subperiod returns and an investor-perspective XIRR. They do not infer external cashflows from positions or choose valuation dates. Returns are decimals, not percentages; `horizon_years` is supplied in 365-day years.

## Algorithms and controls

**Modified Dietz.** Given beginning and ending market values `B,E`, contributions `C_j` and remaining-period fractions `a_j`, calculate

`R = (E-B-sum C_j)/(B+sum a_j*C_j)`.

Positive `C` is capital contributed to the portfolio. Validate finite values and `a_j` in `[0,1]`, require a finite strictly positive denominator and a finite result. Flows exactly at period start use fraction one; end flows use zero. The wrapper accepts the fractions directly, so date consistency is caller-owned. This approximates a time-weighted return when intraperiod valuation is unavailable.

**Linked return.** Validate finite horizon and finite subperiod returns strictly greater than `-1`. Multiply `G=product(1+r_j)` and require finite positive growth. Return cumulative `G-1`. Only when horizon is at least one year return `G^(1/horizon)-1` as annualized; for shorter, zero or negative finite horizons the annualized field equals cumulative and the flag is false. An empty return sequence produces zero. The algorithm uses direct multiplication, not log accumulation, and rejects underflow to zero or overflow.

**Money-weighted return.** Convert dated flows to the core XIRR input and call `xirr(...,Act365F,None)`. Contributions are **negative** here and withdrawals/terminal proceeds positive, opposite to Dietz. Core date netting, one-sign-change restriction, root bracketing and residual checks are dependencies described in [XIRR](../core-conventions/017-irr.md); this wrapper does not implement another root selection policy.

## Alternatives and evidence

Exact subperiod valuation and geometric linking avoid Dietz timing approximation but require more observations. Modified IRR and multiple-root selection are not provided by this XIRR wrapper. Log accumulation could extend linking's numerical range, but current direct-product failure semantics must be recorded accurately.

[Source and inline formula examples](../../../../finstack-quant/portfolio/src/performance.rs) include a named GIPS example, no-flow identity, annualization thresholds and XIRR roundtrip. A fixture name does not certify GIPS compliance; reference edition and independent recomputation were not verified here. No tests ran.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
