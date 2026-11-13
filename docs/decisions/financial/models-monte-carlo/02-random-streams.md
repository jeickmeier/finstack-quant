# MODELS-MONTE-CARLO-02: Philox normals and stable stream seeds

- Decision status: **Accepted — complete Philox streams and seed derivation package, 2026-11-11**.
- Implementation status: Existing Philox and seed mapping observed. The accepted fallible draws, cache/exhaustion handling and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent Philox, midpoint, capacity and seed examples checked. Runtime and host qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Philox streams and seed derivation

1. **Pin the shared Philox4x32-10 block profile.** Use an explicit
   effective `u64` seed and absolute `u64` stream ID, both including
   zero. For zero-based block index `j`, use
   `[lo32(j), hi32(j), lo32(stream), hi32(stream)]` and key
   `[lo32(seed), hi32(seed)]` as word values independent of host byte
   order. Execute ten Random123 rounds with multipliers
   `M0=0xD2511F53`, `M1=0xCD9E8D57` and Weyl increments
   `W0=0x9E3779B9`, `W1=0xBB67AE85`. For a round on
   `[v0,v1,v2,v3]`, form the exact unsigned 64-bit products
   `p0=M0*v0` and `p1=M1*v2`. Return
   `[hi32(p1) xor v1 xor k0, lo32(p1), hi32(p0) xor v3 xor k1, lo32(p0)]`.
   Add the Weyl increments modulo `2^32` between rounds.
   [Philox and Random123](../../../REFERENCES.md#salmon-2011-philox).

2. **Define stream selection and address exhaustion.** Use one direct
   constructor with explicit seed and stream ID. Existing seed-only
   callers pass zero when they select the root stream. A fresh stream
   starts at block zero with an empty normal spare.
   A split/substream selects the supplied absolute ID under the same
   key and starts fresh regardless of the parent ID, consumption or
   spare. Cloning copies the exact cursor and spare for replay.
   Distinct IDs for a fixed key use disjoint input blocks and, by the
   fixed-key permutation, disjoint full output blocks. Individual
   truncated values or prefixes can still coincide, and the address
   structure gives no universal statistical independence guarantee.
   Each stream admits all `2^64` block addresses including
   `u64::MAX`. After the last available raw slot, any request needing
   more raw slots fails. Do not wrap, reseed, advance another stream,
   or substitute a generator.

3. **Use exact open midpoint uniforms and uniform-only seek.** Consume
   word pairs `(0,1)` and `(2,3)` in block order. For words
   `(w0,w1)`, take `b=((u64(w1)<<32)|u64(w0))>>12` and return
   `(b+1/2)*2^-52`. This gives `2^52` exactly representable
   midpoints from `2^-53` through `1-2^-53`. The complement
   `1-u` is exact and corresponds to `(2^52-1)-b`.
   Retain the crate-private `u64` uniform offset used by rates-credit
   sampling. Offset `k` starts at block `floor(k/2)`, pair
   `k mod 2`, with no spare. It addresses the first `2^64` of the
   stream's `2^65` raw uniform slots. It restores uniform-only
   position. Mixed uniform/normal replay requires the original call
   order or an exact clone. Model checkpoints retain their separately
   validated provenance. [Accepted rates-credit sampling](../models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling).

4. **Use the shared checked reflected Box–Muller transform.** Pass two
   successive actual open uniforms to core. Use
   `r=sqrt(-2*log1p(-u1))`, `theta=2*pi*u2` and the ordered
   pair `(r*cos(theta), r*sin(theta))`. Take a cached standard
   normal first if present, then generate pairs and keep an unused
   second normal. Interposed uniform requests leave the spare intact.
   A zero-length fill changes neither the cursor nor the spare.
   Scaling belongs to the model or scheme. The finite midpoint grid
   has a largest ideal radius `sqrt(106*ln(2))` (about 8.57)
   and yields discrete variates. Pin the transformed sequence to
   this accepted core formula, including its reflection relative
   to the observed runtime. [Core random transforms](../core-numerics/pseudorandom-and-bridge.md),
   [Box–Muller](../../../REFERENCES.md#box-muller-1958).

5. **Make scalar and bulk draws fallible.** Propagate capacity and
   transform failures through the Rust stream interface and actual
   pricing, risk and simulation users. Preflight a bulk request using
   checked widened counts. For `n` normals and `s` usable cached
   normals (0 or 1), the raw uniform demand is
   `2*ceil(max(n-s,0)/2)`. A capacity failure leaves the cursor,
   spare and output buffer unchanged. An exhausted raw stream can
   still deliver its remaining spare. If a checked transform fails
   during a fill, its attempted pair is consumed, no companion is
   retained from the failed pair, and the caller discards the fill's
   output. No wrap, retry, numerical sentinel or replacement draw.
   Preserve operation and path/node context at the caller boundary.

6. **Retain the existing FNV-1a-64 derivation when explicitly selected.**
   Hash the literal UTF-8 instrument bytes, one byte `0xff`, then
   the literal UTF-8 scenario bytes. Start at
   `0xcbf29ce484222325`. For each byte XOR first, then multiply
   by `0x100000001b3` modulo `2^64`. No normalization, trimming
   or case conversion. The delimiter is outside valid UTF-8 and
   makes the encoded pair unambiguous. The resulting 64-bit seed can
   collide. Preserve a caller-supplied literal seed exactly, including
   zero. The product can explicitly select a named scenario and the
   derivation. Remove silent inferred labels or replacement seeds on
   covered routes. Empty labels supplied to the helper are literal
   bytes when admitted by the owner. A product requiring distinct
   effective keys must reject equality. No automatic rehash,
   collision registry or identity/security guarantee.
   [FNV and UTF-8 framing](../../../REFERENCES.md#fnv-1a-seed-derivation-and-utf-8-framing).

7. **Keep draw assignment and reproducibility with the owning method.**
   Common random numbers require matching effective key, stream
   ID and logical draw assignment. Matching seed values alone
   cannot align a changed grid, factor order, payoff-uniform demand,
   cache consumption or antithetic layout. The primary engine uses
   the accepted estimator IDs and fixed run layout. Exact Hull–White
   keeps one checked pair per positive interval in state-then-integral
   order and no zero-time draw. Rates-credit keeps its departing-step
   uniform address, including reserved slots for deterministic steps
   and no terminal draw. Owners select training/replay and risk
   namespaces and check any required separation. Pin the integer
   words and exact midpoint values across targets. Transcendental
   normal draws and pricing comparisons follow the accepted
   reproducibility tier and its qualified cross-target tolerances.
   [Primary engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [exact Hull–White sampling](../models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling),
   [execution standards](../../standards/0043-deterministic-ordering-and-monte-carlo-execution.md).

8. **Keep Sobol whole-point and host boundaries.** Remove the public
   scalar `RandomStream` implementation for core Sobol as already
   accepted. Retain checked whole-point delivery and the private
   precomputed-path adapter for the dedicated RQMC driver. Record 03
   owns Sobol replicate allocation, auxiliary Philox payoff draws,
   draw layout and uncertainty. The generic IID pricing engine
   rejects QMC. Keep RNGs and derivation Rust-only. Reconcile the
   existing product controls, error mappings, schemas and replay
   metadata. Preserve exact `u64` values in Python integers,
   JavaScript safe integers or BigInt inputs, BigInt for full-width
   JS replay output, and exact integer tokens in raw JSON parsed by
   Rust. Reject lossy JS numbers and invalid unsigned seed values.
   [Accepted Sobol profile](../core-numerics/sobol-sequences.md),
   [dedicated RQMC](03-randomized-qmc.md).

## Observed choice, domain and rationale

The production pseudorandom stream is counter-based Philox4x32 with ten rounds. The key and addressed block determine the integer words independently of worker scheduling. Current Sobol integration includes a public scalar adapter as well as a dedicated whole-path driver.

## Observed algorithm and controls

Philox packs the 64-bit counter into the first two counter words and stream ID into the last two, uses the two seed words as keys, and executes ten multiply/high-low/XOR rounds with Weyl key increments. Uniforms take 52 upper random bits and add one half, yielding values strictly inside `(0,1)`. Two uniforms feed core Box–Muller, and an unused normal is normally cached across odd requests. `with_stream_u01_offset` selects the block and word pair directly. Splitting starts a fresh stream. `derive_seed` hashes instrument bytes, delimiter `0xff`, and scenario bytes with FNV-1a-64.

## Observed failure behavior

The block cursor wraps on overflow. The normal fill removes a spare before it checks whether the output is empty. Public draws return infallibly and the observed core transform uses a clamped direct logarithm. The valuation seed resolver and Hull–White scenario helper can infer a base label. The FNV helper's documentation overstates distinct-scenario separation. The generic engine rejects quasi-random input despite the current scalar Sobol implementation.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| Random123 all-zero key and counter | Block words `6627e8d5 e169c58d bc57ac4c 9b00dbd8`. First midpoint indices `e169c58d6627e` and `9b00dbd8bc57a` in hexadecimal. The reflected cosine/sine pair is approximately `(-1.62496344087104,-1.26834920254695)`. |
| Random123 all-one bits in key/counter, or the published mixed case | All-ones words `408f276d 41c83b0e a20bc7c6 6d5451fd`. Mixed seed/stream/block `299f31d0a4093822 / 0370734413198a2e / 85a308d3243f6a88` gives `d16cfe09 94fdcceb 5001e420 24126ea1`. |
| Fresh stream, one normal, empty normal fill, one uniform, one normal, one uniform | First normal consumes uniforms 0 and 1. The uniform outputs use slots 2 and 3. The intervening normal is the original sine spare. |
| One raw uniform left, with no spare | One normal fails without advancing. One uniform can still consume the final raw slot. With a spare and no raw slots, a one-normal fill succeeds and a two-normal fill fails without discarding the spare. |
| Two different component boundaries and two Unicode forms | `("ab","c") -> fdf3a083ef9bc024` and `("a","bc") -> d2b3f5819298d9d6`. U+00E9 with an empty label gives `5fec201c1dfca99a`. U+0065 followed by U+0301 with an empty label gives `1722465d43cfa6a0`. |
| Existing seed and full-width transport fixtures | `("TEST-001","base") -> f4f7d33fea0c2ce9`. Preserve literal zero and `2^64-1` in admitted exact carriers. A JS Number for `2^53` is outside the safe-integer contract. Use BigInt or exact raw JSON for full-width seeds. |

Qualification includes published block vectors, odd/even offsets and
segmented replay, midpoint endpoint/complement identities, cache and
mixed-call traces, checked capacity at the last block, exact seed transport,
propagated failures, and actual same-target replay through model callers.
Independently qualify finite-output stream behavior and financial results
under each owner's layout and error budget. Known-answer and arithmetic
checks verify profile details. They establish no universal independence,
financial tail coverage or production price accuracy.

## Evidence

Existing tests include `test_philox_matches_random123_reference_vector`,
stream-correlation and open-endpoint checks, spare-normal cases and a fixed
FNV vector. Reconcile the fallible shared stream with generic pricing,
simulation, Hull–White, joint tree segment replay, Greeks, LSMC and rough
drivers. Reconcile the actual seed resolution and existing host callers in
the same slice. [Philox implementation and tests](../../../../finstack-quant/models/src/monte_carlo/rng/philox.rs),
[seed hashing](../../../../finstack-quant/models/src/monte_carlo/seed.rs),
[current Sobol adapter](../../../../finstack-quant/models/src/monte_carlo/rng/sobol.rs),
[valuation seed resolver](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs),
[Hull–White configuration](../../../../finstack-quant/models/src/rates/hw1f/config.rs),
[tree sampling](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs),
[WASM seed input](../../../../finstack-quant-wasm/src/utils/input.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
