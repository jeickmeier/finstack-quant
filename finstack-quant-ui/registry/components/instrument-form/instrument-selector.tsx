"use client";
import { useEffect, useId, useState } from "react";
import {
  Combobox,
  ComboboxInput,
  ComboboxTrigger,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
  ComboboxGroup,
  ComboboxLabel,
  ComboboxCollection,
  ComboboxEmpty,
} from "@/components/ui/combobox";
import { InputGroupAddon, InputGroupButton } from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
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
    <div>
      <Combobox
        items={groups}
        value={instruments.find((entry) => entry.type === value) ?? null}
        itemToStringLabel={(item: Entry) => item.title}
        onValueChange={(item) => {
          if (item) select(item.type);
        }}
        openOnInputClick
      >
        <Label htmlFor={id}>Instrument type</Label>
        <ComboboxInput
          id={id}
          placeholder="Search instruments…"
          showTrigger={false}
        >
          <InputGroupAddon align="inline-end">
            <InputGroupButton
              size="icon-xs"
              variant="ghost"
              render={<ComboboxTrigger />}
              aria-label="Show instrument types"
            />
          </InputGroupAddon>
        </ComboboxInput>

        <ComboboxContent>
          <ComboboxEmpty>No matching instrument</ComboboxEmpty>
          <ComboboxList>
            {(group: { value: string; items: Entry[] }) => (
              <ComboboxGroup key={group.value} items={group.items}>
                <ComboboxLabel>
                  {group.value.replaceAll("_", " ")}
                </ComboboxLabel>
                <ComboboxCollection>
                  {(entry: Entry) => (
                    <ComboboxItem key={entry.type} value={entry}>
                      <span>{entry.title}</span>
                      <span aria-hidden>
                        {entry.group.replaceAll("_", " ")}
                      </span>
                    </ComboboxItem>
                  )}
                </ComboboxCollection>
              </ComboboxGroup>
            )}
          </ComboboxList>
        </ComboboxContent>
      </Combobox>
      {recent.length > 0 && (
        <nav aria-label="Recently used instruments">
          <span>Recent</span>
          {recent.map((type) => (
            <Button
              variant="ghost"
              size="sm"
              key={type}
              type="button"
              onClick={() => select(type)}
            >
              {type}
            </Button>
          ))}
        </nav>
      )}
    </div>
  );
}
