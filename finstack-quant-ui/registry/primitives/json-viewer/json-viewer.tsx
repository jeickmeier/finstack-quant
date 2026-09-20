"use client";
import { useState } from "react";
import { Button } from "@base-ui/react/button";
/** Read-only original supplied text. Copy is exact; print only changes layout. */
export function JsonViewer({
  text,
  label = "JSON",
  loading = false,
  error,
  density = "compact",
}: {
  text: string | null | undefined;
  label?: string;
  loading?: boolean;
  error?: string;
  density?: "compact" | "comfortable";
}) {
  const [copyStatus, setCopyStatus] = useState<{
    text: string | null | undefined;
    message: string;
  } | null>(null);
  const present = text !== null && text !== undefined && text !== "";
  return (
    <section
      aria-label={label}
      data-density={density}
      aria-busy={loading}
      className="font-sans text-base text-foreground"
    >
      <div className="flex items-center gap-2 print:hidden">
        <span className="text-sm">{label}</span>
        <Button
          disabled={!present}
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(text!);
              setCopyStatus({ text, message: "Copied" });
            } catch {
              setCopyStatus({ text, message: "Copy failed" });
            }
          }}
          className="rounded-sm border border-border px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
        >
          Copy
        </Button>
        <Button
          onClick={() => window.print()}
          className="rounded-sm border border-border px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
        >
          Print
        </Button>
        <span role="status" className="text-xs">
          {copyStatus && copyStatus.text === text ? copyStatus.message : ""}
        </span>
      </div>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      {loading && <p role="status">Loading…</p>}
      {present ? (
        <pre
          tabIndex={0}
          className={`max-h-96 overflow-auto whitespace-pre font-mono focus-visible:outline-2 focus-visible:outline-ring print:max-h-none print:whitespace-pre-wrap print:break-all print:overflow-visible ${density === "compact" ? "text-xs leading-5" : "text-sm leading-6"}`}
        >
          {text}
        </pre>
      ) : (
        !loading && <p className="text-muted-foreground">No JSON supplied</p>
      )}
    </section>
  );
}
