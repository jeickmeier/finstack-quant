"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  useLinkedSelection,
  type LinkedSelection,
} from "@/hooks/use-linked-selection/use-linked-selection";
import { StoredCurveView } from "../curve-chart/stored-curve-view";
import {
  VolSurfaceChart,
  type VolSurfaceChartProps,
} from "../vol-surface-chart/vol-surface-chart";
import { FxDeltaQuotes } from "../fx-delta-quotes/fx-delta-quotes";
import { FxMatrixGrid } from "../fx-matrix-grid/fx-matrix-grid";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import {
  marketTree,
  flattenMarket,
  marketPointer,
  type MarketEntry,
} from "./tree";
export { marketTree, flattenMarket, marketPointer } from "./tree";
export interface MarketContextBrowserProps {
  /** Complete canonical validated state. Data remains read-only until a parent accepts native validation. */
  state: MarketContextStateWire;
  link?: LinkedSelection;
  /** Optional explicit color extents for stored surfaces; no financial units or ranges are inferred. */
  surfaceOptions?(
    surface: MarketContextStateWire["surfaces"][number],
  ): Pick<VolSurfaceChartProps, "colorDomain"> | undefined;
  /** Host-owned evaluated views require explicit strike/forward/convention inputs and a provider. */
  renderObject?(entry: MarketEntry): ReactNode | undefined;
}
function EntryBranch({
  entry,
  visible,
  selectedKey,
  onSelect,
  searching,
}: {
  entry: MarketEntry;
  visible: Set<string> | null;
  selectedKey: string | null;
  onSelect: (key: string) => void;
  searching: boolean;
}) {
  const [open, setOpen] = useState(false);
  if (visible && !visible.has(entry.key)) return null;
  const button = (
    <button
      type="button"
      aria-pressed={entry.key === selectedKey}
      onClick={() => onSelect(entry.key)}
      className="rounded-sm px-1 text-left focus-visible:outline-2 focus-visible:outline-ring"
      aria-label={`Inspect ${marketPointer(entry.path)}`}
    >
      {entry.label}
      {!entry.children.length
        ? `: ${entry.value === undefined ? "Unavailable" : serializeHost(entry.value)}`
        : ""}
    </button>
  );
  const expanded = searching || open;
  return (
    <li>
      <div className="flex items-start">
        {entry.children.length > 0 && (
          <button
            type="button"
            aria-expanded={expanded}
            aria-label={`${expanded ? "Collapse" : "Expand"} ${marketPointer(entry.path)}`}
            disabled={searching}
            onClick={() => setOpen(!open)}
            className="rounded-sm px-1 focus-visible:outline-2 focus-visible:outline-ring"
          >
            {expanded ? "−" : "+"}
          </button>
        )}
        {button}
      </div>
      {expanded && entry.children.length > 0 && (
        <ul className="ml-4 space-y-1">
          {entry.children.map((child) => (
            <EntryBranch
              key={child.key}
              entry={child}
              visible={visible}
              selectedKey={selectedKey}
              onSelect={onSelect}
              searching={searching}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
/** Searchable native disclosure tree over the generated contract and actual state, with exact selected exports. */
export function MarketContextBrowser({
  state,
  link: external,
  surfaceOptions,
  renderObject,
}: MarketContextBrowserProps) {
  const owned = useLinkedSelection(),
    link = external ?? owned;
  const [search, setSearch] = useState("");
  const entries = useMemo(() => marketTree(state), [state]);
  const all = useMemo(() => flattenMarket(entries), [entries]);
  const index = useMemo(
    () => ({
      keys: new Map(all.map((entry) => [entry.key, entry])),
      paths: new Map(all.map((entry) => [JSON.stringify(entry.path), entry])),
    }),
    [all],
  );
  const selected = link.selectedKey
    ? index.keys.get(link.selectedKey)
    : undefined;
  const query = search.toLowerCase().trim();
  const visible = useMemo(() => {
    if (!query) return null;
    const keys = new Set<string>();
    const walk = (entry: MarketEntry): boolean => {
      const children = entry.children.map(walk).some(Boolean);
      const own =
        `${marketPointer(entry.path)} ${entry.key} ${entry.label} ${entry.children.length ? "" : entry.value === undefined ? "Unavailable" : serializeHost(entry.value)}`
          .toLowerCase()
          .includes(query);
      if (own || children) keys.add(entry.key);
      return own || children;
    };
    entries.forEach(walk);
    return keys;
  }, [entries, query]);
  let view: ReactNode;
  if (selected) {
    const [field, position] = selected.path;
    const owner =
      typeof position === "number"
        ? index.paths.get(JSON.stringify([field, position]))
        : selected;
    view = owner ? renderObject?.(owner) : undefined;
    if (view === undefined) {
      if (field === "curves" && typeof position === "number")
        view = (
          <StoredCurveView
            curves={[state.curves[position]!]}
            ariaLabel="Selected market curve"
          />
        );
      else if (field === "fx") view = <FxMatrixGrid state={state.fx} />;
      else if (
        field === "fx_delta_vol_surfaces" &&
        typeof position === "number"
      )
        view = (
          <FxDeltaQuotes surface={state.fx_delta_vol_surfaces[position]!} />
        );
      else if (field === "surfaces" && typeof position === "number") {
        const surface = state.surfaces[position]!,
          options = surfaceOptions?.(surface);
        view = options ? (
          <VolSurfaceChart surface={surface} {...options} />
        ) : (
          <JsonViewer
            label="Stored surface state"
            text={serializeHost(surface)}
          />
        );
      } else if (field === "vol_cubes" && typeof position === "number")
        view = (
          <JsonViewer
            label="Stored cube state"
            text={serializeHost(state.vol_cubes[position])}
          />
        );
    }
  }
  return (
    <section
      aria-label="Market context browser"
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <label>
        Search market fields
        <input
          type="search"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          className="ml-2 rounded-sm border border-border bg-background px-2"
        />
      </label>
      <nav
        aria-label="Market fields"
        className="max-h-80 overflow-auto rounded-sm border border-border p-2"
      >
        <ul className="space-y-1">
          {entries.map((entry) => (
            <EntryBranch
              key={entry.key}
              entry={entry}
              visible={visible}
              selectedKey={selected?.key ?? null}
              onSelect={link.select}
              searching={!!query}
            />
          ))}
        </ul>
      </nav>
      {selected ? (
        <section aria-label="Selected market field">
          <h2>{marketPointer(selected.path)}</h2>
          {selected.value === undefined ? (
            <p>Field unavailable in supplied state</p>
          ) : (
            <JsonViewer
              label="Selected stored value"
              text={serializeHost(selected.value)}
            />
          )}
          {view}
        </section>
      ) : (
        <p>
          {link.selectedKey
            ? "Selected field is no longer available"
            : "Select a market field"}
        </p>
      )}
    </section>
  );
}
