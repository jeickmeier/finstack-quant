"use client";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { RollupSpecWire } from "@/lib/finstack/generated/types/rollup_spec";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";

/** Native RollupMethod: flows sum, balances take the period end, ratios re-run their formula. */
export type RollupMethod = NonNullable<RollupSpecWire["methods"]>[string];
const methodLabels: Record<RollupMethod, string> = {
  sum: "Sum (flow)",
  period_end: "Period end (balance)",
  formula: "Recalculate formula (ratio)",
};

/** The methods of a host-supplied RollupSpec. Throws on JSON that does not parse. */
export function hostRollupMethods(
  specJson: string,
): Record<string, RollupMethod> {
  return { ...(JSON.parse(specJson) as RollupSpecWire).methods };
}

/** Native annual RollupSpec JSON; a listed `sum` overrides the line's declared method. */
export function annualRollupSpecJson(
  methods: Record<string, RollupMethod>,
): string {
  const spec: RollupSpecWire = { frequency: "annual", methods };
  return JSON.stringify(spec);
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
    <details>
      <summary>Annual roll-up methods</summary>
      <p className="mt-2 text-xs text-muted-foreground">
        Flows sum across the year, balances and trailing metrics take the
        year-end value, and ratios are recalculated from the annual figures.
        WASM rolls the values up; only complete years are shown.
      </p>
      <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {nodeIds.map((id) => {
          const node = model.nodes[id];
          return (
            <EnumField
              key={id}
              label={`${node?.name ?? id} roll-up method`}
              layout="select"
              value={methods[id] ?? node?.rollup ?? "sum"}
              onValueChange={(next) =>
                onChange({ ...methods, [id]: next as RollupMethod })
              }
              options={(Object.keys(methodLabels) as RollupMethod[])
                .filter((method) => method !== "formula" || node?.formula_text)
                .map((method) => ({
                  value: method,
                  label: methodLabels[method],
                }))}
            />
          );
        })}
      </div>
    </details>
  );
}
