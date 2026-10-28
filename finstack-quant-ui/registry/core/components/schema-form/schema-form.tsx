"use client";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { useStore } from "@tanstack/react-form";
import { useAppForm, focusFormIssue } from "@/lib/finstack/form";
import { FieldRendererContext, type FieldRenderer } from "./field-renderer";
import { FormChromeContext } from "./field-chrome";
import { RenderField } from "./render-field";
import {
  SheetContext,
  revealKey,
  secondaryDisclosure,
  type Disclosure,
  type SheetRoot,
} from "./disclosure";
import type { InstrumentModule } from "./schema";
import { editValue, objectValue, structuralValidator } from "./schema-walk";
export type { InstrumentModule };
export interface FieldFilter {
  allow?: readonly string[];
  deny?: readonly string[];
}
/** What a promoted field can reach: the live form, its working values and group reveal. */
export type SheetControls = {
  form: SchemaFormApi;
  values: Record<string, unknown>;
  /** Open, focus and scroll to the collapsed `group` of the object at `path`; a no-op when that group is not rendered. */
  reveal(path: string, group: string): void;
};
/** One input rendered before the sheet; its canonical `path` is left out of the sheet. */
export type PromotedField = { path: string; node: ReactNode };
export interface SchemaFormProps {
  /** Lazily import a generated contract module; the shared renderer follows its reachable schema. */
  module: InstrumentModule;
  /** Initial working host values; changing this prop does not reset an active edit. Remount for another document. */
  defaultValues?: Record<string, unknown>;
  layout?: "basic" | "full";
  /** Canonical dotted field paths from the document root. Hidden fields retain their values. */
  fields?: FieldFilter;
  /** Where layout="basic" collapses object properties. Defaults to Less common terms for defaulted or empty values. */
  disclosure?: Disclosure;
  /** Object rendered as the term sheet; its ancestors render without fieldsets. */
  sheet?: SheetRoot;
  /** One input shown before the sheet for the current values, or null for none. */
  promotedField?(controls: SheetControls): PromotedField | null;
  /** Replace presentation for selected schema fields while retaining shared validation. */
  renderField?: FieldRenderer;
  submitLabel?: string;
  /** "outline" when the composing block owns the primary action. */
  submitVariant?: "default" | "outline";
  /** Native validation for this contract, usually a validator hook inside FinstackProvider; returns native canonical JSON. */
  validate(json: string, signal?: AbortSignal): Promise<string>;
  /** Receives canonical native JSON only after valid submission. Never replaces active working values. */
  onSubmit(json: string): void | Promise<void>;
  /** Canonical validation result, cleared immediately when working values change. */
  onValidated?: (json: string | null) => void;
  /** Structurally valid working JSON before native acceptance; null immediately on edits. Useful for native dry-run diagnostics. */
  onInputJson?: (json: string | null) => void;
  /** Render Apply and Reset. Embedded editors publish through onInputJson instead. */
  actions?: boolean;
  /** Show “Set to null” before a nullable field is added. */
  explicitNull?: boolean;
}
function useSchemaForm(props: SchemaFormProps) {
  const validator = useMemo(() => {
    const schema = structuralValidator(props.module);
    let snapshot: unknown;
    let result: ReturnType<typeof schema.safeParse> | undefined;
    const safeParse = (value: unknown) => {
      // TanStack values are immutable snapshots. Share their transformed result
      // across field errors, initial/reset validation, native checks and submit.
      if (!result || snapshot !== value) {
        snapshot = value;
        result = schema.safeParse(value);
      }
      return result;
    };
    return {
      safeParse,
      hasSnapshot: (value: unknown) =>
        result !== undefined && snapshot === value,
      "~standard": {
        version: 1 as const,
        vendor: "finstack",
        validate(value: unknown) {
          const parsed = safeParse(value);
          return parsed.success
            ? { value: parsed.data }
            : { issues: parsed.error.issues };
        },
      },
    };
  }, [props.module]);
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
  const notifyInput = useRef(props.onInputJson);
  notifyInput.current = props.onInputJson;
  const valid = useRef<{
    input: string;
    canonical: string;
    validate: SchemaFormProps["validate"];
  } | null>(null);
  const request = useRef<AbortController | null>(null);
  const submission = useRef<AbortController | null>(null);
  useEffect(() => () => submission.current?.abort(), []);
  const [native, setNative] = useState<{
    pending: boolean;
    error: string | null;
  }>({ pending: true, error: null });
  const [submitError, setSubmitError] = useState<string | null>(null);
  const form = useAppForm({
    defaultValues: initial,
    listeners: {
      onChange: () => {
        request.current?.abort();
        submission.current?.abort();
        setSubmitError(null);
        notify.current?.(null);
        notifyInput.current?.(null);
      },
    },
    validators: {
      onChange: validator,
      onSubmit: validator,
    },
    onSubmit: async ({ value }) => {
      submission.current?.abort();
      const controller = new AbortController();
      submission.current = controller;
      setSubmitError(null);
      try {
        const structural = validator.safeParse(value);
        if (!structural.success) throw structural.error;
        const text = serializeHost(structural.data);
        const canonical =
          valid.current?.input === text &&
          valid.current.validate === props.validate
            ? valid.current.canonical
            : await props.validate(text, controller.signal);
        if (!controller.signal.aborted) await props.onSubmit(canonical);
      } catch (error) {
        if (!controller.signal.aborted)
          setSubmitError(
            error instanceof Error ? error.message : String(error),
          );
      }
    },
  });
  const values = useStore(form.store, (state) => state.values);
  useEffect(() => {
    const controller = new AbortController();
    request.current = controller;
    // Validate initial/reset values here; edits were already checked by TanStack.
    if (!validator.hasSnapshot(values)) void form.validate("change");
    notify.current?.(null);
    notifyInput.current?.(null);
    setNative({ pending: true, error: null });
    const timer = setTimeout(async () => {
      const structural = validator.safeParse(values);
      if (!structural.success) {
        setNative({ pending: false, error: null });
        return;
      }
      try {
        const text = serializeHost(structural.data);
        notifyInput.current?.(text);
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
      submission.current?.abort();
    };
  }, [values, validator, props.module, props.validate, form]);
  return { form, submitError, native };
}
export type SchemaFormApi = ReturnType<typeof useSchemaForm>["form"];

/** Schema-driven form over a generated contract with structural change and abort-aware native validation. */
export function SchemaForm(props: SchemaFormProps) {
  const { form, submitError, native } = useSchemaForm(props);
  const element = useRef<HTMLFormElement>(null);
  const revealers = useRef(new Map<string, () => void>());
  const reveal = (path: string, group: string) =>
    revealers.current.get(revealKey(path, group))?.();
  const [focusAttempt, setFocusAttempt] = useState(0);
  useEffect(() => {
    if (focusAttempt) focusFormIssue(element.current);
  }, [focusAttempt]);
  return (
    <form
      ref={element}
      noValidate
      className="finstack-term-sheet space-y-3 font-sans text-foreground"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        const snapshot = form.state.values;
        void form.handleSubmit().finally(() => {
          if (form.state.values === snapshot)
            setFocusAttempt((value) => value + 1);
        });
      }}
    >
      <SheetContext.Provider
        value={{
          disclosure: props.disclosure ?? secondaryDisclosure,
          root: props.sheet,
          revealers: revealers.current,
        }}
      >
        <FormChromeContext.Provider
          value={{ explicitNull: props.explicitNull ?? true }}
        >
          <FieldRendererContext.Provider value={props.renderField}>
            <form.AppForm>
              <form.ErrorSummary errors={[submitError ?? native.error]} />
              <form.Subscribe selector={(state) => state.values}>
                {(value) => {
                  const promoted =
                    props.promotedField?.({ form, values: value, reveal }) ??
                    null;
                  return (
                    <>
                      {promoted?.node}
                      <RenderField
                        module={props.module}
                        form={form}
                        location={{
                          schema: props.module.schema,
                          pointer: "#",
                        }}
                        path=""
                        label={props.module.schema.title ?? "Document"}
                        value={value}
                        layout={props.layout ?? "basic"}
                        fields={
                          promoted
                            ? {
                                ...props.fields,
                                deny: [
                                  ...(props.fields?.deny ?? []),
                                  promoted.path,
                                ],
                              }
                            : props.fields
                        }
                      />
                    </>
                  );
                }}
              </form.Subscribe>
              {(props.actions ?? true) && (
                <div className="flex flex-wrap gap-2 border-t border-border pt-2">
                  <form.SubmitButton
                    label={props.submitLabel}
                    variant={props.submitVariant}
                    disabled={native.pending}
                    allowInvalidSubmission
                  />
                  <form.ResetButton onReset={() => props.onValidated?.(null)} />
                </div>
              )}
            </form.AppForm>
          </FieldRendererContext.Provider>
        </FormChromeContext.Provider>
      </SheetContext.Provider>
    </form>
  );
}
