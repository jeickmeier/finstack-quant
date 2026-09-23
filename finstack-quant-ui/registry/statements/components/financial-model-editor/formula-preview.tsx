"use client";
import { useEffect, useState } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";

/** Native formula validation and opaque AST preview for a supplied text field. */
export function FormulaPreview({ formula }: { formula: string | null }) {
  const { client, status } = useFinstack();
  const [preview, setPreview] = useState<{
    formula: string;
    text?: string;
    error?: string;
  } | null>(null);
  useEffect(() => {
    if (!formula || !client || status !== "ready") return;
    let current = true;
    const timer = setTimeout(() => {
      void client.call("validateStatementFormula", formula).then(
        (text) => {
          if (current) setPreview({ formula, text });
        },
        (error) => {
          if (current)
            setPreview({
              formula,
              error: error instanceof Error ? error.message : String(error),
            });
        },
      );
    }, 250);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [formula, client, status]);
  if (!formula) return null;
  return (
    <div className="mt-1 text-xs text-muted-foreground" aria-live="polite">
      {status !== "ready" ? (
        <p>Formula preview waits for WASM</p>
      ) : preview?.formula !== formula ? (
        <p>Checking formula…</p>
      ) : preview.error ? (
        <p role="alert" className="text-error">
          {preview.error}
        </p>
      ) : (
        <details>
          <summary>Formula valid · native parse preview</summary>
          <pre className="max-h-40 overflow-auto whitespace-pre-wrap font-mono">
            {preview.text}
          </pre>
        </details>
      )}
    </div>
  );
}
