"use client";
import { useCallback, useMemo, useState } from "react";

/** Accepted semantic identity shared by controls. Focus and activation are separate. */
export interface LinkedSelection {
  readonly selectedKey: string | null;
  /** Propose a stable key or clear it; the controlled parent decides acceptance. */
  select(key: string | null): void;
  clear(): void;
}
export interface LinkedSelectionOptions {
  /** Supply null for a controlled empty selection; omit for local ownership. */
  selectedKey?: string | null;
  /** Initial accepted key for local ownership. Later changes are ignored. */
  defaultSelectedKey?: string | null;
  /** Called for a changed proposal. Do not write selection again from chart onSelect. */
  onSelectedKeyChange?: (key: string | null) => void;
}
/** React-only accepted selection. Data removal does not silently rewrite the parent's key. */
export function useLinkedSelection(
  options: LinkedSelectionOptions = {},
): LinkedSelection {
  const [local, setLocal] = useState(options.defaultSelectedKey ?? null);
  const controlled = options.selectedKey !== undefined;
  const selectedKey = controlled ? options.selectedKey! : local;
  const onChange = options.onSelectedKeyChange;
  const select = useCallback(
    (key: string | null) => {
      if (key === selectedKey) return;
      if (!controlled) setLocal(key);
      onChange?.(key);
    },
    [controlled, selectedKey, onChange],
  );
  const clear = useCallback(() => select(null), [select]);
  return useMemo(
    () => ({ selectedKey, select, clear }),
    [selectedKey, select, clear],
  );
}
