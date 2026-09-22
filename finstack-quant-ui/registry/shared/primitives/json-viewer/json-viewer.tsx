"use client";
import { useMemo, useState } from "react";
import { parse, stringify } from "lossless-json";
import { Button } from "@/components/ui/button";
/** Read-only JSON with lossless formatted or original presentation.
 * Formatting preserves numeric tokens; invalid JSON (including duplicate keys)
 * remains in Original view. Copy, download and compact print retain the supplied bytes.
 */
export function JsonViewer({
  text,
  label = "JSON",
  loading = false,
  error,
  downloadName,
  density = "compact",
}: {
  text: string | null | undefined;
  label?: string;
  loading?: boolean;
  error?: string;
  /** Optional filename for downloading the exact supplied text without reserialization. */
  downloadName?: string;
  density?: "compact" | "comfortable";
}) {
  const [copyStatus, setCopyStatus] = useState<{
    text: string | null | undefined;
    message: string;
  } | null>(null);
  const [view, setView] = useState<"formatted" | "original">("formatted");
  const presentation = useMemo(() => {
    if (!text) return { formatted: null, error: null };
    try {
      return {
        formatted: stringify(parse(text), undefined, 2) ?? null,
        error: null,
      };
    } catch (error) {
      return {
        formatted: null,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }, [text]);
  const formatted = view === "formatted" && presentation.formatted !== null;
  const present = text !== null && text !== undefined && text !== "";
  return (
    <section
      aria-label={label}
      data-density={density}
      aria-busy={loading}
      className="font-sans text-base text-foreground"
    >
      <h3 className="hidden text-sm font-semibold print:block">{label}</h3>
      <div className="flex flex-wrap items-center gap-2 print:hidden">
        <span className="text-sm font-medium">{label}</span>
        <div role="group" aria-label="JSON presentation" className="flex gap-1">
          <Button
            aria-pressed={formatted}
            disabled={presentation.formatted === null}
            onClick={() => setView("formatted")}
            variant="ghost"
            size="sm"
          >
            Formatted
          </Button>
          <Button
            aria-pressed={!formatted}
            onClick={() => setView("original")}
            variant="ghost"
            size="sm"
          >
            Original
          </Button>
        </div>
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
          variant="outline"
          size="sm"
        >
          Copy
        </Button>
        <Button onClick={() => window.print()} variant="outline" size="sm">
          Print
        </Button>
        {downloadName && (
          <Button
            disabled={!present}
            onClick={() => {
              const url = URL.createObjectURL(
                new Blob([text!], { type: "application/json;charset=utf-8" }),
              );
              const anchor = document.createElement("a");
              anchor.href = url;
              anchor.download = downloadName;
              anchor.click();
              setTimeout(() => URL.revokeObjectURL(url), 1000);
            }}
            variant="outline"
            size="sm"
          >
            Download
          </Button>
        )}
        <span role="status" className="text-xs">
          {copyStatus && copyStatus.text === text ? copyStatus.message : ""}
        </span>
      </div>
      {present && presentation.error && (
        <p className="text-xs text-muted-foreground" role="status">
          Original text — formatting unavailable: {presentation.error}
        </p>
      )}
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      {loading && <p role="status">Loading…</p>}
      {present ? (
        <>
          <pre
            tabIndex={0}
            className={`mt-2 max-h-96 overflow-auto rounded-md border border-border bg-card p-2 whitespace-pre-wrap break-words font-mono focus-visible:outline-2 focus-visible:outline-ring print:hidden ${density === "compact" ? "text-xs leading-5" : "text-sm leading-6"}`}
          >
            {formatted ? presentation.formatted : text}
          </pre>
          <code
            data-json-print-source
            className="hidden whitespace-pre-wrap break-all font-mono text-xs leading-5 print:block"
          >
            {text}
          </code>
        </>
      ) : (
        !loading && <p className="text-muted-foreground">No JSON supplied</p>
      )}
    </section>
  );
}
