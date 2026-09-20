"use client";
import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "@tanstack/react-form";
import { useAppForm } from "@/lib/finstack/form";
import { RenderField } from "./render-field";
import {
  structuralValidator,
  objectValue,
  editValue,
  type InstrumentModule,
} from "./schema";
export type { InstrumentModule };
export interface FieldFilter {
  allow?: readonly string[];
  deny?: readonly string[];
}
export interface SchemaFormProps {
  /** Lazily import the generated instrument module; the shared renderer follows its reachable schema. */
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
  /** Canonical validation result, cleared immediately when working values change. */
  onValidated?: (json: string | null) => void;
}
function useSchemaForm(props: SchemaFormProps) {
  const validator = useMemo(
    () => structuralValidator(props.module),
    [props.module],
  );
  const [initial] = useState(
    () =>
      editValue(
        props.module.schema,
        { schema: props.module.schema, pointer: "#" },
        structuredClone(
          props.defaultValues ?? objectValue(props.module.example),
        ),
      ) as Record<string, unknown>,
  );
  const notify = useRef(props.onValidated);
  notify.current = props.onValidated;
  const valid = useRef<{
    input: string;
    canonical: string;
    validate: SchemaFormProps["validate"];
  } | null>(null);
  const request = useRef<AbortController | null>(null);
  const [native, setNative] = useState<{
    pending: boolean;
    error: string | null;
  }>({ pending: true, error: null });
  const [submitError, setSubmitError] = useState<string | null>(null);
  const input = (value: unknown) =>
    props.module.codec.stringify(validator.parse(value));
  const form = useAppForm({
    defaultValues: initial,
    listeners: {
      onChange: () => {
        request.current?.abort();
        setSubmitError(null);
        notify.current?.(null);
      },
    },
    validators: {
      onChange: validator,
      onSubmit: validator,
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
  const values = useStore(form.store, (state) => state.values);
  useEffect(() => {
    const controller = new AbortController();
    request.current = controller;
    // Structural errors remain in TanStack Form; cancelled native work cannot clear them.
    void form.validate("change");
    notify.current?.(null);
    setNative({ pending: true, error: null });
    const timer = setTimeout(async () => {
      const structural = validator.safeParse(values);
      if (!structural.success) {
        setNative({ pending: false, error: null });
        return;
      }
      try {
        const text = props.module.codec.stringify(structural.data);
        const canonical = await props.validate(text, controller.signal);
        if (controller.signal.aborted) return;
        valid.current = { input: text, canonical, validate: props.validate };
        setNative({ pending: false, error: null });
        notify.current?.(canonical);
      } catch (error) {
        if (!controller.signal.aborted)
          setNative({
            pending: false,
            error: error instanceof Error ? error.message : String(error),
          });
      }
    }, 250);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [values, validator, props.module, props.validate, form]);
  return { form, submitError, native };
}
export type SchemaFormApi = ReturnType<typeof useSchemaForm>["form"];
/** Schema-driven instrument term sheet with structural change and abort-aware native validation. */
export function SchemaForm(props: SchemaFormProps) {
  const { form, submitError, native } = useSchemaForm(props);
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
        {(submitError || native.error) && (
          <p role="alert" className="text-sm text-error">
            {submitError ?? native.error}
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
          <form.SubmitButton
            disabled={native.pending || Boolean(native.error)}
          />
          <form.ResetButton onReset={() => props.onValidated?.(null)} />
        </div>
      </form.AppForm>
    </form>
  );
}
