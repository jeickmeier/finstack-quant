"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";

type Node = FinancialModelSpecWire["nodes"][string];
export type ForecastSpecWire = NonNullable<Node["forecast"]>;
type Method = ForecastSpecWire["method"];
type Field = {
  key: string;
  label: string;
  kind?: "integer" | "array" | "select";
  options?: string[];
  required?: boolean;
  hint?: string;
};
const methods: { id: Method; label: string; help: string }[] = [
  {
    id: "forward_fill",
    label: "Carry forward",
    help: "Repeat the last reported value.",
  },
  {
    id: "growth_pct",
    label: "Fixed growth",
    help: "Compound by one decimal growth rate per period.",
  },
  {
    id: "curve_pct",
    label: "Growth by period",
    help: "Set a different decimal growth rate for each forecast period.",
  },
  {
    id: "override",
    label: "Scheduled values",
    help: "Set exact levels for selected periods; carry forward between them.",
  },
  {
    id: "fade_to_target",
    label: "Fade to target",
    help: "Move toward a long-run level over the forecast horizon.",
  },
  {
    id: "time_series",
    label: "Time series",
    help: "Project from supplied historical observations.",
  },
  {
    id: "seasonal",
    label: "Seasonal",
    help: "Project a repeating pattern from historical observations.",
  },
  {
    id: "normal",
    label: "Normal",
    help: "Seeded additive random-walk forecast.",
  },
  {
    id: "log_normal",
    label: "Log-normal",
    help: "Seeded multiplicative forecast for positive values.",
  },
  {
    id: "mean_reverting",
    label: "Mean reverting",
    help: "Seeded return toward a long-run mean.",
  },
  {
    id: "bootstrap",
    label: "Bootstrap",
    help: "Resample changes from supplied historical observations.",
  },
];
const field = (
  key: string,
  label: string,
  required = false,
  hint?: string,
): Field => ({ key, label, required, hint });
function fieldsFor(method: Method, params: Record<string, unknown>): Field[] {
  switch (method) {
    case "forward_fill":
      return [];
    case "growth_pct":
      return [field("rate", "Growth per period", true, "Decimal: 0.03 = 3%")];
    case "curve_pct":
      return [];
    case "override":
      return [];
    case "fade_to_target":
      return [
        field("target", "Terminal target", true, "In the line's base units"),
        {
          key: "shape",
          label: "Fade shape",
          kind: "select",
          options: ["linear", "geometric", "exponential"],
        },
        ...(params.shape === "exponential"
          ? [field("half_life", "Half-life (periods)", true)]
          : []),
      ];
    case "normal":
    case "log_normal":
      return [
        field("mean", "Mean per period", true),
        field("std_dev", "Standard deviation", true),
        { key: "seed", label: "Random seed", kind: "integer", required: true },
        field("correlation_with", "Correlation peer (Monte Carlo only)"),
        field("correlation", "Correlation (Monte Carlo only)"),
      ];
    case "mean_reverting":
      return [
        field("long_run_mean", "Long-run mean", true),
        field("reversion_speed", "Reversion speed", true, "Decimal in (0, 1]"),
        field("std_dev", "Standard deviation", true),
        { key: "seed", label: "Random seed", kind: "integer", required: true },
        field("correlation_with", "Correlation peer (Monte Carlo only)"),
        field("correlation", "Correlation (Monte Carlo only)"),
      ];
    case "bootstrap":
      return [
        {
          key: "historical",
          label: "Historical observations",
          kind: "array",
          required: true,
          hint: "Comma-separated exact values, oldest first",
        },
        {
          key: "mode",
          label: "Resample",
          kind: "select",
          options: ["growth", "diff"],
        },
        { key: "seed", label: "Random seed", kind: "integer", required: true },
      ];
    case "time_series":
      return [
        {
          key: "historical",
          label: "Historical observations",
          kind: "array",
          required: true,
          hint: "Comma-separated exact values, oldest first",
        },
        {
          key: "method",
          label: "Trend method",
          kind: "select",
          options: ["linear", "exponential", "moving_average"],
        },
        ...(params.method === "exponential"
          ? [
              field("alpha", "Level smoothing", true),
              field("beta", "Trend smoothing", true),
              field("phi", "Trend damping"),
            ]
          : []),
        ...(params.method === "moving_average"
          ? [
              {
                key: "window",
                label: "Moving-average window",
                kind: "integer" as const,
                required: true,
              },
            ]
          : []),
      ];
    case "seasonal":
      return [
        {
          key: "historical",
          label: "Historical observations",
          kind: "array",
          required: true,
          hint: "At least two full seasonal cycles",
        },
        {
          key: "season_length",
          label: "Periods per cycle",
          kind: "integer",
          required: true,
        },
        {
          key: "mode",
          label: "Seasonality",
          kind: "select",
          options: ["additive", "multiplicative"],
          required: true,
        },
        field("growth", "Trend growth per period", false, "Decimal: 0.03 = 3%"),
      ];
  }
}
function initialDraft(
  method: Method,
  params: Record<string, unknown>,
  periods: string[],
) {
  const values: Record<string, string> = {};
  for (const [key, value] of Object.entries(params))
    values[key] = Array.isArray(value)
      ? value.join(", ")
      : typeof value === "object"
        ? ""
        : String(value);
  if (method === "curve_pct")
    periods.forEach((id, index) => {
      values[id] = String(
        (params.curve as unknown[] | undefined)?.[index] ?? 0,
      );
    });
  if (method === "override")
    periods.forEach((id) => {
      values[id] = String(
        (params.overrides as Record<string, unknown> | undefined)?.[id] ?? "",
      );
    });
  if (method === "growth_pct" && values.rate === undefined) values.rate = "0";
  if (method === "fade_to_target" && values.shape === undefined)
    values.shape = "linear";
  if (method === "time_series" && values.method === undefined)
    values.method = "linear";
  if (method === "seasonal" && values.mode === undefined)
    values.mode = "additive";
  if (method === "bootstrap" && values.mode === undefined)
    values.mode = "growth";
  return values;
}

/** Configure the native forecast method and its parameters; WASM validates and evaluates the result. */
export function ForecastEditor({
  model,
  nodeId,
  node,
  onApply,
  applyLabel = "Apply forecast",
}: {
  model: FinancialModelSpecWire;
  nodeId: string;
  /** The line whose forecast seeds the editor. */
  node: Node;
  /** Receives the complete forecast spec; return an error message or null when accepted. */
  onApply: (forecast: ForecastSpecWire) => Promise<string | null>;
  applyLabel?: string;
}) {
  const periods = model.periods
    .filter((period) => !period.is_actual)
    .map((period) => period.id);
  const [method, setMethod] = useState<Method>(
    node.forecast?.method ?? "forward_fill",
  );
  const [draft, setDraft] = useState<Record<string, string>>(() =>
    initialDraft(
      node.forecast?.method ?? "forward_fill",
      node.forecast?.params ?? {},
      periods,
    ),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const selected = methods.find((item) => item.id === method)!;
  const fieldList = fieldsFor(method, draft);
  const changeMethod = (nextMethod: Method) => {
    setMethod(nextMethod);
    setDraft(
      initialDraft(
        nextMethod,
        nextMethod === node.forecast?.method
          ? (node.forecast.params ?? {})
          : {},
        periods,
      ),
    );
    setError(null);
  };
  const apply = async () => {
    const params: Record<string, unknown> = {};
    const numeric = (key: string, value: string, integer = false): number => {
      const result = Number(value);
      if (
        !value.trim() ||
        !Number.isFinite(result) ||
        (integer && (!Number.isInteger(result) || result < 0))
      )
        throw new Error(
          `${key} must be a ${integer ? "non-negative integer" : "finite decimal number"}.`,
        );
      return result;
    };
    try {
      if (method === "curve_pct")
        params.curve = periods.map((id) => numeric(id, draft[id] ?? ""));
      if (method === "override")
        params.overrides = Object.fromEntries(
          periods
            .filter((id) => draft[id]?.trim())
            .map((id) => [id, numeric(id, draft[id])]),
        );
      for (const item of fieldList) {
        const value = draft[item.key]?.trim() ?? "";
        if (!value) {
          if (item.required) throw new Error(`${item.label} is required.`);
          continue;
        }
        if (item.kind === "select" || item.key === "correlation_with")
          params[item.key] = value;
        else if (item.kind === "array")
          params[item.key] = value
            .split(",")
            .map((part) => numeric(item.key, part.trim()));
        else
          params[item.key] = numeric(item.key, value, item.kind === "integer");
      }
      for (const key of ["min", "max"])
        if (draft[key]?.trim()) params[key] = numeric(key, draft[key]);
      setSaving(true);
      const message = await onApply({ method, params });
      setSaving(false);
      setError(message);
    } catch (cause) {
      setSaving(false);
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const history = model.periods
    .filter((period) => period.is_actual)
    .flatMap((period) => {
      const value = node.values?.[period.id];
      return value === undefined
        ? []
        : [typeof value === "number" ? String(value) : value.amount];
    });
  const set = (key: string, value: string) =>
    setDraft({ ...draft, [key]: value });
  return (
    <section
      aria-label={`${node.name ?? nodeId} forecast`}
      className="rounded-lg border border-border bg-card p-4"
    >
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_15rem]">
        <div>
          <h4 className="font-semibold">{node.name ?? nodeId}</h4>
          <p className="text-xs text-muted-foreground">{selected.help}</p>
        </div>
        <EnumField
          label={`${node.name ?? nodeId} forecast method`}
          layout="select"
          value={method}
          onValueChange={(next) => changeMethod(next as Method)}
          options={methods.map((item) => ({
            value: item.id,
            label: item.label,
            description: item.help,
          }))}
        />
      </div>
      <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {method === "curve_pct" &&
          periods.map((id) => (
            <FieldFrame key={id} label={`${id} growth rate`}>
              {(control) => (
                <Input
                  {...control}
                  inputMode="decimal"
                  className="font-mono"
                  value={draft[id] ?? ""}
                  onChange={(event) => set(id, event.target.value)}
                />
              )}
            </FieldFrame>
          ))}
        {method === "override" &&
          periods.map((id) => (
            <FieldFrame key={id} label={`${id} scheduled value`}>
              {(control) => (
                <Input
                  {...control}
                  inputMode="decimal"
                  className="font-mono"
                  placeholder="Carry forward"
                  value={draft[id] ?? ""}
                  onChange={(event) => set(id, event.target.value)}
                />
              )}
            </FieldFrame>
          ))}
        {fieldList.map((item) => {
          const value = draft[item.key] ?? "";
          if (item.kind === "select")
            return (
              <EnumField
                key={item.key}
                label={item.label}
                value={value}
                onValueChange={(next) => set(item.key, next)}
                options={(item.options ?? []).map((option) => ({
                  value: option,
                  label: option.replaceAll("_", " "),
                }))}
              />
            );
          if (item.key === "correlation_with")
            return (
              <IdCombobox
                key={item.key}
                label={item.label}
                value={value}
                onValueChange={(next) => set(item.key, next)}
                options={Object.keys(model.nodes).filter((id) => id !== nodeId)}
              />
            );
          return (
            <div key={item.key} className="space-y-1">
              <FieldFrame label={item.label} help={item.hint}>
                {(control) => (
                  <Input
                    {...control}
                    inputMode={
                      item.kind === "array"
                        ? undefined
                        : item.kind === "integer"
                          ? "numeric"
                          : "decimal"
                    }
                    className="font-mono"
                    value={value}
                    onChange={(event) => set(item.key, event.target.value)}
                  />
                )}
              </FieldFrame>
              {item.key === "historical" && history.length > 0 && (
                <Button
                  type="button"
                  variant="link"
                  size="xs"
                  onClick={() => set("historical", history.join(", "))}
                >
                  Use reported history
                </Button>
              )}
            </div>
          );
        })}
        {(["min", "max"] as const).map((key) => (
          <FieldFrame
            key={key}
            label={`${key === "min" ? "Minimum" : "Maximum"} (optional)`}
          >
            {(control) => (
              <Input
                {...control}
                inputMode="decimal"
                className="font-mono"
                value={draft[key] ?? ""}
                onChange={(event) => set(key, event.target.value)}
              />
            )}
          </FieldFrame>
        ))}
      </div>
      {error && (
        <p role="alert" className="mt-2 text-xs text-error">
          {error}
        </p>
      )}
      <Button
        type="button"
        size="sm"
        className="mt-4"
        disabled={saving}
        onClick={() => void apply()}
      >
        {saving ? "Evaluating…" : applyLabel}
      </Button>
    </section>
  );
}
