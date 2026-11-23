//! Shewchuk exact floating-point expansion arithmetic for FX wing recovery and strike scaling.

use crate::error::InputError;
use crate::math::stats::{frexp, frexp_exp, ldexp};

const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fee0_0000);
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;

#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bv = s - a;
    let e = (a - (s - bv)) + (b - bv);
    (s, e)
}

#[inline]
pub(super) fn two_prod(a: f64, b: f64) -> (f64, f64) {
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

/// Evaluate exact sum of `terms`, then multiply by `2^out_pow2` with single IEEE-754 rounding.
pub(super) fn exact_sum_scaled_to_f64(terms: &[f64], out_pow2: i32) -> f64 {
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
        return ldexp(e[0], out_pow2);
    }
    let hi = e[len - 1];
    let mut tail = 0.0_f64;
    for &x in &e[..len - 1] {
        let (s, err) = two_sum(tail, x);
        tail = odd_round(s, err);
    }

    let hi_abs = hi.abs();
    let hi_exp = frexp_exp(hi_abs);
    // Target exponent of MSB (where `1.0 * 2^e` corresponds to true_exp = hi_exp - 1 + out_pow2).
    let true_msb_exp = hi_exp - 1 + out_pow2;
    if true_msb_exp >= -1022 {
        let (rounded_hi, _) = two_sum(hi, tail);
        ldexp(rounded_hi, out_pow2)
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

pub(super) fn scaled_exp_product(forward: f64, log_ratio: f64) -> crate::Result<f64> {
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
    let (m, e_f) = frexp(forward);
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
    let strike = ldexp(base, total_exp);
    if !strike.is_finite() || strike <= 0.0 {
        return Err(crate::Error::Validation(
            "FX strike is not representable as a finite positive binary64 value".to_string(),
        ));
    }
    Ok(strike)
}
