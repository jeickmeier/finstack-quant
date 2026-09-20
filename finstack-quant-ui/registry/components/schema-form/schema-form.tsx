"use client";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAppForm } from "@/lib/finstack/form";
import { RenderField } from "./render-field";
import {
  structuralValidator,
  objectValue,
  type InstrumentModule,
} from "./schema";
export type { InstrumentModule };
export interface FieldFilter {
  allow?: readonly string[];
  deny?: readonly string[];
}
export interface SchemaFormProps {
  /** Lazily import the generated instrument module; only bond-subset coverage is accepted here. */
  module: InstrumentModule;
  /** Initial working host values; changing this prop does not reset an active edit. Remount for another instrument. */
  defaultValues?: Record<string, unknown>;
  layout?: "basic" | "full";
  /** Canonical field paths, including instrument.spec. Hidden fields retain their values. */
  fields?: FieldFilter;
  /** Connect useInstrumentValidator() inside FinstackProvider; returns native canonical JSON. */
  validate(json: string, signal?: AbortSignal): Promise<string>;
  /** Receives canonical native JSON only after valid submission. Never replaces active working values. */
  onSubmit(json: string): void | Promise<void>;
}
function useSchemaForm(props: SchemaFormProps) {
  const validator = useMemo(
    () => structuralValidator(props.module),
    [props.module],
  );
  const [initial] = useState(() =>
    structuredClone(props.defaultValues ?? objectValue(props.module.example)),
  );
  const valid = useRef<{
    input: string;
    canonical: string;
    validate: SchemaFormProps["validate"];
  } | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const input = (value: unknown) =>
    props.module.codec.stringify(validator.parse(value));
  const form = useAppForm({
    defaultValues: initial,
    listeners: { onChange: () => setSubmitError(null) },
    validators: {
      onChange: validator,
      onChangeAsyncDebounceMs: 250,
      onChangeAsync: async ({ value, signal }) => {
        try {
          const text = input(value);
          const canonical = await props.validate(text, signal);
          if (!signal.aborted)
            valid.current = {
              input: text,
              canonical,
              validate: props.validate,
            };
          return undefined;
        } catch (error) {
          return signal.aborted
            ? undefined
            : { form: error instanceof Error ? error.message : String(error) };
        }
      },
    },
    onSubmit: async ({ value }) => {
      setSubmitError(null);
      try {
        const text = input(value);
        const canonical =
          valid.current?.input === text &&
          valid.current.validate === props.validate
            ? valid.current.canonical
            : await props.validate(text);
        await props.onSubmit(canonical);
      } catch (error) {
        setSubmitError(error instanceof Error ? error.message : String(error));
      }
    },
  });
  useEffect(() => {
    // A ready/restarted worker supplies a new validator; retry without changing edits.
    void form.validate("change");
  }, [form, props.validate]);
  return { form, submitError };
}
export type SchemaFormApi = ReturnType<typeof useSchemaForm>["form"];
/** Schema-driven bond term sheet with structural change and abort-aware native validation. */
export function SchemaForm(props: SchemaFormProps) {
  const { form, submitError } = useSchemaForm(props);
  return (
    <form
      noValidate
      className="space-y-3 font-sans text-foreground"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <form.AppForm>
        <form.ErrorSummary />
        {submitError && (
          <p role="alert" className="text-sm text-error">
            {submitError}
          </p>
        )}
        <form.Subscribe selector={(state) => state.values}>
          {(value) => (
            <RenderField
              module={props.module}
              form={form}
              location={{ schema: props.module.schema, pointer: "#" }}
              path=""
              label={props.module.schema.title ?? "Instrument"}
              value={value}
              layout={props.layout ?? "basic"}
              fields={props.fields}
            />
          )}
        </form.Subscribe>
        <div className="flex gap-2">
          <form.SubmitButton />
          <form.ResetButton />
        </div>
      </form.AppForm>
    </form>
  );
}
