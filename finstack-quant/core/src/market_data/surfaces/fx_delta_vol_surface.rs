//! FX delta-based volatility surface and checked delta/quote math.
//!
//! Stores FX option volatility quotes in delta space (ATM DNS, 25-delta RR/BF,
//! optional 10-delta RR/BF) alongside explicit pair, quote profile, and optional
//! expiry clock metadata. Delta-to-strike conversion, volatility evaluation,
//! and strike-grid materialization live in `finstack-quant-models`.
//!
//! # Delta Convention
//!
//! This module uses **forward delta** (premium-unadjusted) throughout:
//!
//! ```text
//! Delta_call = N(d1)
//! d1 = [ln(F/K) + 0.5 * sigma^2 * T] / (sigma * sqrt(T))
//! ```
//!
//! Inverting gives:
//!
//! ```text
//! K_call(p) = F * exp(-N_inv(p) * sigma_call * sqrt(T) + 0.5 * sigma_call^2 * T)
//! K_put(p)  = F * exp(-N_inv(1 - p) * sigma_put * sqrt(T) + 0.5 * sigma_put^2 * T)
//! K_DNS     = F * exp(0.5 * sigma_ATM^2 * T)
//! ```
//!
//! # Smile Butterfly Convention
//!
//! Risk reversals and butterflies use the two-volatility arithmetic **smile
//! butterfly** convention:
//!
//! ```text
//! RR_p = sigma_call,p - sigma_put,p
//! BF_p = 0.5 * (sigma_call,p + sigma_put,p) - sigma_ATM
//! sigma_call,p = sigma_ATM + BF_p + 0.5 * RR_p
//! sigma_put,p  = sigma_ATM + BF_p - 0.5 * RR_p
//! ```
//!
//! # References
//!
//! - Wystup (2006), *FX Options and Structured Products*, Ch. 1 `docs/REFERENCES.md#wystup-fx-options`
//! - Clark (2011), *Foreign Exchange Option Pricing*, Ch. 3-4 `docs/REFERENCES.md#clark-fx-options`

use super::VolQuoteType;
use crate::{
    dates::VolatilityClock,
    error::InputError,
    math::special_functions::{log_norm_cdf, standard_normal_inv_cdf},
    money::fx::CurrencyPair,
    types::CurveId,
};

/// Default relative delta matching tolerance used by checked delta-to-strike inversion.
pub const DEFAULT_FX_DELTA_REL_TOL: f64 = 1e-3;

const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fee0_0000);
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;

/// Delta convention governing how [`FxDeltaVolSurface`] maps wing deltas onto strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(rename = "FxSurfaceDeltaConvention")
)]
#[serde(rename_all = "snake_case")]
pub enum FxDeltaConvention {
    /// Premium-unadjusted forward delta (`Delta_call = N(d1)`).
    ForwardUnadjusted,
}

impl FxDeltaConvention {
    /// Return the canonical `snake_case` wire label for this delta convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForwardUnadjusted => "forward_unadjusted",
        }
    }
}

impl std::fmt::Display for FxDeltaConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxDeltaConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "forward_unadjusted" => Ok(Self::ForwardUnadjusted),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX delta convention '{s}'; expected 'forward_unadjusted'"
            ))),
        }
    }
}

/// At-the-money strike convention on [`FxDeltaVolSurface`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxAtmConvention {
    /// Forward delta-neutral straddle (`K_ATM = F * exp(0.5 * sigma_ATM^2 * T)`).
    DeltaNeutralStraddle,
}

impl FxAtmConvention {
    /// Return the canonical `snake_case` wire label for this ATM convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeltaNeutralStraddle => "delta_neutral_straddle",
        }
    }
}

impl std::fmt::Display for FxAtmConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxAtmConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "delta_neutral_straddle" => Ok(Self::DeltaNeutralStraddle),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX ATM convention '{s}'; expected 'delta_neutral_straddle'"
            ))),
        }
    }
}

/// Butterfly quote convention on [`FxDeltaVolSurface`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxButterflyConvention {
    /// Arithmetic two-volatility smile butterfly (`0.5 * (sigma_call + sigma_put) - sigma_ATM`),
    /// distinct from a market-strangle premium-consistency solve.
    Smile,
}

impl FxButterflyConvention {
    /// Return the canonical `snake_case` wire label for this butterfly convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Smile => "smile",
        }
    }
}

impl std::fmt::Display for FxButterflyConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxButterflyConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "smile" => Ok(Self::Smile),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX butterfly convention '{s}'; expected 'smile'"
            ))),
        }
    }
}

#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bv = s - a;
    let e = (a - (s - bv)) + (b - bv);
    (s, e)
}

#[inline]
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let e = a.mul_add(b, -p);
    (p, e)
}

#[inline]
fn odd_round(s: f64, e: f64) -> f64 {
    if e == 0.0 || (s.to_bits() & 1) == 1 {
        s
    } else if e > 0.0 {
        s.next_up()
    } else {
        s.next_down()
    }
}

fn grow_expansion(e: &mut [f64; 64], len: &mut usize, b: f64) {
    if b == 0.0 {
        return;
    }
    let mut q = b;
    let mut write = 0;
    for read in 0..*len {
        let (q_new, h) = two_sum(q, e[read]);
        q = q_new;
        if h != 0.0 {
            e[write] = h;
            write += 1;
        }
    }
    if q != 0.0 {
        e[write] = q;
        write += 1;
    }
    *len = write;
}

fn compress_expansion(e: &mut [f64; 64], len: &mut usize) {
    if *len <= 1 {
        return;
    }
    let mut scratch = [0.0_f64; 64];
    for _ in 0..2 {
        let mut q = e[*len - 1];
        let mut bottom = *len - 1;
        for i in (0..*len - 1).rev() {
            let (q_new, h) = two_sum(q, e[i]);
            if h != 0.0 {
                scratch[bottom] = q_new;
                bottom -= 1;
                q = h;
            } else {
                q = q_new;
            }
        }
        let mut top = 0;
        for &val in &scratch[bottom + 1..*len] {
            let (q_new, h) = two_sum(val, q);
            if h != 0.0 {
                e[top] = h;
                top += 1;
            }
            q = q_new;
        }
        if q != 0.0 {
            e[top] = q;
            top += 1;
        }
        *len = top;
        if *len <= 1 {
            return;
        }
    }
}

#[inline]
fn pow2_f64(exp: i32) -> f64 {
    debug_assert!((-1022..=1023).contains(&exp));
    f64::from_bits(((exp + 1023) as u64) << 52)
}

fn scale_by_pow2(mut value: f64, mut exp: i32) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    while exp > 1000 {
        value *= pow2_f64(1000);
        exp -= 1000;
    }
    while exp < -1000 {
        value *= pow2_f64(-1000);
        exp += 1000;
    }
    value * pow2_f64(exp)
}

fn frexp_f64(x: f64) -> (f64, i32) {
    if x >= f64::MIN_POSITIVE {
        let bits = x.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32 - 1022;
        let mant_bits = (bits & 0x000f_ffff_ffff_ffff) | 0x3fe0_0000_0000_0000;
        (f64::from_bits(mant_bits), exp)
    } else {
        let scaled = x * pow2_f64(54);
        let bits = scaled.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32 - 1022 - 54;
        let mant_bits = (bits & 0x000f_ffff_ffff_ffff) | 0x3fe0_0000_0000_0000;
        (f64::from_bits(mant_bits), exp)
    }
}

/// Evaluate exact sum of `terms`, then multiply by `2^out_pow2` with single IEEE-754 rounding.
fn exact_sum_scaled_to_f64(terms: &[f64], out_pow2: i32) -> f64 {
    let mut e = [0.0_f64; 64];
    let mut len = 0_usize;
    for &t in terms {
        grow_expansion(&mut e, &mut len, t);
    }
    compress_expansion(&mut e, &mut len);
    if len == 0 {
        return 0.0;
    }
    if len == 1 {
        return scale_by_pow2(e[0], out_pow2);
    }
    let hi = e[len - 1];
    let mut tail = 0.0_f64;
    for &x in &e[..len - 1] {
        let (s, err) = two_sum(tail, x);
        tail = odd_round(s, err);
    }

    let hi_abs = hi.abs();
    let (_, hi_exp) = frexp_f64(hi_abs);
    // Target exponent of MSB (where `1.0 * 2^e` corresponds to true_exp = hi_exp - 1 + out_pow2).
    let true_msb_exp = hi_exp - 1 + out_pow2;
    if true_msb_exp >= -1022 {
        let (rounded_hi, _) = two_sum(hi, tail);
        scale_by_pow2(rounded_hi, out_pow2)
    } else if true_msb_exp < -1075 {
        if hi >= 0.0 { 0.0 } else { -0.0 }
    } else {
        // Output lands in the subnormal range: round the 53-bit normal significand plus
        // exact tail direction directly to target subnormal precision in a single step.
        let sign_bit = hi.to_bits() & 0x8000_0000_0000_0000;
        let signed_tail = if hi >= 0.0 { tail } else { -tail };
        let sig53 = (1_u64 << 52) | (hi_abs.to_bits() & 0x000f_ffff_ffff_ffff);
        let shift = (-1022 - true_msb_exp) as u32;
        let mut q = sig53 >> shift;
        let mask = (1_u64 << shift) - 1;
        let rem = sig53 & mask;
        let half = 1_u64 << (shift - 1);
        let round_up = if rem > half {
            true
        } else if rem < half {
            false
        } else if signed_tail > 0.0 {
            true
        } else if signed_tail < 0.0 {
            false
        } else {
            (q & 1) == 1
        };
        if round_up {
            q += 1;
        }
        f64::from_bits(sign_bit | q)
    }
}

/// Recover 25-delta or 10-delta `(sigma_put, sigma_call)` from ATM DNS, RR, and smile BF quotes.
///
/// Uses power-of-2 scaling and exact compensated summation across `2 * (ATM + BF) ± RR`
/// followed by a single final rounding step so large cancellation or tiny valid wings
/// are preserved without subnormal half-RR distortion or intermediate rounding loss.
///
/// # Arguments
///
/// * `atm` - Positive finite ATM delta-neutral-straddle volatility (decimal).
/// * `rr` - Finite risk reversal quote (`sigma_call - sigma_put`, decimal).
/// * `bf` - Finite arithmetic smile butterfly quote (`0.5 * (sigma_call + sigma_put) - atm`, decimal).
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when `atm` or either recovered wing is `<= 0.0`
/// (including cancellation or exact subnormal underflow to zero), or [`crate::Error::Validation`]
/// when an input is non-finite or a positive recovered wing overflows binary64.
pub fn recover_fx_wing_vols(atm: f64, rr: f64, bf: f64) -> crate::Result<(f64, f64)> {
    if !atm.is_finite() || atm <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    if !rr.is_finite() || !bf.is_finite() {
        return Err(crate::Error::Validation(
            "FX risk reversal and butterfly quotes must be finite".to_string(),
        ));
    }

    let max_abs = atm.max(rr.abs()).max(bf.abs());
    let mut min_nz = atm;
    if rr != 0.0 {
        min_nz = min_nz.min(rr.abs());
    }
    if bf != 0.0 {
        min_nz = min_nz.min(bf.abs());
    }

    let in_shift = if max_abs > pow2_f64(400) {
        -520
    } else if min_nz < pow2_f64(-900) {
        520
    } else {
        0
    };

    let atm_s = scale_by_pow2(atm, in_shift);
    let bf_s = scale_by_pow2(bf, in_shift);
    let rr_s = scale_by_pow2(rr, in_shift);

    // Multiplication by 2 is exact inside the scaled normal range.
    let two_atm = 2.0 * atm_s;
    let two_bf = 2.0 * bf_s;

    let sigma_put = exact_sum_scaled_to_f64(&[two_atm, two_bf, -rr_s], -in_shift - 1);
    let sigma_call = exact_sum_scaled_to_f64(&[two_atm, two_bf, rr_s], -in_shift - 1);

    if !sigma_put.is_finite() || !sigma_call.is_finite() {
        return Err(crate::Error::Validation(
            "recovered FX wing volatility overflows binary64".to_string(),
        ));
    }
    if sigma_put <= 0.0 || sigma_call <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok((sigma_put, sigma_call))
}

/// Linearly interpolate `(atm, rr, bf)` quotes between two rows and reconstruct
/// `(atm, sigma_put, sigma_call)` with one final rounding per volatility.
///
/// Evaluates the interpolated affine combinations across all six unrounded
/// endpoint products so exact cancellation across `(ATM, BF, RR)` endpoints
/// does not lose valid positive wings through per-lane rounding.
///
/// # Arguments
///
/// * `w0` - Non-negative finite weight applied to `row0`.
/// * `row0` - First endpoint tuple `(atm, rr, bf)` in decimal volatility units.
/// * `w1` - Non-negative finite weight applied to `row1`.
/// * `row1` - Second endpoint tuple `(atm, rr, bf)` in decimal volatility units.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when either row has `atm <= 0.0` or
/// any interpolated output is `<= 0.0`, or [`crate::Error::Validation`] when
/// weights or quotes are invalid or an output overflows binary64.
pub fn interpolate_fx_wing_vols(
    w0: f64,
    row0: (f64, f64, f64),
    w1: f64,
    row1: (f64, f64, f64),
) -> crate::Result<(f64, f64, f64)> {
    let (atm0, rr0, bf0) = row0;
    let (atm1, rr1, bf1) = row1;
    if !w0.is_finite() || !w1.is_finite() || w0 < 0.0 || w1 < 0.0 {
        return Err(crate::Error::Validation(
            "FX interpolation weights must be finite and non-negative".to_string(),
        ));
    }
    if !atm0.is_finite() || atm0 <= 0.0 || !atm1.is_finite() || atm1 <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    if !rr0.is_finite() || !bf0.is_finite() || !rr1.is_finite() || !bf1.is_finite() {
        return Err(crate::Error::Validation(
            "FX risk reversal and butterfly quotes must be finite".to_string(),
        ));
    }

    let mut min_nz_q = atm0.min(atm1);
    let mut max_q = atm0.max(atm1);
    for q in [rr0.abs(), bf0.abs(), rr1.abs(), bf1.abs()] {
        if q > max_q {
            max_q = q;
        }
        if q != 0.0 && q < min_nz_q {
            min_nz_q = q;
        }
    }
    let q_shift = if max_q > pow2_f64(300) {
        -400
    } else if min_nz_q < pow2_f64(-800) && max_q < pow2_f64(300) {
        400
    } else {
        0
    };

    let mut min_nz_w = f64::INFINITY;
    let max_w = w0.max(w1);
    for w in [w0, w1] {
        if w != 0.0 && w < min_nz_w {
            min_nz_w = w;
        }
    }
    let w_shift = if max_w > pow2_f64(300) {
        -400
    } else if min_nz_w < pow2_f64(-800) && max_w < pow2_f64(300) {
        400
    } else {
        0
    };

    let w0_s = scale_by_pow2(w0, w_shift);
    let w1_s = scale_by_pow2(w1, w_shift);
    let a0_s = scale_by_pow2(atm0, q_shift);
    let r0_s = scale_by_pow2(rr0, q_shift);
    let b0_s = scale_by_pow2(bf0, q_shift);
    let a1_s = scale_by_pow2(atm1, q_shift);
    let r1_s = scale_by_pow2(rr1, q_shift);
    let b1_s = scale_by_pow2(bf1, q_shift);

    let out_shift_atm = -q_shift - w_shift;
    let out_shift_wing = -q_shift - w_shift - 1;

    let (pa0, ea0) = two_prod(w0_s, a0_s);
    let (pa1, ea1) = two_prod(w1_s, a1_s);
    let atm = exact_sum_scaled_to_f64(&[pa0, ea0, pa1, ea1], out_shift_atm);

    let (p2a0, e2a0) = two_prod(w0_s, 2.0 * a0_s);
    let (p2b0, e2b0) = two_prod(w0_s, 2.0 * b0_s);
    let (pr0, er0) = two_prod(w0_s, r0_s);
    let (p2a1, e2a1) = two_prod(w1_s, 2.0 * a1_s);
    let (p2b1, e2b1) = two_prod(w1_s, 2.0 * b1_s);
    let (pr1, er1) = two_prod(w1_s, r1_s);

    let sigma_put = exact_sum_scaled_to_f64(
        &[
            p2a0, e2a0, p2b0, e2b0, -pr0, -er0, p2a1, e2a1, p2b1, e2b1, -pr1, -er1,
        ],
        out_shift_wing,
    );
    let sigma_call = exact_sum_scaled_to_f64(
        &[
            p2a0, e2a0, p2b0, e2b0, pr0, er0, p2a1, e2a1, p2b1, e2b1, pr1, er1,
        ],
        out_shift_wing,
    );

    if !atm.is_finite() || !sigma_put.is_finite() || !sigma_call.is_finite() {
        return Err(crate::Error::Validation(
            "interpolated FX volatility overflows binary64".to_string(),
        ));
    }
    if atm <= 0.0 || sigma_put <= 0.0 || sigma_call <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok((atm, sigma_put, sigma_call))
}

/// Compute `ln(num / den)` stably across extreme ratios without overflow or underflow.
///
/// # Arguments
///
/// * `num` - Finite strictly positive numerator.
/// * `den` - Finite strictly positive denominator.
#[must_use]
pub fn checked_ln_ratio(num: f64, den: f64) -> f64 {
    let ratio = num / den;
    if ratio.is_finite() && (0.5..=2.0).contains(&ratio) {
        return ((num - den) / den).ln_1p();
    }
    let (m_n, e_n) = frexp_f64(num);
    let (m_d, e_d) = frexp_f64(den);
    let exp_diff = f64::from(e_n - e_d);
    (m_n / m_d).ln() + exp_diff * std::f64::consts::LN_2
}

fn scaled_exp_product(forward: f64, log_ratio: f64) -> crate::Result<f64> {
    if !forward.is_finite() || forward <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    if !log_ratio.is_finite() {
        return Err(crate::Error::Validation(
            "FX strike log-ratio must be finite".to_string(),
        ));
    }
    if (-700.0..=700.0).contains(&log_ratio) {
        let k = forward * log_ratio.exp();
        if k.is_finite() && k >= f64::MIN_POSITIVE {
            return Ok(k);
        }
    }
    if log_ratio > 2500.0 {
        return Err(crate::Error::Validation(
            "FX strike overflows binary64".to_string(),
        ));
    }
    if log_ratio < -2500.0 {
        return Err(crate::Error::Validation(
            "FX strike underflows binary64".to_string(),
        ));
    }
    let (m, e_f) = frexp_f64(forward);
    let k_f = (log_ratio / std::f64::consts::LN_2).round();
    let k_int = k_f as i32;
    let rem = (log_ratio - k_f * LN2_HI) - k_f * LN2_LO;
    let base = m * rem.exp();
    let total_exp = e_f + k_int;
    if total_exp > 1024 {
        return Err(crate::Error::Validation(
            "FX strike overflows binary64".to_string(),
        ));
    }
    if total_exp < -1075 {
        return Err(crate::Error::Validation(
            "FX strike underflows binary64".to_string(),
        ));
    }
    let strike = scale_by_pow2(base, total_exp);
    if !strike.is_finite() || strike <= 0.0 {
        return Err(crate::Error::Validation(
            "FX strike is not representable as a finite positive binary64 value".to_string(),
        ));
    }
    Ok(strike)
}

/// Compute the delta-neutral-straddle ATM strike `K_DNS = F * exp(0.5 * vol^2 * expiry)`.
///
/// # Arguments
///
/// * `forward` - Finite positive FX forward in quote currency per base unit.
/// * `vol` - Finite positive annualized Black volatility as a decimal.
/// * `expiry` - Finite positive time to expiry in years.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when any input is non-positive or non-finite,
/// or [`crate::Error::Validation`] when the strike overflows binary64.
pub fn fx_atm_dns_strike_checked(forward: f64, vol: f64, expiry: f64) -> crate::Result<f64> {
    if !forward.is_finite()
        || forward <= 0.0
        || !vol.is_finite()
        || vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    let std_dev = vol * expiry.sqrt();
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(crate::Error::Validation(
            "FX volatility standard deviation underflowed or overflowed".to_string(),
        ));
    }
    let half_var = 0.5 * std_dev * std_dev;
    if !half_var.is_finite() {
        return Err(crate::Error::Validation(
            "FX strike variance term overflowed binary64".to_string(),
        ));
    }
    scaled_exp_product(forward, half_var)
}

/// Convert an option side and absolute forward delta into a strike with relative tail verification.
///
/// Uses direct lower/upper tail inversion (`standard_normal_inv_cdf(p)` for `p <= 0.5` and
/// `-standard_normal_inv_cdf(1 - p)` for `p > 0.5`), scaled log/exp strike evaluation, and
/// checks the represented strike's Black forward delta in the smaller log-probability tail
/// against `delta_rel_tol`.
///
/// # Arguments
///
/// * `is_call` - `true` for a call option (`Delta_call = N(d1)`), `false` for a put (`|Delta_put| = N(-d1)`).
/// * `delta_abs` - Absolute forward delta strictly in the open unit interval `(0, 1)`.
/// * `forward` - Finite positive FX forward in quote currency per base unit.
/// * `vol` - Finite positive annualized Black volatility as a decimal.
/// * `expiry` - Finite positive time to expiry in years.
/// * `delta_rel_tol` - Finite strictly positive relative delta acceptance tolerance (for example `1e-3`).
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when `forward`, `vol`, or `expiry` is non-positive
/// or non-finite, or [`crate::Error::Validation`] when `delta_abs` is outside `(0, 1)`,
/// `delta_rel_tol` is invalid, the strike is non-representable, or floating-point rounding
/// causes the represented strike's delta to diverge beyond `delta_rel_tol`.
pub fn fx_option_delta_to_strike(
    is_call: bool,
    delta_abs: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
    delta_rel_tol: f64,
) -> crate::Result<f64> {
    if !forward.is_finite()
        || forward <= 0.0
        || !vol.is_finite()
        || vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    if !delta_abs.is_finite() || delta_abs <= 0.0 || delta_abs >= 1.0 {
        return Err(crate::Error::Validation(format!(
            "FX forward delta magnitude must lie strictly in (0, 1), got {delta_abs}"
        )));
    }
    if !delta_rel_tol.is_finite() || delta_rel_tol <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "FX delta relative tolerance must be finite and positive, got {delta_rel_tol}"
        )));
    }

    let std_dev = vol * expiry.sqrt();
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(crate::Error::Validation(
            "FX option standard deviation must be finite and positive".to_string(),
        ));
    }
    let half_var = 0.5 * std_dev * std_dev;
    if !half_var.is_finite() {
        return Err(crate::Error::Validation(
            "FX option variance term overflowed binary64".to_string(),
        ));
    }

    let z_q = if delta_abs <= 0.5 {
        standard_normal_inv_cdf(delta_abs)?
    } else {
        -standard_normal_inv_cdf(1.0 - delta_abs)?
    };
    if !z_q.is_finite() {
        return Err(crate::Error::Validation(
            "FX normal quantile is non-finite".to_string(),
        ));
    }
    let signed_z = if is_call { -z_q } else { z_q };
    let log_ratio = signed_z.mul_add(std_dev, half_var);
    let strike = scaled_exp_product(forward, log_ratio)?;

    let d1_actual = checked_ln_ratio(forward, strike) / std_dev + 0.5 * std_dev;
    if !d1_actual.is_finite() {
        return Err(crate::Error::Validation(
            "FX represented strike produces non-finite d1".to_string(),
        ));
    }
    let z_actual = if is_call { d1_actual } else { -d1_actual };
    let (log_p_target, log_p_actual) = if delta_abs <= 0.5 {
        (delta_abs.ln(), log_norm_cdf(z_actual)?)
    } else {
        ((1.0 - delta_abs).ln(), log_norm_cdf(-z_actual)?)
    };
    if !log_p_actual.is_finite() {
        return Err(crate::Error::Validation(
            "FX represented strike log-delta is non-finite".to_string(),
        ));
    }
    let rel_err = (log_p_actual - log_p_target).exp_m1().abs();
    if !rel_err.is_finite() || rel_err > delta_rel_tol {
        return Err(crate::Error::Validation(format!(
            "FX represented strike {strike} delta relative error {rel_err} exceeds tolerance {delta_rel_tol}"
        )));
    }
    Ok(strike)
}

/// Construct checked `(strikes, vols)` pillars for a single FX expiry smile and
/// verify strict ascending strike order `K_10P < K_25P < K_DNS < K_25C < K_10C`.
///
/// # Arguments
///
/// * `forward` - Finite positive FX forward in quote currency per base unit.
/// * `expiry` - Finite positive expiry time in years.
/// * `atm` - Positive finite ATM DNS volatility (decimal).
/// * `wings_25d` - Reconstructed 25-delta wing volatilities `(sigma_25p, sigma_25c)`.
/// * `wings_10d` - Optional reconstructed 10-delta wing volatilities `(sigma_10p, sigma_10c)`.
/// * `delta_rel_tol` - Relative delta acceptance tolerance for strike inversion.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] or [`crate::Error::Validation`] if any
/// delta-to-strike conversion fails or if pillar strikes cross or collide.
pub fn fx_smile_pillars_checked(
    forward: f64,
    expiry: f64,
    atm: f64,
    wings_25d: (f64, f64),
    wings_10d: Option<(f64, f64)>,
    delta_rel_tol: f64,
) -> crate::Result<(Vec<f64>, Vec<f64>)> {
    let (sigma_put_25d, sigma_call_25d) = wings_25d;
    let k_atm = fx_atm_dns_strike_checked(forward, atm, expiry)?;
    let k_put_25d =
        fx_option_delta_to_strike(false, 0.25, forward, sigma_put_25d, expiry, delta_rel_tol)?;
    let k_call_25d =
        fx_option_delta_to_strike(true, 0.25, forward, sigma_call_25d, expiry, delta_rel_tol)?;

    let (strikes, vols) = if let Some((sigma_put_10d, sigma_call_10d)) = wings_10d {
        let k_put_10d =
            fx_option_delta_to_strike(false, 0.10, forward, sigma_put_10d, expiry, delta_rel_tol)?;
        let k_call_10d =
            fx_option_delta_to_strike(true, 0.10, forward, sigma_call_10d, expiry, delta_rel_tol)?;
        (
            vec![k_put_10d, k_put_25d, k_atm, k_call_25d, k_call_10d],
            vec![
                sigma_put_10d,
                sigma_put_25d,
                atm,
                sigma_call_25d,
                sigma_call_10d,
            ],
        )
    } else {
        (
            vec![k_put_25d, k_atm, k_call_25d],
            vec![sigma_put_25d, atm, sigma_call_25d],
        )
    };

    for pair in strikes.windows(2) {
        if pair[0].partial_cmp(&pair[1]) != Some(std::cmp::Ordering::Less) {
            return Err(crate::Error::Validation(format!(
                "FX smile pillar strikes must be strictly increasing across put, ATM, and call deltas (got {} followed by {})",
                pair[0], pair[1]
            )));
        }
    }
    Ok((strikes, vols))
}

/// Construction parameters for [`FxDeltaVolSurface::from_params`].
#[derive(Clone, Debug, PartialEq)]
pub struct FxDeltaVolSurfaceParams {
    /// Unique non-empty surface identifier.
    pub id: CurveId,
    /// Currency pair (`base/quote`) whose forward is expressed in quote units per base unit.
    pub pair: CurrencyPair,
    /// Forward delta convention governing delta-to-strike inversion.
    pub delta_convention: FxDeltaConvention,
    /// At-the-money strike convention (`DeltaNeutralStraddle`).
    pub atm_convention: FxAtmConvention,
    /// Butterfly quote convention (`Smile`: wing average minus ATM).
    pub butterfly_convention: FxButterflyConvention,
    /// Strictly increasing positive expiry times in years.
    pub expiries: Vec<f64>,
    /// Positive ATM delta-neutral-straddle volatilities per expiry (decimal).
    pub atm_vols: Vec<f64>,
    /// 25-delta risk reversals per expiry (`sigma_call - sigma_put`, decimal).
    pub rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterflies per expiry (decimal).
    pub bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversals per expiry; must be paired with `bf_10d`.
    pub rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterflies per expiry; must be paired with `rr_10d`.
    pub bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates into numeric expiry times.
    pub expiry_clock: Option<VolatilityClock>,
}

/// Builder for constructing a validated [`FxDeltaVolSurface`] without positional argument sprawl.
#[derive(Clone, Debug)]
pub struct FxDeltaVolSurfaceBuilder {
    id: CurveId,
    pair: CurrencyPair,
    delta_convention: Option<FxDeltaConvention>,
    atm_convention: Option<FxAtmConvention>,
    butterfly_convention: Option<FxButterflyConvention>,
    expiries: Option<Vec<f64>>,
    atm_vols: Option<Vec<f64>>,
    rr_25d: Option<Vec<f64>>,
    bf_25d: Option<Vec<f64>>,
    rr_10d: Option<Vec<f64>>,
    bf_10d: Option<Vec<f64>>,
    expiry_clock: Option<VolatilityClock>,
}

impl FxDeltaVolSurfaceBuilder {
    /// Set the forward delta convention (`FxDeltaConvention::ForwardUnadjusted`).
    ///
    /// # Arguments
    ///
    /// * `convention` - Forward delta convention governing wing strikes.
    #[must_use]
    pub fn delta_convention(mut self, convention: FxDeltaConvention) -> Self {
        self.delta_convention = Some(convention);
        self
    }

    /// Set the at-the-money convention (`FxAtmConvention::DeltaNeutralStraddle`).
    ///
    /// # Arguments
    ///
    /// * `convention` - ATM strike definition.
    #[must_use]
    pub fn atm_convention(mut self, convention: FxAtmConvention) -> Self {
        self.atm_convention = Some(convention);
        self
    }

    /// Set the butterfly convention (`FxButterflyConvention::Smile`).
    ///
    /// # Arguments
    ///
    /// * `convention` - Arithmetic smile butterfly quote definition.
    #[must_use]
    pub fn butterfly_convention(mut self, convention: FxButterflyConvention) -> Self {
        self.butterfly_convention = Some(convention);
        self
    }

    /// Set the strictly increasing positive expiry times in years.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Option expiry times in years.
    #[must_use]
    pub fn expiries(mut self, expiries: &[f64]) -> Self {
        self.expiries = Some(expiries.to_vec());
        self
    }

    /// Set the ATM delta-neutral-straddle volatilities (decimal).
    ///
    /// # Arguments
    ///
    /// * `atm_vols` - Strictly positive decimal Black volatilities per expiry.
    #[must_use]
    pub fn atm_vols(mut self, atm_vols: &[f64]) -> Self {
        self.atm_vols = Some(atm_vols.to_vec());
        self
    }

    /// Set the 25-delta risk reversals (`sigma_call - sigma_put`, decimal).
    ///
    /// # Arguments
    ///
    /// * `rr_25d` - Finite 25-delta risk reversals per expiry.
    #[must_use]
    pub fn rr_25d(mut self, rr_25d: &[f64]) -> Self {
        self.rr_25d = Some(rr_25d.to_vec());
        self
    }

    /// Set the 25-delta arithmetic smile butterflies (`0.5 * (sigma_call + sigma_put) - sigma_ATM`, decimal).
    ///
    /// # Arguments
    ///
    /// * `bf_25d` - Finite 25-delta butterflies per expiry.
    #[must_use]
    pub fn bf_25d(mut self, bf_25d: &[f64]) -> Self {
        self.bf_25d = Some(bf_25d.to_vec());
        self
    }

    /// Set the optional 10-delta risk reversals (`sigma_call - sigma_put`, decimal).
    ///
    /// # Arguments
    ///
    /// * `rr_10d` - Finite 10-delta risk reversals per expiry; requires `bf_10d`.
    #[must_use]
    pub fn rr_10d(mut self, rr_10d: &[f64]) -> Self {
        self.rr_10d = Some(rr_10d.to_vec());
        self
    }

    /// Set the optional 10-delta arithmetic smile butterflies (decimal).
    ///
    /// # Arguments
    ///
    /// * `bf_10d` - Finite 10-delta butterflies per expiry; requires `rr_10d`.
    #[must_use]
    pub fn bf_10d(mut self, bf_10d: &[f64]) -> Self {
        self.bf_10d = Some(bf_10d.to_vec());
        self
    }

    /// Attach an optional [`VolatilityClock`] recording the expiry calibration basis.
    ///
    /// # Arguments
    ///
    /// * `clock` - Validated volatility clock for dated expiry lookup.
    #[must_use]
    pub fn expiry_clock(mut self, clock: VolatilityClock) -> Self {
        self.expiry_clock = Some(clock);
        self
    }

    /// Validate all required conventions, arrays, and implied wing volatilities and build [`FxDeltaVolSurface`].
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing or fails structural or wing validation.
    pub fn build(self) -> crate::Result<FxDeltaVolSurface> {
        let missing = |field: &str| {
            crate::Error::Validation(format!("FxDeltaVolSurface builder requires '{field}'"))
        };
        FxDeltaVolSurface::from_params(FxDeltaVolSurfaceParams {
            id: self.id,
            pair: self.pair,
            delta_convention: self
                .delta_convention
                .ok_or_else(|| missing("delta_convention"))?,
            atm_convention: self
                .atm_convention
                .ok_or_else(|| missing("atm_convention"))?,
            butterfly_convention: self
                .butterfly_convention
                .ok_or_else(|| missing("butterfly_convention"))?,
            expiries: self.expiries.ok_or_else(|| missing("expiries"))?,
            atm_vols: self.atm_vols.ok_or_else(|| missing("atm_vols"))?,
            rr_25d: self.rr_25d.ok_or_else(|| missing("rr_25d"))?,
            bf_25d: self.bf_25d.ok_or_else(|| missing("bf_25d"))?,
            rr_10d: self.rr_10d,
            bf_10d: self.bf_10d,
            expiry_clock: self.expiry_clock,
        })
    }
}

/// Delta-quoted FX volatility surface.
///
/// Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
/// 25-delta arithmetic smile butterfly, optional 10-delta wings) together with
/// explicit currency pair, quote profile conventions, and optional [`VolatilityClock`].
/// Models-layer functions perform delta conversion and volatility evaluation.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::surfaces::{
///     FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
/// };
/// use finstack_quant_core::money::fx::CurrencyPair;
///
/// let pair = CurrencyPair::try_new(Currency::EUR, Currency::USD)?;
/// let surface = FxDeltaVolSurface::builder("EURUSD-DELTA-VOL", pair)
///     .delta_convention(FxDeltaConvention::ForwardUnadjusted)
///     .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
///     .butterfly_convention(FxButterflyConvention::Smile)
///     .expiries(&[0.25, 0.5, 1.0])
///     .atm_vols(&[0.08, 0.085, 0.09])
///     .rr_25d(&[0.01, 0.012, 0.015])
///     .bf_25d(&[0.005, 0.006, 0.007])
///     .build()?;
///
/// assert_eq!(surface.expiries().len(), 3);
/// assert!((surface.atm_vols()[0] - 0.08).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "FxDeltaVolSurfaceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "FxDeltaVolSurfaceWire"))]
pub struct FxDeltaVolSurface {
    id: CurveId,
    /// Currency pair (`base/quote`) matching quoted FX forward orientation.
    pair: CurrencyPair,
    /// Delta convention governing wing delta-to-strike conversion.
    delta_convention: FxDeltaConvention,
    /// At-the-money strike convention.
    atm_convention: FxAtmConvention,
    /// Butterfly quote convention.
    butterfly_convention: FxButterflyConvention,
    /// Expiry times in years (strictly increasing, all positive).
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry (call vol - put vol).
    rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterfly per expiry (wing avg - ATM).
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates to `expiries`.
    expiry_clock: Option<VolatilityClock>,
}

/// Raw deserialization state of [`FxDeltaVolSurface`].
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FxDeltaVolSurfaceWire {
    /// Surface identifier.
    id: CurveId,
    /// Currency pair (`base/quote`) matching quoted FX forward orientation.
    pair: CurrencyPair,
    /// Delta convention governing wing delta-to-strike conversion.
    delta_convention: FxDeltaConvention,
    /// At-the-money strike convention.
    atm_convention: FxAtmConvention,
    /// Butterfly quote convention.
    butterfly_convention: FxButterflyConvention,
    /// Expiry times in years.
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry.
    rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterfly per expiry.
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates to `expiries`.
    #[serde(default)]
    expiry_clock: Option<VolatilityClock>,
}

impl TryFrom<FxDeltaVolSurfaceWire> for FxDeltaVolSurface {
    type Error = crate::Error;

    fn try_from(raw: FxDeltaVolSurfaceWire) -> crate::Result<Self> {
        Self::from_params(FxDeltaVolSurfaceParams {
            id: raw.id,
            pair: raw.pair,
            delta_convention: raw.delta_convention,
            atm_convention: raw.atm_convention,
            butterfly_convention: raw.butterfly_convention,
            expiries: raw.expiries,
            atm_vols: raw.atm_vols,
            rr_25d: raw.rr_25d,
            bf_25d: raw.bf_25d,
            rr_10d: raw.rr_10d,
            bf_10d: raw.bf_10d,
            expiry_clock: raw.expiry_clock,
        })
    }
}

impl FxDeltaVolSurface {
    /// Start building an [`FxDeltaVolSurface`] for `id` and `pair`.
    ///
    /// # Arguments
    ///
    /// * `id` - Non-empty unique surface identifier used for lookup in a market context.
    /// * `pair` - Ordered FX pair (`base/quote`) matching the forward rate's quote-per-base units.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>, pair: CurrencyPair) -> FxDeltaVolSurfaceBuilder {
        FxDeltaVolSurfaceBuilder {
            id: id.into(),
            pair,
            delta_convention: None,
            atm_convention: None,
            butterfly_convention: None,
            expiries: None,
            atm_vols: None,
            rr_25d: None,
            bf_25d: None,
            rr_10d: None,
            bf_10d: None,
            expiry_clock: None,
        }
    }

    /// Construct and validate an [`FxDeltaVolSurface`] from explicit [`FxDeltaVolSurfaceParams`].
    ///
    /// # Arguments
    ///
    /// * `params` - Complete FX delta quote profile, expiry/quote vectors, and optional clock.
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is empty, `pair` has identical base and quote currencies,
    /// `rr_10d`/`bf_10d` are unpaired, any quote vector is empty or mismatched in length,
    /// `expiries` are non-positive/non-finite/non-increasing, or any ATM/wing volatility
    /// recovered via [`recover_fx_wing_vols`] is non-positive or non-finite.
    pub fn from_params(params: FxDeltaVolSurfaceParams) -> crate::Result<Self> {
        if params.id.as_str().trim().is_empty() {
            return Err(crate::Error::Validation(
                "FX delta vol surface id cannot be empty".to_string(),
            ));
        }
        let pair = CurrencyPair::try_new(params.pair.base, params.pair.quote)?;
        Self::validate(
            &params.expiries,
            &params.atm_vols,
            &params.rr_25d,
            &params.bf_25d,
            params.rr_10d.as_deref(),
            params.bf_10d.as_deref(),
        )?;
        Ok(Self {
            id: params.id,
            pair,
            delta_convention: params.delta_convention,
            atm_convention: params.atm_convention,
            butterfly_convention: params.butterfly_convention,
            expiries: params.expiries,
            atm_vols: params.atm_vols,
            rr_25d: params.rr_25d,
            bf_25d: params.bf_25d,
            rr_10d: params.rr_10d,
            bf_10d: params.bf_10d,
            expiry_clock: params.expiry_clock,
        })
    }

    /// Surface identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Currency pair (`base/quote`) whose forwards are quoted in `quote` per `base`.
    #[inline]
    pub const fn pair(&self) -> CurrencyPair {
        self.pair
    }

    /// Expiry times (years).
    #[inline]
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Return the ATM delta-neutral-straddle volatility nodes.
    pub fn atm_vols(&self) -> &[f64] {
        &self.atm_vols
    }

    /// Return the 25-delta risk-reversal quote nodes.
    pub fn rr_25d(&self) -> &[f64] {
        &self.rr_25d
    }

    /// Return the 25-delta butterfly quote nodes.
    pub fn bf_25d(&self) -> &[f64] {
        &self.bf_25d
    }

    /// Return the optional 10-delta risk-reversal quote nodes.
    pub fn rr_10d(&self) -> Option<&[f64]> {
        self.rr_10d.as_deref()
    }

    /// Return the optional 10-delta butterfly quote nodes.
    pub fn bf_10d(&self) -> Option<&[f64]> {
        self.bf_10d.as_deref()
    }

    /// Quoting convention of the stored implied volatilities (`black_lognormal`).
    #[inline]
    pub fn quote_type(&self) -> VolQuoteType {
        VolQuoteType::BlackLognormal
    }

    /// Delta convention governing delta-to-strike inversion (`forward_unadjusted`).
    #[inline]
    pub const fn delta_convention(&self) -> FxDeltaConvention {
        self.delta_convention
    }

    /// At-the-money strike convention (`delta_neutral_straddle`).
    #[inline]
    pub const fn atm_convention(&self) -> FxAtmConvention {
        self.atm_convention
    }

    /// Butterfly quote convention (`smile`: wing average minus ATM).
    #[inline]
    pub const fn butterfly_convention(&self) -> FxButterflyConvention {
        self.butterfly_convention
    }

    /// Optional volatility clock recording the date-to-expiry basis.
    #[inline]
    pub const fn expiry_clock(&self) -> Option<&VolatilityClock> {
        self.expiry_clock.as_ref()
    }

    fn validate(
        expiries: &[f64],
        atm_vols: &[f64],
        rr_25d: &[f64],
        bf_25d: &[f64],
        rr_10d: Option<&[f64]>,
        bf_10d: Option<&[f64]>,
    ) -> crate::Result<()> {
        if rr_10d.is_some() != bf_10d.is_some() {
            return Err(crate::Error::Validation(
                "rr_10d and bf_10d must both be provided or both omitted".into(),
            ));
        }
        if expiries.is_empty() || atm_vols.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let n = expiries.len();
        if atm_vols.len() != n || rr_25d.len() != n || bf_25d.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(rr) = rr_10d
            && rr.len() != n
        {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(bf) = bf_10d
            && bf.len() != n
        {
            return Err(InputError::DimensionMismatch.into());
        }

        for &t in expiries {
            if !t.is_finite() || t <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }
        for w in expiries.windows(2) {
            if w[1] <= w[0] {
                return Err(InputError::NonMonotonicKnots.into());
            }
        }

        for &v in atm_vols {
            if !v.is_finite() || v <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }

        for ((&atm, &rr), &bf) in atm_vols.iter().zip(rr_25d.iter()).zip(bf_25d.iter()) {
            recover_fx_wing_vols(atm, rr, bf)?;
        }
        if let (Some(rr10), Some(bf10)) = (rr_10d, bf_10d) {
            for ((&atm, &rr), &bf) in atm_vols.iter().zip(rr10.iter()).zip(bf10.iter()) {
                recover_fx_wing_vols(atm, rr, bf)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_FX_DELTA_REL_TOL, FxAtmConvention, FxButterflyConvention, FxDeltaConvention,
        FxDeltaVolSurface, VolQuoteType, fx_option_delta_to_strike, fx_smile_pillars_checked,
        interpolate_fx_wing_vols, recover_fx_wing_vols,
    };
    use crate::{
        currency::Currency,
        dates::{DayCount, DayCountContextState, VolatilityClock},
        money::fx::CurrencyPair,
    };
    use time::macros::date;

    fn eurusd() -> CurrencyPair {
        CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("distinct pair")
    }

    #[test]
    fn rejects_quotes_with_non_positive_implied_wing_volatilities() {
        let err = FxDeltaVolSurface::builder("EURUSD-VOL", eurusd())
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.25, 1.0])
            .atm_vols(&[0.08, 0.08])
            .rr_25d(&[0.0, 0.0])
            .bf_25d(&[-0.09, -0.02])
            .build()
            .expect_err("negative implied 25d wing vol should be rejected");
        assert!(matches!(
            err,
            crate::Error::Input(crate::InputError::NonPositiveValue)
        ));
    }

    #[test]
    fn exposes_explicit_quote_delta_atm_butterfly_pair_and_clock_conventions() {
        let clock = VolatilityClock::try_new(
            date!(2025 - 01 - 15),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .expect("clock");
        let surface = FxDeltaVolSurface::builder("EURUSD-VOL", eurusd())
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.25, 1.0])
            .atm_vols(&[0.08, 0.09])
            .rr_25d(&[0.01, 0.015])
            .bf_25d(&[0.005, 0.007])
            .expiry_clock(clock.clone())
            .build()
            .expect("valid FX delta vol surface");

        assert_eq!(surface.pair(), eurusd());
        assert_eq!(surface.expiry_clock(), Some(&clock));
        assert_eq!(surface.quote_type(), VolQuoteType::BlackLognormal);
        assert_eq!(
            surface.delta_convention(),
            FxDeltaConvention::ForwardUnadjusted
        );
        assert_eq!(
            surface.atm_convention(),
            FxAtmConvention::DeltaNeutralStraddle
        );
        assert_eq!(surface.butterfly_convention(), FxButterflyConvention::Smile);
        assert_eq!(surface.delta_convention().to_string(), "forward_unadjusted");
        assert_eq!(
            surface.atm_convention().to_string(),
            "delta_neutral_straddle"
        );
        assert_eq!(surface.butterfly_convention().to_string(), "smile");
    }

    #[test]
    fn checked_wing_recovery_and_interpolation_handle_cancellation_and_subnormals() {
        // Cancellation: ATM=1, BF=1e-20, RR=-2 -> call wing ~1e-20, put wing ~2+1e-20.
        let (put, call) = recover_fx_wing_vols(1.0, -2.0, 1e-20).expect("valid cancelled wing");
        assert!((put - 2.0).abs() < 1e-12);
        assert!((call - 1e-20).abs() < 1e-34);

        // Minimum positive subnormal: put = 0.5 * min_subnormal rounds to 0 -> error.
        let min_sub = f64::from_bits(1);
        assert!(recover_fx_wing_vols(min_sub, min_sub, 0.0).is_err());

        // Single-rounding interpolation across endpoints with midpoint 5 * 2^-55.
        let (_, _, call_mid) = interpolate_fx_wing_vols(
            0.5,
            (1.0, -2.5, 0.25_f64.next_up()),
            0.5,
            (1.0_f64.next_up(), -2.5, 0.25),
        )
        .expect("single-rounded wing interpolation");
        let expected = 5.0 * f64::from_bits((1023 - 55) << 52);
        assert_eq!(call_mid, expected);
    }

    #[test]
    fn checked_delta_inversion_handles_extreme_tails_scaling_and_tol() {
        // Tiny tail put delta 1e-20.
        let k_tail =
            fx_option_delta_to_strike(false, 1e-20, 1.0, 0.2, 1.0, DEFAULT_FX_DELTA_REL_TOL)
                .expect("tiny tail strike");
        assert!((k_tail - 0.160_018_15).abs() < 1e-6, "got {k_tail}");

        // Extreme forward * exp(w^2 / 2) where exp(800) alone overflows binary64.
        let k_scaled =
            fx_option_delta_to_strike(true, 0.5, 1e-300, 40.0, 1.0, DEFAULT_FX_DELTA_REL_TOL)
                .expect("scaled DNS-like half-delta strike");
        assert!((k_scaled / 2.726_374_572_1e47 - 1.0).abs() < 1e-9);

        // Extremely small vol/time rounds K -> 1.0 so call delta snaps to 0.5 and fails 1e-3 check.
        assert!(
            fx_option_delta_to_strike(true, 0.25, 1.0, 1e-17, 1.0, DEFAULT_FX_DELTA_REL_TOL)
                .is_err()
        );
    }

    #[test]
    fn strike_ordering_rejects_crossed_pillars() {
        let (put, call) = recover_fx_wing_vols(0.2, -2.8, 1.4).expect("positive wings");
        assert!(
            fx_smile_pillars_checked(1.0, 1.0, 0.2, (put, call), None, DEFAULT_FX_DELTA_REL_TOL)
                .is_err()
        );
    }
}
