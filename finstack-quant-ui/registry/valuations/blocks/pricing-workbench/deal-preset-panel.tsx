"use client";
import { lazy, Suspense, useState } from "react";
// Lazy: the form links the shared instrument definitions, which this block otherwise loads per instrument.
const DealPresetForm = lazy(() => import("./deal-preset-form"));
/** Disclosure over the standard-deal form; opening it loads the form's contract. */
export function DealPresetPanel({ onDeal }: { onDeal(json: string): void }) {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="mb-3 text-sm"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary>Start from a standard deal</summary>
      {open && (
        <Suspense
          fallback={
            <p role="status" className="text-sm text-muted-foreground">
              Loading the deal form…
            </p>
          }
        >
          <DealPresetForm onDeal={onDeal} />
        </Suspense>
      )}
    </details>
  );
}
