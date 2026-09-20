"use client";
import { useId, type ReactNode } from "react";
import { Field, FieldLabel, FieldError } from "@/components/ui/field";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverTitle,
  PopoverDescription,
} from "@/components/ui/popover";
/** Supplied field text and validation status, shared by controlled primitives. */
export interface FieldInfo {
  label: string;
  help?: string;
  error?: string;
  id?: string;
  /** Stock field layout for the surrounding domain composition. */
  orientation?: "horizontal" | "vertical" | "responsive";
  /** Canonical form path independent of DOM IDs. */ path?: string;
}
export interface FieldControlProps {
  id: string;
  "aria-labelledby": string;
  "aria-describedby"?: string;
  "aria-invalid"?: boolean;
}
/** Accessible domain field composition using stock shadcn Field and help controls. */
export function FieldFrame({
  label,
  help,
  error,
  id,
  path,
  orientation,
  children,
}: FieldInfo & { children: (control: FieldControlProps) => ReactNode }) {
  const generated = useId(),
    fieldId = id ?? generated;
  return (
    <Field
      orientation={orientation}
      data-field-path={path}
      data-invalid={Boolean(error)}
      className="finstack-field-frame"
    >
      <div className="finstack-field-label flex items-center gap-2">
        <FieldLabel id={`${fieldId}-label`} htmlFor={fieldId}>
          {label}
        </FieldLabel>
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
        <FieldError id={`${fieldId}-error`} role="alert">
          {error}
        </FieldError>
      )}
    </Field>
  );
}
/** Supplied schema description in an unmodified stock popover. */
export function FieldHelp({ label, help }: { label: string; help: string }) {
  return (
    <Popover>
      <PopoverTrigger
        aria-label={`${label} help`}
        render={<Button variant="ghost" size="icon-xs" />}
      >
        ?
      </PopoverTrigger>
      <PopoverContent>
        <PopoverTitle>{label}</PopoverTitle>
        <PopoverDescription>{help}</PopoverDescription>
      </PopoverContent>
    </Popover>
  );
}
