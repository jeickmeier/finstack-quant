"use client";
import { useId, type ReactNode } from "react";
import { Popover } from "@base-ui/react/popover";
/** Supplied field text and validation status, shared by controlled primitives. */
export interface FieldInfo {
  label: string;
  help?: string;
  error?: string;
  id?: string;
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
  children,
}: FieldInfo & { children: (control: FieldControlProps) => ReactNode }) {
  const generated = useId();
  const fieldId = id ?? generated;
  return (
    <div className="min-w-0 space-y-1 font-sans text-base text-foreground">
      <div className="flex items-center gap-2">
        <label
          id={`${fieldId}-label`}
          htmlFor={fieldId}
          className="text-sm text-muted-foreground"
        >
          {label}
        </label>
        {help && (
          <Popover.Root>
            <Popover.Trigger
              aria-label={`${label} help`}
              className="text-sm text-primary focus-visible:outline-2 focus-visible:outline-ring"
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
        )}
      </div>
      {children({
        id: fieldId,
        "aria-labelledby": `${fieldId}-label`,
        "aria-describedby": error ? `${fieldId}-error` : undefined,
        "aria-invalid": Boolean(error),
      })}
      {error && (
        <p id={`${fieldId}-error`} role="alert" className="text-xs text-error">
          {error}
        </p>
      )}
    </div>
  );
}
