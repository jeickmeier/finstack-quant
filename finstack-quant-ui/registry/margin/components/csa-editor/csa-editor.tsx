"use client";
import { useState } from "react";
import type { CsaSpec } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { csaModule } from "./csa";

/** A Rust regulatory CSA preset request; the same shape useCsaPreset accepts. */
export type CsaPreset =
  | { readonly kind: "usd" }
  | { readonly kind: "eur" }
  | {
      readonly kind: "regulatory";
      readonly currency: string;
      readonly id: string;
      readonly collateralCurve: string;
    };
export interface CsaEditorProps {
  /** Initial CsaSpec text; omitted starts from the schema's Rust example. Remount to load another document. */
  defaultJson?: string;
  /** Native validation returning canonical CsaSpec text; connect useCsaValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical CSA on Use CSA; connect it to the useVm, useMarginCalls or useMarginCallDates request. */
  onSubmit(json: string): void;
  /** Loads a Rust regulatory preset into the form; connect useCsaPreset. Omitted hides the presets. */
  loadPreset?: (preset: CsaPreset) => Promise<CsaSpec>;
  density?: "compact" | "comfortable";
}
type Initial = { value?: Record<string, unknown>; error?: string };
function parse(json: string): Initial {
  try {
    const value = csaModule.codec.parse(json);
    if (value === null || typeof value !== "object" || Array.isArray(value))
      throw new TypeError("A CSA specification must be a JSON object");
    return { value: value as Record<string, unknown> };
  } catch (failure) {
    return {
      error: failure instanceof Error ? failure.message : String(failure),
    };
  }
}
/**
 * Edits a Credit Support Annex with the generated schema form, optionally
 * starting from a Rust regulatory preset (USD, EUR, or regulatory terms for a
 * currency, id and collateral curve the caller types). Validation, the
 * presets and every term are native; nothing is defaulted here.
 */
export function CsaEditor({
  defaultJson,
  validate,
  onSubmit,
  loadPreset,
  density = "compact",
}: CsaEditorProps) {
  const [doc, setDoc] = useState<Initial & { revision: number }>(() => ({
    revision: 0,
    ...(defaultJson === undefined ? {} : parse(defaultJson)),
  }));
  const [presetError, setPresetError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [currency, setCurrency] = useState("");
  const [id, setId] = useState("");
  const [collateralCurve, setCollateralCurve] = useState("");
  async function load(preset: CsaPreset) {
    if (!loadPreset) return;
    setLoading(true);
    setPresetError(null);
    try {
      const csa = await loadPreset(preset);
      setDoc((current) => ({
        // finstack-arith: remount revision counter, not a value
        revision: current.revision + 1,
        value: csa as unknown as Record<string, unknown>,
      }));
    } catch (failure) {
      setPresetError(
        failure instanceof Error ? failure.message : String(failure),
      );
    } finally {
      setLoading(false);
    }
  }
  if (doc.error) return <p role="alert">{doc.error}</p>;
  return (
    <section
      aria-label="CSA editor"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      {loadPreset ? (
        <section aria-label="Regulatory presets" className="space-y-3">
          <h3 className="text-sm font-medium">Regulatory presets</h3>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              disabled={loading}
              onClick={() => load({ kind: "usd" })}
            >
              USD regulatory
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={loading}
              onClick={() => load({ kind: "eur" })}
            >
              EUR regulatory
            </Button>
          </div>
          <form
            aria-label="Regulatory terms for a currency"
            className="grid items-end gap-3 @lg:grid-cols-4"
            onSubmit={(event) => {
              event.preventDefault();
              void load({ kind: "regulatory", currency, id, collateralCurve });
            }}
          >
            <FieldFrame label="Currency" help="ISO-4217 base currency.">
              {(control) => (
                <Input
                  {...control}
                  value={currency}
                  onChange={(event) => setCurrency(event.target.value)}
                />
              )}
            </FieldFrame>
            <FieldFrame label="CSA id" help="Identifier stamped on the CSA.">
              {(control) => (
                <Input
                  {...control}
                  value={id}
                  onChange={(event) => setId(event.target.value)}
                />
              )}
            </FieldFrame>
            <FieldFrame
              label="Collateral curve"
              help="Discounting curve id, for example USD-OIS."
            >
              {(control) => (
                <Input
                  {...control}
                  value={collateralCurve}
                  onChange={(event) => setCollateralCurve(event.target.value)}
                />
              )}
            </FieldFrame>
            <Button type="submit" variant="outline" disabled={loading}>
              Load regulatory terms
            </Button>
          </form>
          {presetError ? (
            <p role="alert" className="text-error">
              {presetError}
            </p>
          ) : null}
        </section>
      ) : null}
      <SchemaForm
        key={doc.revision}
        module={csaModule}
        defaultValues={doc.value}
        validate={validate}
        onSubmit={onSubmit}
        submitLabel="Use CSA"
        layout="basic"
      />
    </section>
  );
}
