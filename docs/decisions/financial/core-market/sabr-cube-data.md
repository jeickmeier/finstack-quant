# CORE-MARKET-16: SABR parameter grids and materialized expiry slices

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Core separates validated SABR node data from model expansion/calibration and from materialized volatility slices. This preserves the quote/model input contract while allowing evaluators to choose their interpolation policy. A `VolCube` construction is not evidence that the supplied parameters reproduce market quotes or are arbitrage-free.

## Algorithm, domain, and controls

`SabrParameterData::new(alpha,beta,nu,rho,shift)` requires finite alpha>0, beta in [0,1], nu>=0, rho strictly between -1 and 1, and optional finite shift. Zero vol-of-vol is deliberately allowed. A finite negative shift is allowed. Note the constructor's order is **nu before rho**, while the stored fields list rho before nu; callers must respect the named signature. Deserialization uses the validating constructor.

`VolCube::from_grid` requires nonempty finite strictly increasing expiry and tenor axes, both strictly positive. Parameters and forwards must each have `expiry_count*tenor_count` entries in expiry-major order. Forwards need only be finite; positivity of forward+shift is not checked in this storage layer. Node access checks both indices and computes `expiry_index*tenor_count+tenor_index`. Default interpolation metadata is Vol; TotalVariance can be set explicitly and survives serialization. No interpolation, Hagan expansion, root solver or fitting procedure is executed here.

`VolCubeExpirySlice` stores one finite positive expiry and a rectangular tenor/strike grid of nonnegative finite vols. Tenors are positive and axes ordered; strike values may be signed. The slice carries BlackLognormal, ShiftedBlackLognormal or Normal quote type. Shifted quotes require one finite displacement per **tenor row**, whereas `VolSurface` shifts are per expiry row. The slice's ordering is tenor-major. Deserialization reruns the same validators.

## Limitations, alternatives

These containers validate numerical admissibility, not model evaluation domains, quote fit, parameter identifiability, calendar arbitrage or strike convexity. Rho endpoints are excluded although some limit formulas may exist; nu zero is supported. The optional shift alone does not specify how node shifts/forwards must be combined off-grid. Generic MarketContext curve-bump dispatch does not shock cubes or SABR node parameters.

The model choices and materialization rules are in [volatility-source interpolation](../models-analytic/volatility-source-interpolation.md), [SABR expansions](../models-analytic/sabr-expansions.md) and [SABR calibration](../models-analytic/sabr-calibration.md). Interpolating model parameters versus node-implied vol or variance changes smiles and is a consumer decision, not selectable computation inside this file.

## Evidence

Owning code: [parameter validator](../../../../finstack-quant/core/src/market_data/surfaces/sabr_parameter_data.rs), [cube](../../../../finstack-quant/core/src/market_data/surfaces/vol_cube.rs), [expiry slice](../../../../finstack-quant/core/src/market_data/surfaces/vol_cube_expiry_slice.rs). Inspected [surface tests](../../../../finstack-quant/core/tests/market_data/surfaces/mod.rs) assert zero-nu support, negative/zero/positive shifts, parameter rejection and round trips. Inspected expiry-slice tests reject missing displacement/invalid expiry and wire corruption. This is domain/serialization evidence; independent Hagan-value/market calibration evidence belongs to model records. Canonical context: [Hagan SABR](../../../REFERENCES.md#hagan-2002-sabr).

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
