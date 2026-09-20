"use client";
import { useId, type ReactNode } from "react";
import { Popover } from "@base-ui/react/popover";
/** Supplied field text and validation status, shared by controlled primitives. */
export interface FieldInfo {
  label: string;
  help?: string;
  error?: string;
  id?: string;
  /** Canonical form path used by the error summary; independent of unique DOM IDs. */
  path?: string;
}
export interface FieldControlProps {
  id: string;
  "aria-labelledby": string;
  "aria-describedby"?: string;
  "aria-invalid"?: boolean;
}
/** Accessible label, supplied error and optional help popover; owns no field value. */
export function FieldFrame({
  label,
  help,
  error,
  id,
  path,
  children,
}: FieldInfo & { children: (control: FieldControlProps) => ReactNode }) {
  const generated = useId();
  const fieldId = id ?? generated;
  return (
    <div
      data-field-path={path}
      className="finstack-field-frame min-w-0 space-y-1 font-sans text-base text-foreground"
    >
      <div className="finstack-field-label flex min-w-0 items-center gap-1">
        <label
          id={`${fieldId}-label`}
          htmlFor={fieldId}
          className="text-sm text-muted-foreground"
        >
          {label}
        </label>
        {help && <FieldHelp label={label} help={help} />}
      </div>
      <div className="finstack-field-control min-w-0">
        {children({
          id: fieldId,
          "aria-labelledby": `${fieldId}-label`,
          "aria-describedby": error ? `${fieldId}-error` : undefined,
          "aria-invalid": Boolean(error),
        })}
      </div>
      {error && (
        <p id={`${fieldId}-error`} role="alert" className="text-xs text-error">
          {error}
        </p>
      )}
    </div>
  );
}

/** Shared on-demand schema description for a field or section. */
export function FieldHelp({ label, help }: { label: string; help: string }) {
  return (
    <Popover.Root>
      <Popover.Trigger
        aria-label={`${label} help`}
        className="inline-flex size-4 shrink-0 items-center justify-center rounded-full border border-border text-xs text-muted-foreground hover:text-primary focus-visible:outline-2 focus-visible:outline-ring"
      >
        ?
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner sideOffset={4}>
          <Popover.Popup className="max-w-sm rounded-md border border-border bg-card p-3 text-sm text-card-foreground shadow-[var(--elevation)]">
            <Popover.Title>{label}</Popover.Title>
            <Popover.Description className="whitespace-pre-wrap">
              {help}
            </Popover.Description>
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}
