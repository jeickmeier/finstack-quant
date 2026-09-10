# Historical SIMM v2.6 correction provenance

Source: [ISDA SIMM v2.6 official methodology](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf), retrieved and visually checked 2026-09-09. SHA256: `57e9d2e9e080e27abc92197923b019e40eb2fe60d9c82ce2a0f8238b7c4bf8e0`.

The former `simm_schedule_parity.rs` attributed self-captured numbers to this PDF but did not reproduce its tables. Replacements are independent transcriptions from the printed pages below. No Bloomberg/QuantLib expectations or tolerances changed.

| Input | Official location | Correction |
|---|---|---|
| IR weights, all three currency groups | D.1, p14 | e.g. regular USD 5Y 60bp; JPY 23bp; high-volatility 97bp |
| IR correlation matrix and intercurrency | D.2, pp14-15 | 66 distinct tenor pairs; subcurve 0.993; intercurrency 0.32 |
| Credit qualifying issuer correlation and vega | E, pp16-18 | same issuer 0.93, different issuer 0.46, residual 0.50; VRW 0.76 |
| Non-qualifying residual | F, pp18-19 | RW 1300bp, VRW 0.76, raw delta threshold USD0.5mm/bp |
| Equity residual | G, pp19-21 | RW 50%, VRW 0.45, HVR 0.60 |
| Commodity | H, pp22-23 | all 17 weights, within-bucket correlations and HVR 0.74; VRW 0.55 |
| FX | I, pp24-25 | USD regular/high weights 7.4/14.7; correlations 0.50/0.25/-0.05; VRW 0.48, HVR 0.57 |
| Delta and vega concentration | J, pp26-28 | Convert published USD millions to absolute dollars; compare raw delta or defined vega risk exposure before risk weighting |
| Cross-class correlations | K, p29 | all 15 distinct pairs, e.g. IR/CQ 4%, IR/equity 7% |
| Curvature | B, paragraph 11, pp7-8 | SF=min(1,14 days/expiry)/2 before factor netting; squared correlations; class theta/lambda; separate residual aggregation; IR HVR adjustment |

`production_simm_csa_audit.rs` supplies dimensional and hand-formula vectors. The implementation remains historical and indicative: product classes, subcurve delta/vega and selected factor/classification dimensions remain outside the supported input shape. Equity and non-qualifying credit use residual classification. This is not certification against an ISDA licensed implementation.
