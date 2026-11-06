# VALUATIONS-BONDS-16: Par-CDS CS01 versus bond-spread fallback and spread duration

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and selection

[Accepted ADR 0011](0011-bond-cs01-without-hazard-curve.md) owns the no-hazard z-spread contract for supported bonds; [ADR 0010](../valuations-framework/0010-metric-identifiers.md) owns instrument-qualified attribution. This Draft records the actual model selection and repricing below. The option-bearing OAS branch remains ADR 0011's explicit scope-reconciliation item, not an approved exception. [Shared credit sensitivities](../valuations-framework/credit-sensitivities.md) owns the framework estimators; their stencils and bucket methods are distinct from this bond fallback.

When the active model is HazardRate or RatesCredit, parallel/bucketed CS01 delegates to framework par-CDS quote shocks and hazard recalibration. A present hazard curve must carry a replayable calibration recipe; hand-built hazard curves error before OAS quote pinning. A missing curve is left for the shared path to diagnose. Merely attaching a credit ID while selecting a noncredit model does not select hazard CS01.

Without a credit-consuming model, optional bonds use OAS risk and plain bonds use Z-spread risk. Both are forward one-bp differences, keyed by instrument ID (`cs01::BOND_ID`), rather than par-CDS sensitivities. Bucketed CS01 on this route returns the parallel fallback; it does not create a fictitious credit tenor curve.

## Algorithms and conventions

For fallback CS01, hold all model inputs fixed and evaluate `PV(s+0.0001)−PV(s)` through the selected model/registry. Plain Z-spread base and bumped values are converted from valuation PV to settlement dirty with the canonical carry bridge; optional OAS values remain valuation PV. Required solved Z must be finite. Optional OAS comes from computed OAS, or zero if no price driver; a price-driven bond without computed OAS errors. Base and bumped PVs and final difference must be finite.

Spread duration deliberately always uses the deterministic quote/workout Z kernel, even for hazard-priced bonds: `SD=−[P(z+h)−P(z)]/(P(z)h)`, `h=0.0001`. It is the quote-spread exposure used for attribution, not normalized canonical CS01. Near-zero denominator is rejected at `1e−12·max(abs(notional),1)`. The kernel's compounding is periodic, not continuous exponential spread discounting.

## Limitations, alternatives and evidence

The forward difference includes a second-order price term; its deviation from central one-bp P&L is order `h²` in absolute price and generally order `h` relative to the first-order sensitivity. The source comment claiming about `10^−8` **of CS01 magnitude** is not a general bound. Settlement versus valuation anchoring also differs between fallback variants. A symmetric fallback would be a different implemented convention and is not adopted here.

Owning code: [CS01 routing and fallback](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/cs01.rs), [spread duration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/spread_duration.rs), [risk view](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/risk_view.rs), [shared CS01](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs). CS01 inline tests inspect selected custom-registry repricing and early rejection of unreplayable hazards. This is dispatch/domain evidence; independent spread-bump convergence remains open. [Credit valuation reference](../../../REFERENCES.md#hull-white-2000-cds) supports model background, not approval of the bond fallback.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
