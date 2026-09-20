"use client";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import { createFormHook, createFormHookContexts } from "@tanstack/react-form";
import type { ReactNode } from "react";
import {
  FieldFrame,
  FieldHelp,
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
export function errorText(errors: readonly unknown[]): string | undefined {
  const messages = errors
    .flat(Infinity)
    .flatMap((error) =>
      error && typeof error === "object" && "form" in error
        ? [error.form]
        : [error],
    )
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
    <FieldFrame
      {...props}
      path={field.name}
      dirty={field.state.meta.isDirty}
      error={errorText(field.state.meta.errors)}
    >
      {(control) => (
        <Input
          {...control}
          className={
            props.inputMode && props.inputMode !== "text"
              ? "finstack-numeric"
              : undefined
          }
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
      path={field.name}
      dirty={field.state.meta.isDirty}
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
      path={field.name}
      dirty={field.state.meta.isDirty}
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
      layout="select"
      path={field.name}
      dirty={field.state.meta.isDirty}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}
function BooleanField(props: FieldInfo) {
  const field = useFieldContext<boolean>();
  return (
    <FieldFrame
      {...props}
      path={field.name}
      dirty={field.state.meta.isDirty}
      error={errorText(field.state.meta.errors)}
    >
      {(control) => (
        <Checkbox
          {...control}

          checked={field.state.value ?? false}
          onBlur={field.handleBlur}
          onCheckedChange={(checked) => field.handleChange(checked === true)}
        />
      )}
    </FieldFrame>
  );
}
export function Fieldset({
  label,
  help,
  children,
  className = "",
}: FieldInfo & { children: ReactNode; className?: string }) {
  return (
    <fieldset
      aria-label={label}
      className={`finstack-fieldset min-w-0 ${className}`}
    >
      <legend className="w-full border-b border-border pb-1 text-sm font-medium">
        <span className="inline-flex items-center gap-1">
          {label}
          {help && <FieldHelp label={label} help={help} />}
        </span>
      </legend>
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
          className="finstack-array-row min-w-0 border-b border-border py-2"
        >
          {children(index)}
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              type="button"

              onClick={() => field.removeValue(index)}
            >
              Remove {label} {index + 1}
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"

              disabled={index === 0}
              onClick={() => field.moveValue(index, index - 1)}
            >
              Move {label} {index + 1} up
            </Button>
          </div>
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        type="button"

        onClick={() => field.pushValue(create())}
      >
        Add {label}
      </Button>
    </Fieldset>
  );
}
function SubmitButton({
  label = "Apply instrument",
  disabled,
  allowInvalidSubmission = false,
  variant = "default",
}: {
  label?: string;
  disabled?: boolean;
  allowInvalidSubmission?: boolean;
  /** Use "outline" when a block already owns the primary action (e.g. Price). */
  variant?: "default" | "outline";
}) {
  const form = useFormContext();
  return (
    <form.Subscribe
      selector={(state) =>
        [state.canSubmit, state.isSubmitting, state.isValidating] as const
      }
    >
      {([canSubmit, submitting, validating]) => (
        <Button
          size="sm"
          type="submit"
          variant={variant}
          disabled={
            disabled ||
            (!allowInvalidSubmission && !canSubmit) ||
            submitting ||
            validating
          }
        >
          {submitting ? "Validating…" : label}
        </Button>
      )}
    </form.Subscribe>
  );
}
function ResetButton({ onReset }: { onReset?: () => void }) {
  const form = useFormContext();
  return (
    <Button
      variant="outline"
      size="sm"
      type="button"

      onClick={() => {
        onReset?.();
        form.reset();
        void form.validate("change");
      }}
    >
      Reset edits
    </Button>
  );
}
/** Focus a mounted invalid control, otherwise the accessible summary. */
export function focusFormIssue(form: HTMLFormElement | null) {
  if (!form) return;
  const frames = [...form.querySelectorAll<HTMLElement>("[data-field-path]")];
  for (const frame of frames) {
    if (!frame.querySelector('[aria-invalid="true"]')) continue;
    const control = frame.querySelector<HTMLElement>(
      "input:not([aria-hidden=true]):not([type=hidden]):not(:disabled), textarea:not(:disabled), [role=combobox]:not(:disabled), [role=radio]:not([aria-disabled=true]), [role=checkbox]:not([aria-disabled=true])",
    );
    if (control) {
      control.focus();
      return;
    }
  }
  form.querySelector<HTMLElement>("[data-error-summary]")?.focus();
}
function ErrorSummary({ errors: extra = [] }: { errors?: readonly unknown[] }) {
  const form = useFormContext();
  return (
    <form.Subscribe
      selector={(state) => [state.errors, state.fieldMeta] as const}
    >
      {([errors, fields]) => {
        const all = [
          ...extra,
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
            data-error-summary
            tabIndex={-1}
            aria-label="Validation errors"
            className="rounded-sm border border-error p-2 text-sm text-error focus-visible:outline-2 focus-visible:outline-ring"
          >
            <strong>Review the highlighted terms</strong>
            <p>{message}</p>
            {Object.entries(fields)
              .filter(
                ([, meta]) =>
                  meta &&
                  typeof meta === "object" &&
                  "errors" in meta &&
                  Array.isArray(meta.errors) &&
                  errorText(meta.errors),
              )
              .map(([path]) => (
                <Button
                  variant="link"
                  size="sm"
                  key={path}
                  type="button"

                  onClick={(event) => {
                    const form = event.currentTarget.closest("form");
                    const field = [
                      ...(form?.querySelectorAll<HTMLElement>(
                        "[data-field-path]",
                      ) ?? []),
                    ].find((node) => node.dataset.fieldPath === path);
                    field
                      ?.querySelector<HTMLElement>(
                        "input:not([aria-hidden=true]), textarea, [role=combobox], [role=radio], [role=checkbox]",
                      )
                      ?.focus();
                  }}
                >
                  {path}
                </Button>
              ))}
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
