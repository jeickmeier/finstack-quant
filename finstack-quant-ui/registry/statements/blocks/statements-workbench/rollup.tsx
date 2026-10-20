"use client";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { StatementPeriod } from "../../components/statement-grid/projection";

/** Native RollupMethod: flows sum, balances take the period end, ratios re-run their formula. */
export type RollupMethod = "sum" | "period_end" | "formula";
const methodLabels: Record<RollupMethod, string> = {
  sum: "Sum (flow)",
  period_end: "Period end (balance)",
  formula: "Recalculate formula (ratio)",
};

/**
 * Starting methods for a rollup: the host's RollupSpec methods when supplied,
 * otherwise balance-sheet lines take the period end and ratio or metric lines
 * with a formula are recalculated. Unlisted lines sum natively.
 */
export function initialRollupMethods(
  model: FinancialModelSpecWire,
  defaultSpecJson?: string,
): Record<string, RollupMethod> {
  if (defaultSpecJson !== undefined) {
    const spec = JSON.parse(defaultSpecJson) as {
      methods?: Record<string, RollupMethod>;
    };
    return { ...spec.methods };
  }
  const methods: Record<string, RollupMethod> = {};
  for (const [id, node] of Object.entries(model.nodes)) {
    const tags = node.tags ?? [];
    if (tags.includes("balance_sheet")) methods[id] = "period_end";
    else if (
      node.formula_text &&
      tags.some((tag) => tag === "ratio" || tag === "metric")
    )
      methods[id] = "formula";
  }
  return methods;
}

/** Native annual RollupSpec JSON; lines not listed sum. */
export function annualRollupSpecJson(
  methods: Record<string, RollupMethod>,
): string {
  const listed = Object.fromEntries(
    Object.entries(methods).filter(([, method]) => method !== "sum"),
  );
  return JSON.stringify({ frequency: "annual", methods: listed });
}

/**
 * Columns for a natively rolled-up result: its target periods in returned
 * order, spanning their source periods and historical only when every source
 * period is.
 */
export function rolledPeriods(
  model: FinancialModelSpecWire,
  rolled: { nodes: Record<string, Record<string, unknown>> },
): StatementPeriod[] {
  const ids: string[] = [];
  for (const byPeriod of Object.values(rolled.nodes))
    for (const id of Object.keys(byPeriod)) if (!ids.includes(id)) ids.push(id);
  return ids.map((id) => {
    const sources = model.periods.filter(
      (period) =>
        period.id.startsWith(id) && /^[A-Z]/.test(period.id.slice(id.length)),
    );
    return {
      id,
      start: sources[0]?.start ?? "",
      end: sources.at(-1)?.end ?? "",
      is_actual:
        sources.length > 0 && sources.every((period) => period.is_actual),
    };
  });
}

/** Per-line rollup method choices; the formula option needs a formula line. */
export function RollupMethodsEditor({
  model,
  nodeIds,
  methods,
  onChange,
}: {
  model: FinancialModelSpecWire;
  nodeIds: string[];
  methods: Record<string, RollupMethod>;
  onChange: (methods: Record<string, RollupMethod>) => void;
}) {
  return (
    <details className="rounded-lg border border-border bg-card p-4">
      <summary className="cursor-pointer font-semibold">
        Annual roll-up methods
      </summary>
      <p className="mt-2 text-xs text-muted-foreground">
        Flows sum across the year, balances and trailing metrics take the
        year-end value, and ratios are recalculated from the annual figures.
        WASM rolls the values up; only complete years are shown.
      </p>
      <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {nodeIds.map((id) => {
          const node = model.nodes[id];
          return (
            <label key={id} className="text-xs">
              {node?.name ?? id}
              <select
                aria-label={`${node?.name ?? id} roll-up method`}
                className="mt-1 w-full rounded border border-border bg-background px-2 py-1.5 text-sm"
                value={methods[id] ?? "sum"}
                onChange={(event) =>
                  onChange({
                    ...methods,
                    [id]: event.target.value as RollupMethod,
                  })
                }
              >
                {(Object.keys(methodLabels) as RollupMethod[])
                  .filter(
                    (method) => method !== "formula" || node?.formula_text,
                  )
                  .map((method) => (
                    <option key={method} value={method}>
                      {methodLabels[method]}
                    </option>
                  ))}
              </select>
            </label>
          );
        })}
      </div>
    </details>
  );
}
