"use client";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import type { ReactNode } from "react";
import { parse } from "lossless-json";
import { DateInput } from "../../primitives/date-input/date-input";
import { ModelPicker } from "../../primitives/model-picker/model-picker";
import { MetricPicker } from "../../primitives/metric-picker/metric-picker";
import { FieldFrame } from "../../primitives/field-frame/field-frame";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
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
            <ModelPicker
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
          <JsonInput
            label="Pricing overrides"
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
