//! Numerically stable logarithms of positive ratios.

use super::stats;

#[inline]
fn frexp_f64(x: f64) -> (f64, i32) {
    let exp = stats::frexp_exp(x);
    (stats::ldexp(x, -exp), exp)
}

#[inline]
fn diff_of_products(a: f64, b: f64, c: f64, d: f64) -> f64 {
    let cd = c * d;
    let err = (-c).mul_add(d, cd);
    let dop = a.mul_add(b, -cd);
    dop + err
}

/// Evaluate `ln((a * b) / (c * d))` for strictly positive finite inputs with `frexp` scaling and FMA.
pub(crate) fn checked_log_cross_ratio(a: f64, b: f64, c: f64, d: f64) -> f64 {
    let (ma, ea) = frexp_f64(a);
    let (mb, eb) = frexp_f64(b);
    let (mc, ec) = frexp_f64(c);
    let (md, ed) = frexp_f64(d);
    let exp_diff = (ea + eb) - (ec + ed);

    if (-2..=2).contains(&exp_diff) {
        let a_scaled = stats::ldexp(ma, exp_diff);
        let num = a_scaled * mb;
        let den = mc * md;
        let ratio = num / den;
        if (0.5..=2.0).contains(&ratio) {
            let diff = diff_of_products(a_scaled, mb, mc, md);
            return (diff / den).ln_1p();
        }
    }

    ((ma * mb) / (mc * md)).ln() + f64::from(exp_diff) * std::f64::consts::LN_2
}

/// Evaluate `ln(num / den)` stably for finite strictly positive `num` and `den`.
pub(crate) fn stable_positive_log_ratio(num: f64, den: f64) -> crate::Result<f64> {
    if !num.is_finite() || num <= 0.0 || !den.is_finite() || den <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "positive-factor log ratio requires finite positive values, got {num} / {den}"
        )));
    }
    if num.to_bits() == den.to_bits() {
        return Ok(0.0);
    }
    let ratio = checked_log_cross_ratio(num, 1.0, den, 1.0);
    if !ratio.is_finite() {
        return Err(crate::Error::Validation(format!(
            "positive-factor log ratio overflowed f64 for {num} / {den}"
        )));
    }
    Ok(if ratio == 0.0 { 0.0 } else { ratio })
}
