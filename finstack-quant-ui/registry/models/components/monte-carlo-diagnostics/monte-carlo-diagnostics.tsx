"use client";
import type { MonteCarloValuationDetails } from "finstack-quant-wasm";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";
/** Published native diagnostics; counts, flags, seed and uncertainty remain unmodified. */
export function MonteCarloDiagnostics({
  data,
  currency,
  density = "compact",
}: {
  data: MonteCarloValuationDetails;
  currency: string;
  density?: "compact" | "comfortable";
}) {
  const values = [
    ["Model", data.model_key],
    [`Standard error (${currency})`, String(data.standard_error)],
    ["Training paths", String(data.training_paths)],
    ["Training simulated paths", String(data.training_simulated_paths)],
    ["Make-whole training paths", String(data.make_whole_training_paths)],
    [
      "Make-whole training simulated paths",
      String(data.make_whole_training_simulated_paths),
    ],
    ["Estimator paths", String(data.estimator_paths)],
    ["Simulated paths", String(data.simulated_paths)],
    ["Seed", data.seed.toString()],
    ["Antithetic", String(data.antithetic)],
    ["Sobol", String(data.sobol)],
    ["Brownian bridge", String(data.brownian_bridge)],
  ];
  return (
    <section aria-label="Monte Carlo diagnostics" className="space-y-3">
      <h2 className="text-sm font-semibold">Monte Carlo diagnostics</h2>
      <dl className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-x-3 gap-y-1 text-sm">
        {values.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="break-all font-mono">{value}</dd>
          </div>
        ))}
      </dl>
      <p className="text-xs text-muted-foreground">
        For LSMC valuations, standard error measures pricing-path sampling
        uncertainty under the frozen fitted exercise policy. It excludes
        regression approximation, time-grid discretization, and model error.
      </p>
      <details>
        <summary className="text-sm">Time grid (year fractions)</summary>
        <JsonViewer
          label="Monte Carlo time grid"
          density={density}
          text={serializeHost(data.time_grid)}
        />
      </details>
    </section>
  );
}
