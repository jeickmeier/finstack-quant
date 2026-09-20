"use client";
import { useState } from "react";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import fixture from "@/lib/finstack/fixtures/results/bond.json";
import { SchemaForm, type SchemaFormProps } from "../schema-form/schema-form";
import type { FieldRenderer } from "../schema-form/field-renderer";
import {
  KnotTable,
  type KnotEdit,
} from "../../primitives/knot-table/knot-table";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import { marketModule, importMarket } from "./market";
const editable = ["curves", "fx", "prices"];
const knots: FieldRenderer = ({ form, location, path, value }) => {
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
        <label className="block text-sm">
          Market or calibration result JSON
          <textarea
            aria-label="Market or calibration result JSON"
            value={text}
            onChange={(event) => setText(event.target.value)}
            className="block w-full min-h-24 rounded-sm border border-border bg-background p-2 font-mono text-xs"
          />
        </label>
        <div className="flex gap-3">
          <button type="button" onClick={() => load(text)}>
            Import JSON
          </button>
          <button
            type="button"
            onClick={() => {
              setText(fixture.request.marketJson);
              load(fixture.request.marketJson);
            }}
          >
            Load bond market example
          </button>
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
