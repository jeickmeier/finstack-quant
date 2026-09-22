"use client";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { useState, type ReactNode } from "react";
import { parse } from "lossless-json";
import bondSchema from "@/lib/finstack/generated/schemas/bond.json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import { DateInput } from "@/components/finstack/core/primitives/date-input/date-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { SchemaForm } from "@/components/finstack/shared/components/schema-form/schema-form";
import type {
  InstrumentModule,
  Schema,
} from "@/components/finstack/shared/components/schema-form/schema";
import { MetricPicker } from "../../primitives/metric-picker/metric-picker";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
function reachableDefs(
  node: Schema,
  defs: Record<string, Schema>,
  used: Record<string, Schema> = {},
): Record<string, Schema> {
  const visit = (value: unknown) => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      for (const item of value) visit(item);
      return;
    }
    const record = value as Schema;
    const ref = record.$ref;
    if (ref?.startsWith("#/$defs/")) {
      const name = ref.slice("#/$defs/".length);
      if (!Object.hasOwn(used, name)) {
        const target = defs[name];
        if (!target) throw new Error(`Missing schema reference: ${ref}`);
        used[name] = target;
        visit(target);
      }
    }
    for (const child of Object.values(record)) visit(child);
  };
  visit(node);
  return used;
}
function metricOverridesModule(): InstrumentModule {
  const bond = bondSchema as unknown as Schema;
  const defs = bond.$defs ?? {};
  const source = Object.values(defs).find(
    (node) => node?.title === "Metric Pricing Overrides",
  );
  if (!source) throw new Error("Metric Pricing Overrides schema is missing");
  const schema = structuredClone({
    ...source,
    title: "Pricing overrides",
    $defs: reachableDefs(source, defs),
  });
  return {
    schema,
    metadata: [],
    codec: createWireCodec(schema),
    example: {},
  };
}
const metricOverrides = metricOverridesModule();
function overrideValues(
  text: string | null | undefined,
): Record<string, unknown> {
  if (!text) return {};
  try {
    const value = JSON.parse(text) as unknown;
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}
function compactOverrides(text: string): string | undefined {
  const compact = (value: unknown): unknown => {
    if (typeof value === "string")
      return value.trim() === "" ? undefined : value;
    if (value == null) return undefined;
    if (Array.isArray(value)) {
      const items = value.map(compact);
      return items.some((item) => item !== undefined) ? items : undefined;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value).flatMap(([key, item]) => {
        const next = compact(item);
        return next === undefined ? [] : [[key, next] as const];
      });
      return entries.length ? Object.fromEntries(entries) : undefined;
    }
    return value;
  };
  const value = compact(JSON.parse(text) as unknown);
  return value && typeof value === "object" && !Array.isArray(value)
    ? JSON.stringify(value)
    : undefined;
}
/** Caller-owned text is preserved even while incomplete; null/undefined mean omission. */
export interface PricingParams {
  asOf: string;
  model?: string | null;
  metrics?: readonly string[] | null;
  pricingOptions?: string | null;
  marketHistory?: string | null;
}
/** Syntax validation only. Native pricing owns override and history domain rules. */
export function jsonError(text: string | null | undefined): string | undefined {
  if (text == null) return undefined;
  try {
    parse(text);
    return undefined;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}
function JsonInput({
  label,
  value,
  onValueChange,
}: {
  label: string;
  value: string | null | undefined;
  onValueChange(value: string | undefined): void;
}) {
  return (
    <div className="space-y-2">
      <FieldFrame
        label={label}
        error={jsonError(value)}
        help="Original JSON text is sent unchanged. Native validation checks its meaning."
      >
        {(control) => (
          <Textarea
            {...control}
            rows={3}
            value={value ?? ""}
            onChange={(event) => onValueChange(event.target.value)}
          />
        )}
      </FieldFrame>
      <Button
        variant="outline"
        size="sm"
        type="button"
        onClick={() => onValueChange(undefined)}
      >
        Omit {label.toLowerCase()}
      </Button>
      <details>
        <summary className="text-sm">View {label.toLowerCase()}</summary>
        <JsonViewer label={`${label} JSON`} text={value} />
      </details>
    </div>
  );
}
function PricingOverrides({
  value,
  onValueChange,
}: {
  value: string | null | undefined;
  onValueChange(value: string | undefined): void;
}) {
  const [generation, setGeneration] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-sm font-medium">Pricing overrides</p>
      <SchemaForm
        key={generation}
        module={metricOverrides}
        layout="basic"
        actions={false}
        explicitNull={false}
        defaultValues={overrideValues(value)}
        validate={async (json) => json}
        onSubmit={() => {}}
        onInputJson={(json) => {
          if (json == null) return;
          const pricingOptions = compactOverrides(json);
          if ((value ?? undefined) === pricingOptions) return;
          onValueChange(pricingOptions);
        }}
      />
      <Button
        variant="outline"
        size="sm"
        type="button"
        onClick={() => {
          onValueChange(undefined);
          setGeneration((current) => current + 1);
        }}
      >
        Omit pricing overrides
      </Button>
    </div>
  );
}
/** Independent controlled pricing controls. Supply groups from useModels/useMetrics within the shared provider. */
export function PricingParamsForm({
  value,
  onValueChange,
  instrumentType,
  models,
  metrics,
  marketHistorySlot,
  loading,
  error,
  sections = ["context", "metrics", "advanced"],
}: {
  /** Select composable control groups for a toolbar or settings panel. */
  sections?: readonly ("context" | "metrics" | "advanced")[];
  value: PricingParams;
  onValueChange(value: PricingParams): void;
  instrumentType: string;
  models: Readonly<Record<string, readonly string[]>>;
  metrics: Readonly<Record<string, readonly string[]>>;
  /** Optional caller-owned history UI; the parent still owns value.marketHistory. */
  marketHistorySlot?: ReactNode;
  loading?: boolean;
  error?: string;
}) {
  return (
    <section
      aria-label="Pricing parameters"
      aria-busy={loading}
      className="finstack-pricing-params font-sans text-foreground"
    >
      {loading && <p role="status">Loading pricing options…</p>}
      {error && <p role="alert">{error}</p>}
      {sections.includes("context") && (
        <div className="finstack-pricing-context">
          <div className="finstack-pricing-context__date w-fit max-w-full">
            <DateInput
              orientation="horizontal"
              label="As of"
              value={value.asOf}
              onValueChange={(asOf) => onValueChange({ ...value, asOf })}
            />
          </div>
          <div className="w-fit max-w-full">
            <EnumField
              layout="select"
              orientation="horizontal"
              label="Pricing model"
              value={value.model ?? ""}
              onValueChange={(model) => onValueChange({ ...value, model })}
              options={(models[instrumentType] ?? []).map((model) => ({
                value: model,
                label: model,
              }))}
              disabled={loading}
            />
          </div>
          {!loading &&
            value.model &&
            !(models[instrumentType] ?? []).includes(value.model) && (
              <p role="status">
                Selected model unavailable for {instrumentType}: {value.model}
              </p>
            )}
        </div>
      )}
      {sections.includes("metrics") && (
        <div className="finstack-pricing-metrics">
          <MetricPicker
            label="Metrics"
            value={[...(value.metrics ?? [])]}
            onValueChange={(metrics) => onValueChange({ ...value, metrics })}
            options={Object.entries(metrics).flatMap(([group, names]) =>
              names.map((name) => ({ value: name, label: name, group })),
            )}
            disabled={loading}
          />
          <Button
            variant="outline"
            size="sm"
            type="button"

            onClick={() => onValueChange({ ...value, metrics: undefined })}
          >
            Omit metric selection
          </Button>
        </div>
      )}
      {sections.includes("advanced") && (
        <div className="finstack-pricing-advanced">
          <PricingOverrides
            value={value.pricingOptions}
            onValueChange={(pricingOptions) =>
              onValueChange({ ...value, pricingOptions })
            }
          />
          {marketHistorySlot ?? (
            <JsonInput
              label="Market history"
              value={value.marketHistory}
              onValueChange={(marketHistory) =>
                onValueChange({ ...value, marketHistory })
              }
            />
          )}
        </div>
      )}
    </section>
  );
}
