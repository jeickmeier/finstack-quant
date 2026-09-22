"use client";
import { useRef, useImperativeHandle, type RefObject } from "react";
import {
  Combobox,
  ComboboxInput,
  ComboboxTrigger,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
  ComboboxEmpty,
} from "@/components/ui/combobox";
import { InputGroupAddon, InputGroupButton } from "@/components/ui/input-group";
import { useVirtualizer, type Virtualizer } from "@tanstack/react-virtual";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
type ListVirtualizer = Virtualizer<HTMLDivElement, Element>;
/** Free-text IDs with virtualized supplied suggestions composed from stock shadcn Combobox. */
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
  const filtered = options.filter((option) =>
    option.toLocaleLowerCase().includes(value.toLocaleLowerCase()),
  );
  return (
    <FieldFrame {...field}>
      {(control) => (
        <Combobox
          items={options}
          filteredItems={filtered}
          inputValue={value}
          value={options.includes(value) ? value : null}
          onInputValueChange={(next, event) => {
            if (
              event.reason === "input-change" ||
              event.reason === "input-clear" ||
              event.reason === "clear-press"
            )
              onValueChange(next);
          }}
          onValueChange={(next) => {
            if (next !== null) onValueChange(next);
          }}
          disabled={disabled}
          virtualized
          openOnInputClick
          onItemHighlighted={(_, event) => {
            if (event.reason === "keyboard")
              virtualizer.current?.scrollToIndex(event.index);
          }}
        >
          <ComboboxInput {...control} showTrigger={false} disabled={disabled}>
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                size="icon-xs"
                variant="ghost"
                render={<ComboboxTrigger />}
                aria-label={`${field.label} suggestions`}
                disabled={disabled}
              />
            </InputGroupAddon>
          </ComboboxInput>
          <ComboboxContent>
            <ComboboxEmpty>
              No matching IDs; free text is retained
            </ComboboxEmpty>
            <VirtualIds items={filtered} virtualizerRef={virtualizer} />
          </ComboboxContent>
        </Combobox>
      )}
    </FieldFrame>
  );
}
function VirtualIds({
  items,
  virtualizerRef,
}: {
  items: readonly string[];
  virtualizerRef: RefObject<ListVirtualizer | null>;
}) {
  const scroll = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 32,
    overscan: 5,
  });
  useImperativeHandle(virtualizerRef, () => virtualizer);
  return (
    <ComboboxList ref={scroll}>
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((row) => (
          <div
            key={items[row.index]}
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: "100%",
              transform: `translateY(${row.start}px)`,
            }}
            data-index={row.index}
            ref={virtualizer.measureElement}
          >
            <ComboboxItem
              value={items[row.index]}
              index={row.index}
              aria-setsize={items.length}
              aria-posinset={row.index + 1}
            >
              {items[row.index]}
            </ComboboxItem>
          </div>
        ))}
      </div>
    </ComboboxList>
  );
}
