"use client";
import { useEffect, useState } from "react";
import { instruments } from "@/lib/finstack/generated/instruments";
import { EnumField } from "../../primitives/enum-field/enum-field";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import {
  SchemaForm,
  type InstrumentModule,
  type SchemaFormProps,
} from "../schema-form/schema-form";
/** Phase 1 accepts only the verified bond renderer; the generated catalogue remains visible. */
export function InstrumentForm(props: {
  type: string;
  onTypeChange(type: string): void;
  /** Initial canonical JSON. Remount the component to load another document; active edits are retained. */
  defaultJson?: string;
  validate: SchemaFormProps["validate"];
  onSubmit: SchemaFormProps["onSubmit"];
  /** Native canonical result, or null as soon as an edit invalidates the prior validation. */
  onValidated?: (json: string | null) => void;
  layout?: SchemaFormProps["layout"];
}) {
  const [loaded, setLoaded] = useState<{
    type: string;
    module: InstrumentModule;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [example, setExample] = useState(0);
  const [initial] = useState(props.defaultJson);
  useEffect(() => {
    let active = true;
    setError(null);
    if (props.type === "bond") {
      const entry = instruments.find((entry) => entry.type === props.type)!;
      void entry.loader().then(
        (module) => {
          if (active) setLoaded({ type: entry.type, module });
        },
        (error) => {
          if (active) setError(String(error));
        },
      );
    }
    return () => {
      active = false;
    };
  }, [props.type]);
  const module = loaded?.type === props.type ? loaded.module : null;
  let defaults: Record<string, unknown> | undefined;
  let parseError: string | undefined;
  if (module && initial && example === 0) {
    try {
      defaults = module.codec.parse(initial) as Record<string, unknown>;
    } catch (error) {
      parseError = error instanceof Error ? error.message : String(error);
    }
  }
  return (
    <section
      aria-label="Instrument form"
      className="space-y-3 font-sans text-foreground"
    >
      <EnumField
        label="Instrument type"
        layout="select"
        value={props.type}
        onValueChange={(type) => {
          props.onValidated?.(null);
          props.onTypeChange(type);
        }}
        options={instruments.map((entry) => ({
          value: entry.type,
          label:
            entry.type === "bond"
              ? entry.title
              : `${entry.title} — not yet supported`,
          group: entry.group,
        }))}
      />
      {props.type !== "bond" ? (
        <p role="status">{props.type}: not yet supported</p>
      ) : (
        <>
          {error && <p role="alert">{error}</p>}
          {!module && !error && <p role="status">Loading instrument schema…</p>}
          {module && (
            <>
              <button
                type="button"
                className="rounded-sm border border-border px-2 py-1 text-sm focus-visible:outline-2 focus-visible:outline-ring"
                onClick={() => {
                  props.onValidated?.(null);
                  setExample((n) => n + 1);
                }}
              >
                Load example
              </button>
              {parseError ? (
                <p role="alert">{parseError}</p>
              ) : (
                <SchemaForm
                  key={`${props.type}:${example}`}
                  module={module}
                  defaultValues={defaults}
                  validate={props.validate}
                  onSubmit={props.onSubmit}
                  onValidated={props.onValidated}
                  layout={props.layout}
                />
              )}
              <details>
                <summary className="text-sm">Example JSON</summary>
                <JsonViewer
                  label="Generated example JSON"
                  text={module.codec.stringify(module.example)}
                />
              </details>
            </>
          )}
        </>
      )}
    </section>
  );
}
