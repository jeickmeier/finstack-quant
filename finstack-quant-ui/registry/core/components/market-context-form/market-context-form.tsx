"use client";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useState, type ReactNode } from "react";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/shared/components/schema-form/schema-form";
import type { FieldRenderer } from "@/components/finstack/shared/components/schema-form/field-renderer";
import {
  KnotTable,
  type KnotEdit,
} from "../../primitives/knot-table/knot-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { marketModule, importMarket } from "./market";
const editable = [
  "curves",
  "fx",
  "prices",
  "surfaces",
  "vol_cubes",
  "fx_delta_vol_surfaces",
];
function DeferredField({
  label,
  children,
}: {
  label: string;
  children: () => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="space-y-2">
      <Button
        variant="outline"
        size="sm"
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        {open ? "Close" : "Edit"} {label}
      </Button>
      {open && children()}
    </div>
  );
}
const knots: FieldRenderer = ({
  form,
  location,
  path,
  value,
  label,
  renderDefault,
}) => {
  if (/^(surfaces|fx_delta_vol_surfaces|vol_cubes)\[\d+\]$/.test(path))
    return (
      <DeferredField
        label={`${label}: ${String((value as { id?: string })?.id ?? "new")}`}
      >
        {renderDefault}
      </DeferredField>
    );
  // This exact stored field is declared by the selected curve variant; no-knot models delegate.
  if (
    !/^curves\[\d+\]\.knot_points$/.test(path) ||
    location.schema.type !== "array" ||
    !Array.isArray(value)
  )
    return undefined;
  return (
    <KnotTable
      value={value as KnotEdit[]}
      rowIds={value.map((_, i) => String(i))}
      onValueChange={(next) => form.setFieldValue(path, next)}
      xLabel="Stored x"
      yLabel="Stored value"
    />
  );
};
/** Complete canonical market editor; generated fields own shapes and native validation owns acceptance. */
export function MarketContextForm({
  defaultJson,
  validate,
  onSubmit,
  onValidated,
}: {
  /** Complete initial market JSON. Remount to replace from external state. */
  defaultJson: string;
  validate: SchemaFormProps["validate"];
  onSubmit: SchemaFormProps["onSubmit"];
  onValidated?: SchemaFormProps["onValidated"];
}) {
  const [document, setDocument] = useState(() => {
    try {
      return {
        value: importMarket(defaultJson),
        revision: 0,
        error: null as string | null,
      };
    } catch (error) {
      return { value: null, revision: 0, error: String(error) };
    }
  });
  const [text, setText] = useState(defaultJson);
  const load = (json: string) => {
    try {
      const value = importMarket(json);
      onValidated?.(null);
      setDocument((current) => ({
        value,
        revision: current.revision + 1,
        error: null,
      }));
    } catch (error) {
      setDocument((current) => ({
        ...current,
        error: error instanceof Error ? error.message : String(error),
      }));
    }
  };
  const readonly =
    document.value &&
    Object.fromEntries(
      Object.entries(document.value).filter(([key]) => !editable.includes(key)),
    );
  return (
    <section
      aria-label="Market context form"
      className="space-y-3 font-sans text-foreground"
    >
      <details>
        <summary className="text-sm">Import market</summary>
        <Label className="grid gap-2">
          Market or calibration result JSON
          <Textarea
            aria-label="Market or calibration result JSON"
            value={text}
            onChange={(event) => setText(event.target.value)}
          />
        </Label>
        <div className="flex gap-3">
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => load(text)}
          >
            Import JSON
          </Button>
        </div>
      </details>
      {document.error && <p role="alert">{document.error}</p>}
      {document.value && (
        <SchemaForm
          key={document.revision}
          module={marketModule}
          defaultValues={document.value}
          layout="full"
          fields={{ allow: editable }}
          renderField={knots}
          submitLabel="Apply market"
          validate={validate}
          onSubmit={onSubmit}
          onValidated={onValidated}
        />
      )}
      {readonly && (
        <details>
          <summary className="text-sm">Other market data (read-only)</summary>
          <JsonViewer
            label="Read-only market data"
            text={serializeHost(readonly)}
          />
        </details>
      )}
    </section>
  );
}
