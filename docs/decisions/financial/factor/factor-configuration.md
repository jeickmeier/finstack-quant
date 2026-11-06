# FACTOR-001: Factor definitions, units, and configuration validation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Represent the factor universe as ordered typed definitions, an aligned covariance matrix, declarative matching, and a selected risk measure. The observed rationale is a single configuration consumed by portfolio sensitivity engines; the numerical bump execution belongs to that downstream scope.

## Input domain and conventions

A definition associates a factor ID/type with parallel curves, tenor-weighted curve buckets, equity tickers, an FX pair, or volatility surfaces. Rate/credit/inflation/custom bump defaults are in bp; equity/commodity/FX defaults are percent; volatility defaults are vol points (1 = 0.01 absolute vol). Custom types inherit the rates default. Covariance must be expressed in the same factor coordinates and horizon as sensitivities. This crate does not attach a reporting currency or automatically reconcile covariance units.

## Algorithm stages

1. Resolve a per-factor bump override before the type default; every default magnitude is 1.
2. Validate exact ordered equality of definition IDs and covariance IDs. Covariance construction separately rejects duplicate IDs.
3. Validate confidence for VaR/ES: finite and strictly between 0.5 and 1. Variance is the default measure.
4. Enumerate every declaratively emitted factor ID and reject an undeclared ID; reject duplicate credit issuer rows both inside one matcher and across cascade members.
5. A strict envelope loader decodes the exact `finstack_quant.factor_model_config/1` marker, then invokes configuration validation; ordinary configuration deserialization does not run the whole validation.

## Selection and controls

`unmatched_policy=None` selects Strict at the portfolio boundary; Warn is explicit. `bump_config=None` selects type defaults. Volatility uses the shared `Percent` bump tag even though its configured magnitude is absolute vol points; consumers must preserve the described distinction. Inflation and base-correlation dependencies are not automatically assigned to Discount/Forward/Credit: `matches_dependency_type` has explicit branches. A credit index matches Credit but not the Hazard curve-type filter; a credit curve matches both.

## Limitations, failures, and fallbacks

Configuration validation does not validate bump magnitudes, unknown override IDs, compatibility between factor type and mapping, or tenor-weight shapes. Raw in-memory configs and `FactorModelConfigEnvelope::new` can therefore carry values beyond this validator. This is a validation-scope fact, not proof that every downstream engine accepts them.

## Alternatives and tradeoffs

Typed validated mapping constructors could reject invalid combinations earlier; a declarative configuration with consumer checks is more flexible but requires a precise boundary. A universal normalized bump unit would reduce ambiguity but would change all sensitivity/covariance contracts. No new representation is adopted here.

## Implementation and evidence

- [src/config.rs](../../../../finstack-quant/factor/src/config.rs) — inspected implementation.
- [src/primitives/definition.rs](../../../../finstack-quant/factor/src/primitives/definition.rs) — inspected implementation.
- [src/primitives/factor_types.rs](../../../../finstack-quant/factor/src/primitives/factor_types.rs) — inspected implementation.
- [src/primitives/dependency.rs](../../../../finstack-quant/factor/src/primitives/dependency.rs) — inspected implementation.
- [src/envelope.rs](../../../../finstack-quant/factor/src/envelope.rs) — inspected implementation.

[tests/multi_asset_config.rs](../../../../finstack-quant/factor/tests/multi_asset_config.rs) locates multi-asset, exact-axis and strict-envelope checks; [src/config.rs](../../../../finstack-quant/factor/src/config.rs) contains matcher-universe and confidence tests. These establish intended contracts, not economic correctness of a bumped price. Source mappings are the evidence for units.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
