"use client";
import { createFormHook, createFormHookContexts } from "@tanstack/react-form";
import type { ReactNode } from "react";
import {
  FieldFrame,
  type FieldInfo,
} from "@/components/finstack/primitives/field-frame/field-frame";
import { DecimalInput } from "@/components/finstack/primitives/decimal-input/decimal-input";
import { DateInput } from "@/components/finstack/primitives/date-input/date-input";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/primitives/enum-field/enum-field";
export const { fieldContext, formContext, useFieldContext, useFormContext } =
  createFormHookContexts();
export const fieldClass =
  "finstack-field w-full rounded-sm border border-border bg-background px-2 text-foreground focus-visible:outline-2 focus-visible:outline-ring";
export const buttonClass =
  "rounded-sm border border-border px-2 py-1 text-sm focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-50";
export function errorText(errors: readonly unknown[]): string | undefined {
  const messages = errors
    .flat(Infinity)
    .filter(Boolean)
    .map((error) =>
      typeof error === "object" && error && "message" in error
        ? String(error.message)
        : typeof error === "string"
          ? error
          : "Invalid field value",
    );
  return messages.length ? [...new Set(messages)].join("; ") : undefined;
}
function TextField(
  props: FieldInfo & { inputMode?: "text" | "numeric" | "decimal" },
) {
  const field = useFieldContext<unknown>();
  return (
    <FieldFrame {...props} error={errorText(field.state.meta.errors)}>
      {(control) => (
        <input
          {...control}
          className={fieldClass}
          inputMode={props.inputMode}
          value={String(field.state.value ?? "")}
          onBlur={field.handleBlur}
          onChange={(event) => field.handleChange(event.target.value)}
        />
      )}
    </FieldFrame>
  );
}
function DecimalField(props: FieldInfo & { pattern?: string }) {
  const field = useFieldContext<string>();
  return (
    <DecimalInput
      {...props}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}
function DateField(props: FieldInfo) {
  const field = useFieldContext<string>();
  return (
    <DateInput
      {...props}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}
function SelectField(props: FieldInfo & { options: readonly EnumOption[] }) {
  const field = useFieldContext<string>();
  return (
    <EnumField
      {...props}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}
function BooleanField(props: FieldInfo) {
  const field = useFieldContext<boolean>();
  return (
    <FieldFrame {...props} error={errorText(field.state.meta.errors)}>
      {(control) => (
        <input
          {...control}
          type="checkbox"
          checked={field.state.value ?? false}
          onBlur={field.handleBlur}
          onChange={(event) => field.handleChange(event.target.checked)}
        />
      )}
    </FieldFrame>
  );
}
export function Fieldset({
  label,
  help,
  children,
}: FieldInfo & { children: ReactNode }) {
  return (
    <fieldset className="col-span-full min-w-0 space-y-3 rounded-sm border border-border p-3">
      <legend className="px-1 text-sm font-semibold">{label}</legend>
      {help && (
        <details className="text-sm text-muted-foreground">
          <summary>About {label.toLowerCase()}</summary>
          <p className="whitespace-pre-wrap">{help}</p>
        </details>
      )}
      {children}
    </fieldset>
  );
}
function ArrayField({
  label,
  create,
  children,
}: {
  label: string;
  create(): unknown;
  children(index: number): ReactNode;
}) {
  const field = useFieldContext<unknown[]>();
  const rows = field.state.value ?? [];
  return (
    <Fieldset label={label}>
      {rows.map((_, index) => (
        <div
          key={index}
          className="space-y-2 rounded-sm border border-border p-2"
        >
          {children(index)}
          <div className="flex gap-2">
            <button
              type="button"
              className={buttonClass}
              onClick={() => field.removeValue(index)}
            >
              Remove {label} {index + 1}
            </button>
            <button
              type="button"
              className={buttonClass}
              disabled={index === 0}
              onClick={() => field.moveValue(index, index - 1)}
            >
              Move {label} {index + 1} up
            </button>
          </div>
        </div>
      ))}
      <button
        type="button"
        className={buttonClass}
        onClick={() => field.pushValue(create())}
      >
        Add {label}
      </button>
    </Fieldset>
  );
}
function SubmitButton({ label = "Apply instrument" }: { label?: string }) {
  const form = useFormContext();
  return (
    <form.Subscribe
      selector={(state) =>
        [state.canSubmit, state.isSubmitting, state.isValidating] as const
      }
    >
      {([canSubmit, submitting, validating]) => (
        <button
          type="submit"
          className={buttonClass}
          disabled={!canSubmit || submitting || validating}
        >
          {submitting ? "Validating…" : label}
        </button>
      )}
    </form.Subscribe>
  );
}
function ResetButton() {
  const form = useFormContext();
  return (
    <button type="button" className={buttonClass} onClick={() => form.reset()}>
      Reset edits
    </button>
  );
}
function ErrorSummary() {
  const form = useFormContext();
  return (
    <form.Subscribe
      selector={(state) => [state.errors, state.fieldMeta] as const}
    >
      {([errors, fields]) => {
        const all = [
          ...errors,
          ...Object.values(fields).flatMap((field) =>
            field &&
            typeof field === "object" &&
            "errors" in field &&
            Array.isArray(field.errors)
              ? field.errors
              : [],
          ),
        ];
        const message = errorText(all);
        return message ? (
          <div
            role="alert"
            className="rounded-sm border border-error p-2 text-sm text-error"
          >
            <strong>Review the instrument</strong>
            <p>{message}</p>
          </div>
        ) : null;
      }}
    </form.Subscribe>
  );
}
/** Shared TanStack Form contexts and registry primitives; no canonical financial logic. */
export const { useAppForm } = createFormHook({
  fieldContext,
  formContext,
  fieldComponents: {
    TextField,
    DecimalField,
    DateField,
    SelectField,
    BooleanField,
    ArrayField,
  },
  formComponents: { Fieldset, SubmitButton, ResetButton, ErrorSummary },
});
