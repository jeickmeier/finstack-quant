//! Frozen pre-CCM-I5 LMM arithmetic from revision 827ebb75.
//!
//! This test-only reference preserves `process/lmm.rs::compute_drift` and
//! `discretization/lmm_predictor_corrector.rs::step` before reassociation.
//! It deliberately does not call either production implementation.

use finstack_quant_models::monte_carlo::process::lmm::LmmParams;

pub(crate) fn first_alive(params: &LmmParams, t: f64) -> usize {
    params.tenors[..params.num_forwards].partition_point(|&tenor| tenor <= t)
}

pub(crate) fn loading(params: &LmmParams, i: usize, t: f64) -> &[f64; 3] {
    let period = match params
        .vol_times
        .binary_search_by(|v| v.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
    {
        Ok(i) => i + 1,
        Err(i) => i,
    };
    &params.vol_values[period][i]
}

pub(crate) fn step_is_aligned(params: &LmmParams, t: f64, dt: f64) -> bool {
    let end = t + dt;
    let tolerance = 32.0 * f64::EPSILON * end.abs().max(t.abs()).max(1.0);
    [&params.vol_times[..], &params.tenors[..params.num_forwards]]
        .into_iter()
        .all(|knots| {
            let index = knots.partition_point(|&knot| knot <= t);
            knots.get(index).is_none_or(|&knot| knot >= end - tolerance)
        })
}

pub(crate) fn drift(params: &LmmParams, t: f64, x: &[f64], out: &mut [f64]) {
    let n = params.num_forwards;
    let nf = params.num_factors;
    let first = first_alive(params, t);
    out[..n].fill(0.0);
    for i in first..n {
        let lam_i = loading(params, i, t);
        let d_i_shifted = x[i] + params.displacements[i];
        let mut mu = 0.0;
        for (j, &f_j) in x[..n].iter().enumerate().skip(i + 1) {
            let tau_j = params.accrual_factors[j];
            let d_j = params.displacements[j];
            let denom = 1.0 + tau_j * f_j;
            if denom.abs() < 1e-15 {
                continue;
            }
            let lam_j = loading(params, j, t);
            let dot: f64 = lam_i
                .iter()
                .zip(lam_j.iter())
                .take(nf)
                .map(|(a, b)| a * b)
                .sum();
            mu -= tau_j * (f_j + d_j) / denom * dot;
        }
        out[i] = mu * d_i_shifted;
    }
}

pub(crate) fn contribution_scale(params: &LmmParams, t: f64, x: &[f64], i: usize) -> f64 {
    if i < first_alive(params, t) {
        return 0.0;
    }
    let lam_i = loading(params, i, t);
    let mut sum = 0.0;
    for (j, &x_j) in x[..params.num_forwards].iter().enumerate().skip(i + 1) {
        let tau = params.accrual_factors[j];
        let denominator = 1.0 + tau * x_j;
        if denominator.abs() < 1e-15 {
            continue;
        }
        let weight = (tau * (x_j + params.displacements[j]) / denominator).abs();
        let lam_j = loading(params, j, t);
        for factor in 0..params.num_factors {
            sum += weight * (lam_i[factor] * lam_j[factor]).abs();
        }
    }
    sum * (x[i] + params.displacements[i]).abs()
}

pub(crate) fn step(
    params: &LmmParams,
    t: f64,
    dt: f64,
    x: &mut [f64],
    z: &[f64],
    work: &mut [f64],
) {
    if !step_is_aligned(params, t, dt) {
        x.fill(f64::NAN);
        return;
    }
    let n = params.num_forwards;
    let nf = params.num_factors;
    let sqrt_dt = dt.sqrt();
    let first = first_alive(params, t);
    let (drift_curr, rest) = work.split_at_mut(n);
    drift(params, t, x, drift_curr);
    let (predicted, rest2) = rest.split_at_mut(n);
    predicted.copy_from_slice(x);
    for i in first..n {
        let x_disp = x[i] + params.displacements[i];
        if x_disp <= 0.0 {
            continue;
        }
        let lam = loading(params, i, t);
        let mut diff_sum = 0.0;
        let mut lam_sq = 0.0;
        for k in 0..nf {
            diff_sum += lam[k] * z[k];
            lam_sq += lam[k] * lam[k];
        }
        let m_curr = drift_curr[i] / x_disp;
        predicted[i] = x_disp * ((m_curr - 0.5 * lam_sq) * dt + diff_sum * sqrt_dt).exp()
            - params.displacements[i];
    }
    let drift_pred = &mut rest2[..n];
    drift(params, t, predicted, drift_pred);
    for i in first..n {
        let x_disp = x[i] + params.displacements[i];
        if x_disp <= 0.0 {
            continue;
        }
        let lam = loading(params, i, t);
        let mut diff_sum = 0.0;
        let mut lam_sq = 0.0;
        for k in 0..nf {
            diff_sum += lam[k] * z[k];
            lam_sq += lam[k] * lam[k];
        }
        let m_curr = drift_curr[i] / x_disp;
        let pred_disp = predicted[i] + params.displacements[i];
        let m_pred = if pred_disp > 0.0 {
            drift_pred[i] / pred_disp
        } else {
            0.0
        };
        let avg_m = 0.5 * (m_curr + m_pred);
        x[i] = x_disp * ((avg_m - 0.5 * lam_sq) * dt + diff_sum * sqrt_dt).exp()
            - params.displacements[i];
    }
}
