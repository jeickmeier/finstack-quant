"use client";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { useStore } from "@tanstack/react-form";
import { useAppForm, focusFormIssue, errorText } from "@/lib/finstack/form";
import { FieldRendererContext, type FieldRenderer } from "./field-renderer";
import { FormChromeContext } from "./field-chrome";
import { RenderField } from "./render-field";
import type { InstrumentModule } from "./schema";
import {
  editValue,
  labelFor,
  objectValue,
  structuralValidator,
} from "./schema-walk";
export type { InstrumentModule };
export interface FieldFilter {
  allow?: readonly string[];
  deny?: readonly string[];
}
export interface PromotedTerm {
  path: string;
  label: string;
  unit: string;
  hint: string;
  kind:
    "market-quote" | "nullable-term" | "contract-term" | "single-fixed-tranche";
  /** Describes the source used when an optional quote is absent. */
  emptySource?: string;
  /** Other mutually exclusive market quote keys, cleared when this quote is entered. */
  exclusiveWith?: readonly string[];
  /** Set false for instruments without a secondary pricing disclosure. */
  morePricingOverrides?: boolean;
}
export interface SchemaFormProps {
  /** Lazily import the generated instrument module; the shared renderer follows its reachable schema. */
  module: InstrumentModule;
  /** Initial working host values; changing this prop does not reset an active edit. Remount for another instrument. */
  defaultValues?: Record<string, unknown>;
  layout?: "basic" | "full";
  /** Canonical field paths, including instrument.spec. Hidden fields retain their values. */
  fields?: FieldFilter;
  /** One desk-critical native input shown before the full term sheet. */
  promotedTerm?: PromotedTerm;
  /** Replace presentation for selected schema fields while retaining shared validation. */
  renderField?: FieldRenderer;
  submitLabel?: string;
  /** "outline" when the composing block owns the primary action. */
  submitVariant?: "default" | "outline";
  /** Connect useInstrumentValidator() inside FinstackProvider; returns native canonical JSON. */
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

function atPath(value: unknown, path: string): unknown {
  return path
    .split(".")
    .reduce<unknown>((node, key) => objectValue(node)[key], value);
}

function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null && String(value) !== "";
}

function singleFixedTranche(values: Record<string, unknown>) {
  const tranches = atPath(values, "instrument.spec.tranches.tranches");
  if (!Array.isArray(tranches) || tranches.length !== 1) return null;
  const tranche = objectValue(tranches[0]);
  const coupon = objectValue(tranche.coupon);
  if (
    Object.keys(coupon).length !== 1 ||
    !hasValue(objectValue(coupon.fixed).rate)
  )
    return null;
  return tranche;
}

/** Update the optional quote branch as one form edit so omitted wrappers stay omitted. */
function setMarketQuote(
  form: SchemaFormApi,
  values: Record<string, unknown>,
  term: PromotedTerm,
  text: string,
) {
  const spec = objectValue(atPath(values, "instrument.spec"));
  const overrides = {
    ...objectValue(spec.instrument_pricing_overrides),
  };
  const quotes = {
    ...objectValue(overrides.market_quotes),
  };
  const key = term.path.split(".").at(-1)!;
  if (text === "") {
    delete quotes[key];
  } else {
    for (const other of term.exclusiveWith ?? []) delete quotes[other];
    quotes[key] = text;
  }
  // Nullable quote siblings have no effect when absent; keep only supplied values.
  for (const [name, value] of Object.entries(quotes))
    if (value === null || value === undefined) delete quotes[name];
  if (Object.keys(quotes).length) overrides.market_quotes = quotes;
  else delete overrides.market_quotes;
  form.setFieldValue(
    "instrument.spec.instrument_pricing_overrides",
    Object.keys(overrides).length ? overrides : undefined,
  );
}

function PromotedTermField({
  form,
  values,
  term,
}: {
  form: SchemaFormApi;
  values: Record<string, unknown>;
  term: PromotedTerm;
}) {
  const id = useId();
  const optional =
    term.kind === "market-quote" || term.kind === "nullable-term";
  const dealTerm = !optional;
  const trancheId =
    term.kind === "single-fixed-tranche"
      ? singleFixedTranche(values)?.id
      : undefined;
  const quotes = objectValue(
    atPath(
      values,
      "instrument.spec.instrument_pricing_overrides.market_quotes",
    ),
  );
  const competing = (term.exclusiveWith ?? []).filter((key) =>
    hasValue(quotes[key]),
  );
  const setValue = (text: string) => {
    if (term.kind === "market-quote") {
      setMarketQuote(form, values, term, text);
    } else {
      form.setFieldValue(term.path, text === "" && optional ? null : text);
    }
  };
  return (
    <section
      className="finstack-quote-ticket"
      aria-label={dealTerm ? "Deal rate" : "Desk quote"}
    >
      <header className="finstack-quote-ticket__header">
        <span className="finstack-quote-ticket__eyebrow">
          {dealTerm ? "Deal rate" : "Desk quote"}
        </span>
        <span className="finstack-quote-ticket__category">
          {dealTerm ? "Deal term" : "Market override"}
        </span>
      </header>
      <form.AppField name={term.path}>
        {(field) => {
          const error = errorText(field.state.meta.errors);
          const active = hasValue(field.state.value);
          const source = active
            ? dealTerm
              ? "Contract input"
              : "Override active"
            : (term.emptySource ?? "Not set");
          return (
            <div
              className="finstack-quote-ticket__body"
              data-field-path={term.path}
              data-active={active || undefined}
              data-invalid={Boolean(error) || undefined}
            >
              <div className="finstack-quote-ticket__description">
                <div className="finstack-quote-ticket__label-row">
                  <label htmlFor={id}>{term.label}</label>
                  {typeof trancheId === "string" && (
                    <span className="finstack-quote-ticket__tranche-id">
                      {trancheId}
                    </span>
                  )}
                </div>
                <p id={`${id}-hint`}>{term.hint}</p>
                {competing.length > 0 && (
                  <p className="finstack-quote-ticket__replacement">
                    Entering a clean price replaces{" "}
                    {competing.map(labelFor).join(", ")}.
                  </p>
                )}
              </div>
              <div className="finstack-quote-ticket__editor">
                <div className="finstack-quote-ticket__entry">
                  <div className="finstack-quote-ticket__input-wrap">
                    <Input
                      id={id}
                      type="text"
                      inputMode="decimal"
                      className="finstack-quote-ticket__input finstack-numeric"
                      value={active ? String(field.state.value) : ""}
                      placeholder={optional ? "—" : undefined}
                      aria-describedby={`${id}-hint${error ? ` ${id}-error` : ""}`}
                      aria-invalid={Boolean(error)}
                      onBlur={field.handleBlur}
                      onChange={(event) => setValue(event.target.value)}
                    />
                    <span className="finstack-quote-ticket__unit">
                      {term.unit}
                    </span>
                  </div>
                  {optional && active && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      aria-label={`Clear ${term.label.toLowerCase()}`}
                      onClick={() => setValue("")}
                    >
                      Clear
                    </Button>
                  )}
                </div>
                <div className="finstack-quote-ticket__footer">
                  <span
                    className="finstack-quote-ticket__source"
                    data-active={active || undefined}
                  >
                    {source}
                  </span>
                  {term.morePricingOverrides !== false && (
                    <Button
                      type="button"
                      variant="link"
                      size="sm"
                      className="finstack-quote-ticket__more"
                      onClick={(event) => {
                        const details = event.currentTarget
                          .closest("form")
                          ?.querySelector<HTMLDetailsElement>(
                            "details[data-pricing-overrides]",
                          );
                        if (!details) return;
                        details.open = true;
                        details.querySelector<HTMLElement>("summary")?.focus();
                        details.scrollIntoView?.({ block: "nearest" });
                      }}
                    >
                      More pricing overrides
                    </Button>
                  )}
                </div>
                {error && (
                  <p
                    id={`${id}-error`}
                    role="alert"
                    className="finstack-quote-ticket__error"
                  >
                    {error}
                  </p>
                )}
              </div>
            </div>
          );
        }}
      </form.AppField>
    </section>
  );
}

/** Schema-driven instrument term sheet with structural change and abort-aware native validation. */
export function SchemaForm(props: SchemaFormProps) {
  const { form, submitError, native } = useSchemaForm(props);
  const element = useRef<HTMLFormElement>(null);
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
      <FormChromeContext.Provider
        value={{ explicitNull: props.explicitNull ?? true }}
      >
        <FieldRendererContext.Provider value={props.renderField}>
          <form.AppForm>
            <form.ErrorSummary errors={[submitError ?? native.error]} />
            <form.Subscribe selector={(state) => state.values}>
              {(value) => {
                const promotedTerm =
                  props.promotedTerm?.kind === "single-fixed-tranche" &&
                  !singleFixedTranche(value)
                    ? undefined
                    : props.promotedTerm;
                return (
                  <>
                    {promotedTerm && (
                      <PromotedTermField
                        form={form}
                        values={value}
                        term={promotedTerm}
                      />
                    )}
                    <RenderField
                      module={props.module}
                      form={form}
                      location={{ schema: props.module.schema, pointer: "#" }}
                      path=""
                      label={props.module.schema.title ?? "Instrument"}
                      value={value}
                      layout={props.layout ?? "basic"}
                      fields={
                        promotedTerm
                          ? {
                              ...props.fields,
                              deny: [
                                ...(props.fields?.deny ?? []),
                                promotedTerm.path,
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
    </form>
  );
}
