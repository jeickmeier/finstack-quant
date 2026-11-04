"use client";
import type { ReactNode } from "react";
import { Input } from "@/components/ui/input";
import { formatRaw } from "@/lib/finstack/format/format";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";

/** Typed text as a number; empty text is NaN, never 0, so Rust rejects it. */
export const typedNumber = (text: string | undefined) =>
  !text || text.trim() === "" ? Number.NaN : Number(text);
/** Finite numbers through the host format; words, flags and non-finite values as Rust spells them. */
export function shownValue(
  value: unknown,
  format: (value: number) => string,
): string {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value as string | number | null | undefined);
}
/**
 * Form validation that accepts the typed document as is, for configs Rust
 * validates only when it runs them. Module-level so a form does not revalidate
 * on every render.
 */
export const passThrough = async (json: string) => json;
/** A Rust ISO date at UTC midnight. */
export const utc = (iso: string) => new Date(`${iso}T00:00:00Z`);
export const yesNo = (flag: boolean) => (flag ? "Yes" : "No");
/** A value Rust did not return; `reason`, when supplied, is its title. */
export function UndefinedValue({ reason }: { reason?: string }) {
  return (
    <span className="finstack-na" title={reason}>
      Undefined
    </span>
  );
}
/** Named report region with its shared heading and spacing. */
export function ReportSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}
/** Label/value pairs in the order given, as a two-column list. */
export function Pairs({
  label,
  rows,
}: {
  label: string;
  rows: readonly (readonly [string, ReactNode])[];
}) {
  return (
    <dl
      aria-label={label}
      className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-6 gap-y-1"
    >
      {rows.map(([name, value]) => (
        <div key={name} className="contents">
          <dt className="text-muted-foreground">{name}</dt>
          <dd className="finstack-numeric text-right">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
/** One native call's state, as its hook reports it. */
export interface OutcomeState<T> {
  result: T | null | undefined;
  loading?: boolean;
  /** The native error, shown as returned. */
  error?: string;
}
/** The native error, the working note or the empty note, else the result. */
export function Outcome<T>({
  state,
  empty,
  working,
  children,
}: {
  state: OutcomeState<T>;
  empty: string;
  working: string;
  children: (result: T) => ReactNode;
}) {
  if (state.error)
    return (
      <p role="alert" className="text-error">
        {state.error}
      </p>
    );
  if (state.loading)
    return (
      <p role="status" className="text-muted-foreground">
        {working}
      </p>
    );
  if (state.result == null)
    return (
      <p role="status" className="text-muted-foreground">
        {empty}
      </p>
    );
  return <>{children(state.result)}</>;
}
/** A labelled free-text field; the text is passed on as typed. */
export function TextField({
  label,
  help,
  value,
  onValueChange,
  disabled,
  placeholder,
}: {
  label: string;
  help?: string;
  value: string;
  onValueChange(value: string): void;
  disabled?: boolean;
  placeholder?: string;
}) {
  return (
    <FieldFrame label={label} help={help}>
      {(control) => (
        <Input
          {...control}
          type="text"
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(event) => onValueChange(event.currentTarget.value)}
        />
      )}
    </FieldFrame>
  );
}
