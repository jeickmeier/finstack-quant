"use client";
import { Button } from "@/components/ui/button";
import { useEffect, useState } from "react";
import { instruments } from "@/lib/finstack/generated/instruments";
import { InstrumentSelector } from "./instrument-selector";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import {
  SchemaForm,
  type InstrumentModule,
  type SchemaFormProps,
} from "@/components/finstack/shared/components/schema-form/schema-form";
/** Independently installed all-instrument editor; generated modules convert only when selected. */
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
  submitVariant?: SchemaFormProps["submitVariant"];
}) {
  const [loaded, setLoaded] = useState<{
    type: string;
    module: InstrumentModule;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [example, setExample] = useState(0);
  const [initial] = useState({ type: props.type, json: props.defaultJson });
  useEffect(() => {
    let active = true;
    setError(null);
    const entry = instruments.find((entry) => entry.type === props.type);
    if (entry) {
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
  if (module && initial.type === props.type && initial.json && example === 0) {
    try {
      defaults = module.codec.parse(initial.json) as Record<string, unknown>;
    } catch (error) {
      parseError = error instanceof Error ? error.message : String(error);
    }
  }
  return (
    <section
      aria-label="Instrument form"
      className="space-y-3 font-sans text-foreground"
    >
      <header className="finstack-instrument-header">
        <InstrumentSelector
          value={props.type}
          onValueChange={(type) => {
            props.onValidated?.(null);
            props.onTypeChange(type);
          }}
        />
        {module && (
          <Button
            variant="outline"
            size="sm"
            type="button"

            onClick={() => {
              props.onValidated?.(null);
              setExample((n) => n + 1);
            }}
          >
            Load example
          </Button>
        )}
      </header>
      {!instruments.some((entry) => entry.type === props.type) ? (
        <p role="alert" className="text-sm text-error">
          Unknown instrument type: {props.type}
        </p>
      ) : (
        <>
          {error && (
            <p role="alert" className="text-sm text-error">
              {error}
            </p>
          )}
          {!module && !error && <p role="status">Loading instrument schema…</p>}
          {module && (
            <>
              {parseError ? (
                <p role="alert" className="text-sm text-error">
                  {parseError}
                </p>
              ) : (
                <SchemaForm
                  key={`${props.type}:${example}`}
                  module={module}
                  defaultValues={defaults}
                  validate={props.validate}
                  onSubmit={props.onSubmit}
                  onValidated={props.onValidated}
                  layout={props.layout}
                  submitVariant={props.submitVariant}
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
