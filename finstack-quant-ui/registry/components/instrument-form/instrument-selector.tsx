"use client";
import { useEffect, useId, useState } from "react";
import { Combobox } from "@base-ui/react/combobox";
import { instruments } from "@/lib/finstack/generated/instruments";
type Entry = (typeof instruments)[number];
const groups = [...Map.groupBy(instruments, (entry) => entry.group)].map(
  ([value, items]) => ({ value, items }),
);
const storageKey = "finstack.recent-instruments";
/** Canonical choices only; storage contains at most six known type identifiers. */
export function InstrumentSelector({
  value,
  onValueChange,
}: {
  value: string;
  onValueChange(value: string): void;
}) {
  const id = useId();
  const [recent, setRecent] = useState<string[]>([]);
  useEffect(() => {
    try {
      const stored: unknown = JSON.parse(
        localStorage.getItem(storageKey) ?? "[]",
      );
      if (Array.isArray(stored))
        setRecent(
          [
            ...new Set(
              stored.filter(
                (type): type is string =>
                  typeof type === "string" &&
                  instruments.some((entry) => entry.type === type),
              ),
            ),
          ].slice(0, 6),
        );
    } catch {
      /* Storage is optional, including restricted browser contexts. */
    }
  }, []);
  const select = (type: string) => {
    const next = [type, ...recent.filter((entry) => entry !== type)].slice(
      0,
      6,
    );
    setRecent(next);
    try {
      localStorage.setItem(storageKey, JSON.stringify(next));
    } catch {
      /* Selection remains usable without persistence. */
    }
    onValueChange(type);
  };
  return (
    <div className="space-y-2">
      <Combobox.Root
        items={groups}
        value={instruments.find((entry) => entry.type === value) ?? null}
        itemToStringLabel={(item: Entry) => item.title}
        onValueChange={(item) => {
          if (item) select(item.type);
        }}
        openOnInputClick
      >
        <label htmlFor={id} className="block text-sm">
          Instrument type
        </label>
        <Combobox.Input
          id={id}
          placeholder="Search instruments…"
          className="finstack-field w-full rounded-sm border border-border bg-background px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
        />
        <Combobox.Portal>
          <Combobox.Positioner sideOffset={4}>
            <Combobox.Popup className="min-w-[var(--anchor-width)] rounded-md border border-border bg-card text-card-foreground shadow-[var(--elevation)]">
              <Combobox.Empty className="p-2 text-sm">
                No matching instrument
              </Combobox.Empty>
              <Combobox.List className="max-h-72 overflow-auto p-1">
                {(group: { value: string; items: Entry[] }) => (
                  <Combobox.Group key={group.value} items={group.items}>
                    <Combobox.GroupLabel className="p-2 text-xs text-muted-foreground">
                      {group.value.replaceAll("_", " ")}
                    </Combobox.GroupLabel>
                    <Combobox.Collection>
                      {(entry: Entry) => (
                        <Combobox.Item
                          key={entry.type}
                          value={entry}
                          className="flex items-center justify-between gap-4 rounded-sm p-2 text-sm data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                        >
                          <span>{entry.title}</span>
                          <span
                            aria-hidden
                            className="rounded-sm border border-border px-1 text-xs text-muted-foreground"
                          >
                            {entry.group.replaceAll("_", " ")}
                          </span>
                        </Combobox.Item>
                      )}
                    </Combobox.Collection>
                  </Combobox.Group>
                )}
              </Combobox.List>
            </Combobox.Popup>
          </Combobox.Positioner>
        </Combobox.Portal>
      </Combobox.Root>
      {recent.length > 0 && (
        <nav
          aria-label="Recently used instruments"
          className="flex flex-wrap items-center gap-2 text-xs"
        >
          <span className="text-muted-foreground">Recent</span>
          {recent.map((type) => (
            <button
              key={type}
              type="button"
              onClick={() => select(type)}
              className="rounded-sm border border-border px-2 py-1 focus-visible:outline-2 focus-visible:outline-ring"
            >
              {type}
            </button>
          ))}
        </nav>
      )}
    </div>
  );
}
