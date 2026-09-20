"use client";
import { useRef, useImperativeHandle, type RefObject } from "react";
import { Autocomplete } from "@base-ui/react/autocomplete";
import { useVirtualizer, type Virtualizer } from "@tanstack/react-virtual";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
type ListVirtualizer = Virtualizer<HTMLDivElement, Element>;
/** Free-text ID input with virtualized supplied suggestions; no market lookup or ID inference. */
export function IdCombobox({
  value,
  onValueChange,
  options,
  disabled,
  ...field
}: FieldInfo & {
  value: string;
  onValueChange: (value: string) => void;
  options: readonly string[];
  disabled?: boolean;
}) {
  const virtualizer = useRef<ListVirtualizer | null>(null);
  return (
    <FieldFrame {...field}>
      {(control) => (
        <Autocomplete.Root
          items={options}
          value={value}
          onValueChange={(next) => onValueChange(next)}
          disabled={disabled}
          virtualized
          openOnInputClick
          onItemHighlighted={(_, event) => {
            if (event.reason === "keyboard")
              virtualizer.current?.scrollToIndex(event.index);
          }}
        >
          <Autocomplete.Input
            {...control}
            className="finstack-field w-full rounded-sm border border-control-border bg-background px-2 font-mono text-sm focus-visible:outline-2 focus-visible:outline-ring"
          />
          <Autocomplete.Portal>
            <Autocomplete.Positioner sideOffset={4}>
              <Autocomplete.Popup className="min-w-[var(--anchor-width)] rounded-md border border-border bg-card text-card-foreground shadow-[var(--elevation)]">
                <Autocomplete.Empty className="p-2 text-xs text-muted-foreground">
                  No matching IDs; free text is retained
                </Autocomplete.Empty>
                <VirtualIds virtualizerRef={virtualizer} />
              </Autocomplete.Popup>
            </Autocomplete.Positioner>
          </Autocomplete.Portal>
        </Autocomplete.Root>
      )}
    </FieldFrame>
  );
}
function VirtualIds({
  virtualizerRef,
}: {
  virtualizerRef: RefObject<ListVirtualizer | null>;
}) {
  const items = Autocomplete.useFilteredItems<string>();
  const scroll = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroll.current,
    estimateSize: () =>
      Number.parseFloat(
        getComputedStyle(
          scroll.current ?? document.documentElement,
        ).getPropertyValue("--row-height"),
      ),
    overscan: 5,
  });
  useImperativeHandle(virtualizerRef, () => virtualizer);
  return (
    <div ref={scroll} className="h-60 overflow-auto">
      <Autocomplete.List>
        <div
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((row) => (
            <Autocomplete.Item
              key={items[row.index]}
              value={items[row.index]}
              index={row.index}
              aria-setsize={items.length}
              aria-posinset={row.index + 1}
              className="finstack-row flex items-center px-2 font-mono text-sm data-highlighted:bg-accent data-highlighted:text-accent-foreground"
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: row.size,
                transform: `translateY(${row.start}px)`,
              }}
            >
              {items[row.index]}
            </Autocomplete.Item>
          ))}
        </div>
      </Autocomplete.List>
    </div>
  );
}
