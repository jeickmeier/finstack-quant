"use client";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { FinancialModelEditor } from "@/components/finstack/statements/components/financial-model-editor/financial-model-editor";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";

/** Advanced tab: load pasted financials, edit the model structure, and view the canonical model and native result. */
export function AdvancedPanel({
  document,
  resultJson,
  revision,
  importText,
  onImportTextChange,
  ready,
  onLoad,
  onCommit,
  density,
}: {
  /** Canonical model JSON now loaded. */
  document: string;
  /** Host-serialized StatementResult; null before an evaluation. */
  resultJson: string | null;
  /** Remounts the structure editor after each accepted model. */
  revision: number;
  /** Pasted model JSON, as typed. */
  importText: string;
  onImportTextChange(text: string): void;
  /** Whether the statement runtime is ready. */
  ready: boolean;
  /** Validates and evaluates the pasted model as a new document. */
  onLoad(): void;
  /** Validates and evaluates an edited model natively, keeping the line order. */
  onCommit(json: string): Promise<unknown>;
  density: "compact" | "comfortable";
}) {
  return (
    <div className="space-y-6">
      <div className="rounded-lg border border-border bg-card p-4">
        <h3 className="font-semibold">Load updated financials</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Paste the complete canonical FinancialModelSpec JSON returned by your
          data provider or parsing service.
        </p>
        <div className="mt-3">
          <FieldFrame label="Model JSON">
            {(control) => (
              <Textarea
                {...control}
                className="min-h-36 font-mono"
                value={importText}
                onChange={(event) => onImportTextChange(event.target.value)}
                spellCheck={false}
              />
            )}
          </FieldFrame>
        </div>
        <Button
          type="button"
          className="mt-3"
          disabled={!ready}
          onClick={onLoad}
        >
          Load & evaluate
        </Button>
      </div>
      <details>
        <summary>Model structure and formulas</summary>
        <div className="mt-4">
          <FinancialModelEditor
            key={revision}
            defaultJson={document}
            onSubmit={async (json) => {
              await onCommit(json);
            }}
          />
        </div>
      </details>
      <details>
        <summary>Canonical model and native result</summary>
        <div className="mt-3 space-y-4">
          <JsonViewer
            label="Canonical model JSON"
            text={document}
            downloadName="financial-model.json"
            density={density}
          />
          {resultJson && (
            <JsonViewer
              label="Native statement result"
              text={resultJson}
              downloadName="statement-result.json"
              density={density}
            />
          )}
        </div>
      </details>
    </div>
  );
}
