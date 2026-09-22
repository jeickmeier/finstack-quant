"use client";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
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
function EntryBranch({
  entry,
  visible,
  selectedKey,
  onSelect,
  searching,
  activeCategory,
}: {
  entry: MarketEntry;
  visible: Set<string> | null;
  selectedKey: string | null;
  onSelect: (key: string) => void;
  searching: boolean;
  activeCategory?: string | number;
}) {
  const [open, setOpen] = useState(entry.path.length === 1);
  if (visible && !visible.has(entry.key)) return null;
  const button = (
    <Button
      variant="ghost"
      size="sm"
      type="button"
      aria-pressed={entry.key === selectedKey}
      onClick={() => onSelect(entry.key)}

      aria-label={`Inspect ${marketPointer(entry.path)}`}
    >
      {entry.path.length === 1
        ? entry.label.replaceAll("_", " ")
        : entry.value && typeof entry.value === "object" && "id" in entry.value
          ? String(entry.value.id)
          : entry.label}
      {entry.path.length === 1 && (
        <span className="ml-2 font-mono text-xs text-muted-foreground">
          {entry.children.length}
        </span>
      )}
      {entry.path.length > 1 && !entry.children.length
        ? `: ${entry.value === undefined ? "Unavailable" : serializeHost(entry.value)}`
        : ""}
    </Button>
  );
  const expanded = searching || open;
  return (
    <li
      data-active-category={entry.path[0] === activeCategory}
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
            aria-expanded={expanded}
            aria-label={`${expanded ? "Collapse" : "Expand"} ${marketPointer(entry.path)}`}
            disabled={searching}
            onClick={() => setOpen(!open)}
          >
            {expanded ? "−" : "+"}
          </Button>
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
              activeCategory={activeCategory}
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
  const entries = useMemo(() => marketTree(state), [state]);
  const owned = useLinkedSelection({
      defaultSelectedKey:
        entries.find((entry) => entry.children.length)?.children[0]?.key ??
        null,
    }),
    link = external ?? owned;
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
  return (
    <section
      aria-label="Market context browser"
      className="finstack-market font-sans text-sm text-foreground"
    >
      <div className="finstack-market__layout">
        <aside className="finstack-market__rail print:hidden">
          <Label className="mb-3 grid gap-2">
            Search market fields
            <Input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </Label>
          <Label className="finstack-market__category-picker mb-3">
            Market category
            <Select
              items={entries.map((entry) => ({
                value: String(entry.path[0]),
                label: `${entry.label.replaceAll("_", " ")} · ${entry.children.length}`,
              }))}
              value={selected ? String(selected.path[0]) : null}
              onValueChange={(value) => {
                const category = entries.find(
                  (entry) => String(entry.path[0]) === value,
                );
                if (category)
                  link.select(category.children[0]?.key ?? category.key);
              }}
            >
              <SelectTrigger className="w-full" aria-label="Market category">
                <SelectValue placeholder="Choose a category" />
              </SelectTrigger>
              <SelectContent>
                {entries.map((entry) => (
                  <SelectItem key={entry.key} value={String(entry.path[0])}>
                    {entry.label.replaceAll("_", " ")} · {entry.children.length}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Label>
          <nav aria-label="Market fields" data-searching={!!query}>
            <ul>
              {entries.map((entry) => (
                <EntryBranch
                  key={entry.key}
                  entry={entry}
                  visible={visible}
                  selectedKey={selected?.key ?? null}
                  onSelect={link.select}
                  searching={!!query}
                  activeCategory={selected?.path[0]}
                />
              ))}
            </ul>
          </nav>
        </aside>
        {selected ? (
          <section
            aria-label="Selected market field"
            className="finstack-market__selected"
          >
            <header>
              <div>
                <h2>
                  {typeof selected.value === "object" &&
                  selected.value !== null &&
                  "id" in selected.value
                    ? String(selected.value.id)
                    : selected.label.replaceAll("_", " ")}
                </h2>
                <p className="text-xs text-muted-foreground">
                  {marketPointer(selected.path)}
                </p>
              </div>
              <span className="text-xs text-muted-foreground">
                Stored market data
              </span>
            </header>
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
              : "Select a market field"}
          </p>
        )}
      </div>
    </section>
  );
}
