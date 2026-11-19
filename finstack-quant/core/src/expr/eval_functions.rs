//! Checked, row-wise scalar function evaluation for the core expression DAG.

use super::{CompiledExpr, Function};

impl CompiledExpr {
    /// Argument counts and round's integral literal control were checked at compilation.
    pub(super) fn eval_function_into(
        node: u64,
        fun: Function,
        args: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        for (row, output) in out.iter_mut().enumerate() {
            let x = args[0][row];
            if fun == Function::IsMissing {
                *output = f64::from(x.is_nan());
                continue;
            }
            if args.iter().any(|column| column[row].is_nan()) {
                *output = f64::NAN;
                continue;
            }
            let error = |message: String| crate::Error::ExpressionEvaluation { node, row, message };
            let value = match fun {
                Function::Abs => x.abs(),
                Function::Sign if x == 0.0 => 0.0,
                Function::Sign => x.signum(),
                Function::Floor => x.floor(),
                Function::Ceil => x.ceil(),
                Function::Ln => x.ln(),
                Function::Exp => x.exp(),
                Function::Log10 => x.log10(),
                Function::Sqrt => x.sqrt(),
                Function::Pow => x.powf(args[1][row]),
                Function::Round => {
                    let digits = args.get(1).map_or(0, |d| d[0] as i32);
                    crate::math::round_half_away(x, digits).map_err(|e| error(e.to_string()))?
                }
                Function::Clamp => crate::math::clamp(x, args[1][row], args[2][row])
                    .map_err(|e| error(e.to_string()))?,
                _ => return Err(error(format!("unsupported scalar function {fun}"))),
            };
            if !value.is_finite() {
                return Err(error(format!(
                    "{fun} produced a nonfinite value from finite arguments"
                )));
            }
            *output = value;
        }
        Ok(())
    }
}
