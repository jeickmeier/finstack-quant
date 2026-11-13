"use client";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  SchemaForm,
  type SchemaFormApi,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { forecastParamsModule } from "./forecast-parameters";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";

type Node = FinancialModelSpecWire["nodes"][string];
export type ForecastSpecWire = NonNullable<Node["forecast"]>;
type Method = ForecastSpecWire["method"];
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
/** Editable parameters to start from; a method's own stored values, else empty. */
function startingParams(
  method: Method,
  stored: Record<string, unknown>,
  periods: string[],
): Record<string, unknown> {
  if (method !== "curve_pct") return stored;
  // One editable rate per forecast period; the entered values are what Rust evaluates.
  const curve = stored.curve as unknown[] | undefined;
  return { ...stored, curve: periods.map((_, index) => curve?.[index] ?? 0) };
}
/** Comma-separated exact values, kept as typed until the form validates them. */
function HistoricalField({
  form,
  path,
  label,
  help,
  value,
  history,
}: {
  form: SchemaFormApi;
  path: string;
  label: string;
  help?: string;
  value: unknown;
  history: readonly string[];
}) {
  const [text, setText] = useState(
    Array.isArray(value) ? value.join(", ") : "",
  );
  const set = (next: string) => {
    setText(next);
    form.setFieldValue(
      path,
      next
        .split(",")
        .map((part) => part.trim())
        .filter(Boolean),
    );
  };
  return (
    <div className="space-y-1">
      <FieldFrame label={label} help={help}>
        {(control) => (
          <Input
            {...control}
            className="font-mono"
            value={text}
            onChange={(event) => set(event.target.value)}
          />
        )}
      </FieldFrame>
      {history.length > 0 && (
        <Button
          type="button"
          variant="link"
          size="xs"
          onClick={() => set(history.join(", "))}
        >
          Use reported history
        </Button>
      )}
    </div>
  );
}

/**
 * Configure the native forecast method and its parameters. The parameter form
 * is Rust's ForecastParameters schema; WASM validates and evaluates the result.
 */
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
  const periods = model.periods.periods
    .filter((period) => !period.is_actual)
    .map((period) => period.id);
  const [method, setMethod] = useState<Method>(
    node.forecast?.method ?? "forward_fill",
  );
  const [error, setError] = useState<string | null>(null);
  const selected = methods.find((item) => item.id === method)!;
  const module = useMemo(() => forecastParamsModule(method), [method]);
  const history = model.periods.periods
    .filter((period) => period.is_actual)
    .flatMap((period) => {
      const value = node.values?.[period.id];
      return value === undefined
        ? []
        : [typeof value === "number" ? String(value) : value.amount];
    });
  const nodeIds = Object.keys(model.nodes).filter((id) => id !== nodeId);
  // Fields whose control depends on the model: its periods, reported values and lines.
  const renderField: SchemaFormProps["renderField"] = ({
    form,
    path,
    label,
    value,
    location,
  }) => {
    const help = location.schema.description;
    if (path === "curve") {
      const curve = Array.isArray(value) ? value : [];
      return periods.map((id, index) => (
        <FieldFrame key={id} label={`${id} growth rate`} help={help}>
          {(control) => (
            <Input
              {...control}
              inputMode="decimal"
              className="font-mono"
              value={String(curve[index] ?? "")}
              onChange={(event) =>
                form.setFieldValue(
                  path,
                  curve.map((item, position) =>
                    position === index ? event.target.value : item,
                  ),
                )
              }
            />
          )}
        </FieldFrame>
      ));
    }
    if (path === "overrides") {
      const entries = (Array.isArray(value) ? value : []) as {
        key: string;
        value: unknown;
      }[];
      return periods.map((id) => (
        <FieldFrame key={id} label={`${id} scheduled value`} help={help}>
          {(control) => (
            <Input
              {...control}
              inputMode="decimal"
              className="font-mono"
              placeholder="Carry forward"
              value={String(
                entries.find((entry) => entry.key === id)?.value ?? "",
              )}
              onChange={(event) =>
                form.setFieldValue(path, [
                  ...entries.filter((entry) => entry.key !== id),
                  ...(event.target.value.trim()
                    ? [{ key: id, value: event.target.value }]
                    : []),
                ])
              }
            />
          )}
        </FieldFrame>
      ));
    }
    if (path === "historical")
      return (
        <HistoricalField
          form={form}
          path={path}
          label={label}
          help={help}
          value={value}
          history={history}
        />
      );
    if (path === "correlation_with")
      return (
        <IdCombobox
          label={label}
          help={help}
          value={typeof value === "string" ? value : ""}
          onValueChange={(next) => form.setFieldValue(path, next)}
          options={nodeIds}
        />
      );
    return undefined;
  };
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
          onValueChange={(next) => {
            setMethod(next as Method);
            setError(null);
          }}
          options={methods.map((item) => ({
            value: item.id,
            label: item.label,
            description: item.help,
          }))}
        />
      </div>
      <div className="mt-4">
        <SchemaForm
          key={method}
          module={module}
          defaultValues={startingParams(
            method,
            method === node.forecast?.method
              ? (node.forecast.params ?? {})
              : {},
            periods,
          )}
          layout="full"
          explicitNull={false}
          renderField={renderField}
          submitLabel={applyLabel}
          validate={async (json) => json}
          onSubmit={async (json) =>
            setError(await onApply({ method, params: JSON.parse(json) }))
          }
        />
      </div>
      {error && (
        <p role="alert" className="mt-2 text-xs text-error">
          {error}
        </p>
      )}
    </section>
  );
}
