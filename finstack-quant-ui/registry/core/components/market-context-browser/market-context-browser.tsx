"use client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useMemo, useState, type ReactNode } from "react";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  useLinkedSelection,
  type LinkedSelection,
} from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { StoredCurveView } from "../curve-chart/stored-curve-view";
import {
  VolSurfaceChart,
  type VolSurfaceChartProps,
} from "../vol-surface-chart/vol-surface-chart";
import { FxDeltaQuotes } from "../fx-delta-quotes/fx-delta-quotes";
import { FxMatrixGrid } from "../fx-matrix-grid/fx-matrix-grid";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
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

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function entryName(entry: MarketEntry) {
  const id = record(entry.value)?.id;
  if (typeof id === "string") return id;
  if (entry.path.length === 1 && entry.path[0] === "fx") return "FX matrix";
  return entry.label.replaceAll("_", " ");
}

function entryKind(entry: MarketEntry) {
  const type = record(entry.value)?.type;
  const category = String(entry.path[0]).replaceAll("_", " ");
  return typeof type === "string"
    ? `${type.replaceAll("_", " ")} ${category === "curves" ? "curve" : category}`
    : category;
}

/** One row per stored reference; the complete schema tree remains available below it. */
function marketReferences(entries: readonly MarketEntry[]) {
  return entries.flatMap((root) => {
    if (root.path[0] === "schema_version" || root.value == null) return [];
    if (root.path[0] === "fx" || root.path[0] === "hierarchy") return [root];
    return root.children;
  });
}

function entryLabel(entry: MarketEntry) {
  return `Inspect ${entryName(entry)}, ${marketPointer(entry.path)}`;
}

function EntryBranch({
  entry,
  selectedKey,
  onSelect,
}: {
  entry: MarketEntry;
  selectedKey: string | null;
  onSelect: (key: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <li
      className={
        entry.path.length === 1 ? "finstack-market__category" : undefined
      }
    >
      <div className="flex items-start">
        {entry.children.length > 0 && (
          <Button
            variant="ghost"
            size="icon-sm"
            type="button"
            aria-expanded={open}
            aria-label={`${open ? "Collapse" : "Expand"} ${entryName(entry)}, ${marketPointer(entry.path)}`}
            onClick={() => setOpen(!open)}
          >
            {open ? "−" : "+"}
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          type="button"
          aria-pressed={entry.key === selectedKey}
          aria-label={entryLabel(entry)}
          onClick={() => onSelect(entry.key)}
        >
          {entryName(entry)}
          {entry.path.length === 1 && (
            <span className="ml-2 font-mono text-xs text-muted-foreground">
              {entry.children.length}
            </span>
          )}
          {entry.path.length > 1 && !entry.children.length
            ? `: ${entry.value === undefined ? "Unavailable" : serializeHost(entry.value)}`
            : ""}
        </Button>
      </div>
      {open && entry.children.length > 0 && (
        <ul className="ml-4 space-y-1">
          {entry.children.map((child) => (
            <EntryBranch
              key={child.key}
              entry={child}
              selectedKey={selectedKey}
              onSelect={onSelect}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

function ReferenceButton({
  entry,
  selected,
  onSelect,
}: {
  entry: MarketEntry;
  selected: boolean;
  onSelect: (key: string) => void;
}) {
  const base = record(entry.value)?.base;
  return (
    <li>
      <Button
        type="button"
        variant="ghost"
        aria-pressed={selected}
        aria-label={entryLabel(entry)}
        className="finstack-market__reference"
        onClick={() => onSelect(entry.key)}
      >
        <span className="finstack-market__reference-name">
          {entryName(entry)}
        </span>
        <span className="finstack-market__reference-detail">
          {entryKind(entry)}
          {typeof base === "string" ? ` · base ${base}` : ""}
        </span>
      </Button>
    </li>
  );
}

/** Compact stored-reference list with an optional exact-field explorer and exact JSON values. */
export function MarketContextBrowser({
  state,
  link: external,
  surfaceOptions,
  renderObject,
}: MarketContextBrowserProps) {
  const entries = useMemo(() => marketTree(state), [state]);
  const references = useMemo(() => marketReferences(entries), [entries]);
  const first =
    references.find(
      (entry) =>
        entry.path[0] === "curves" && record(entry.value)?.type === "discount",
    ) ?? references[0];
  const owned = useLinkedSelection({ defaultSelectedKey: first?.key ?? null });
  const link = external ?? owned;
  const [search, setSearch] = useState("");
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
  const matches = useMemo(() => {
    if (!query) return references;
    const exactPath = all.find(
      (entry) => marketPointer(entry.path).toLowerCase() === query,
    );
    if (exactPath) return [exactPath];
    const referenceMatches = references.filter((entry) =>
      `${entryName(entry)} ${entryKind(entry)} ${marketPointer(entry.path)}`
        .toLowerCase()
        .includes(query),
    );
    if (referenceMatches.length) return referenceMatches;
    return all.filter(
      (entry) =>
        !entry.children.length &&
        `${entryName(entry)} ${marketPointer(entry.path)} ${entry.value === undefined ? "Unavailable" : serializeHost(entry.value)}`
          .toLowerCase()
          .includes(query),
    );
  }, [all, query, references]);
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
            height={230}
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
  const stored = selected ? record(selected.value) : null;
  const conventions = [
    ["Base", stored?.base ?? stored?.base_date],
    ["Day count", stored?.day_count],
    ["Interpolation", stored?.interp_style],
    ["Extrapolation", stored?.extrapolation],
  ].filter((item): item is [string, string] => typeof item[1] === "string");
  return (
    <section
      aria-label="Market context browser"
      className="finstack-market font-sans text-sm text-foreground"
    >
      <div className="finstack-market__layout">
        <aside className="finstack-market__rail print:hidden">
          <label className="mb-2 grid gap-1">
            Search market data
            <Input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <div className="finstack-market__list-heading">
            <span>Stored references</span>
            <span role="status">{matches.length} shown</span>
          </div>
          <nav aria-label="Market references">
            {matches.length ? (
              <ul className="finstack-market__references">
                {matches.map((entry) => (
                  <ReferenceButton
                    key={entry.key}
                    entry={entry}
                    selected={entry.key === selected?.key}
                    onSelect={link.select}
                  />
                ))}
              </ul>
            ) : (
              <p role="status" className="py-3 text-muted-foreground">
                {query
                  ? `No market data matches “${search.trim()}”.`
                  : "No stored market references in this snapshot."}
              </p>
            )}
          </nav>
          <details className="finstack-market__all-fields">
            <summary>All snapshot fields</summary>
            <nav aria-label="All market fields">
              <ul>
                {entries.map((entry) => (
                  <EntryBranch
                    key={entry.key}
                    entry={entry}
                    selectedKey={selected?.key ?? null}
                    onSelect={link.select}
                  />
                ))}
              </ul>
            </nav>
          </details>
        </aside>
        {selected ? (
          <section
            aria-label="Selected market field"
            className="finstack-market__selected"
          >
            <header>
              <div>
                <h2>{entryName(selected)}</h2>
                <p className="text-xs text-muted-foreground">
                  {entryKind(selected)} · {marketPointer(selected.path)}
                </p>
              </div>
              <span className="text-xs text-muted-foreground">
                Stored snapshot
              </span>
            </header>
            {conventions.length > 0 && (
              <dl className="finstack-market__conventions">
                {conventions.map(([label, value]) => (
                  <div key={label}>
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
            )}
            <p className="finstack-market__provenance">
              Snapshot provider and observation time not supplied
            </p>
            {view}
            {selected.value === undefined ? (
              <p>Field unavailable in supplied state</p>
            ) : view ? (
              <details className="mt-3">
                <summary className="cursor-pointer text-xs text-muted-foreground">
                  Selected stored value · JSON
                </summary>
                <JsonViewer
                  label="Selected stored value"
                  text={serializeHost(selected.value)}
                />
              </details>
            ) : (
              <JsonViewer
                label="Selected stored value"
                text={serializeHost(selected.value)}
              />
            )}
          </section>
        ) : (
          <p className="print:hidden">
            {link.selectedKey
              ? "Selected field is no longer available"
              : "Select a market reference"}
          </p>
        )}
      </div>
    </section>
  );
}
