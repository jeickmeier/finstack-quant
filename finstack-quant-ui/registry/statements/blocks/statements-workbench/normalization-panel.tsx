"use client";
import { useState } from "react";
import { NormalizationEditor } from "@/components/finstack/statements/components/normalization-editor/normalization-editor";
import { useNormalizationConfigValidator } from "@/hooks/statements/use-normalization-config-validator/use-normalization-config-validator";
import { useNormalize } from "@/hooks/statements/use-normalize/use-normalize";

function NormalizationBody({
  resultJson,
  nodeIds,
  density,
}: {
  resultJson: string;
  nodeIds: readonly string[];
  density: "compact" | "comfortable";
}) {
  const validate = useNormalizationConfigValidator();
  const [configJson, setConfigJson] = useState<string | null>(null);
  const normalized = useNormalize(
    configJson === null ? null : { resultJson, configJson },
  );
  return (
    <NormalizationEditor
      nodeIds={nodeIds}
      validate={validate}
      onSubmit={setConfigJson}
      results={configJson === null ? null : normalized.data}
      loading={normalized.isFetching}
      error={configJson === null ? undefined : normalized.error?.message}
      density={density}
    />
  );
}
/**
 * Adjusted EBITDA (or any target node) of the evaluated result: a native
 * NormalizationConfig editor and the statements.normalize bridge, in a
 * disclosure whose native calls run only once it is opened.
 */
export function NormalizationPanel({
  resultJson,
  nodeIds,
  density,
}: {
  /** Host-serialized StatementResult the workbench evaluated. */
  resultJson: string;
  /** The model's node ids, offered for the target and reference nodes. */
  nodeIds: readonly string[];
  density: "compact" | "comfortable";
}) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>Adjusted EBITDA and normalization</summary>
      {open && (
        <div className="mt-3">
          <NormalizationBody
            resultJson={resultJson}
            nodeIds={nodeIds}
            density={density}
          />
        </div>
      )}
    </details>
  );
}
