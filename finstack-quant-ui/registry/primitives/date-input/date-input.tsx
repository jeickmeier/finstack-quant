"use client";
import { useState } from "react";
import { Input } from "@base-ui/react/input";
import { Popover } from "@base-ui/react/popover";
import { DayPicker } from "@daypicker/react";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
/** ISO text plus a calendar presentation; native date validation/business-day results are supplied by the host. */
export function DateInput({
  value,
  onValueChange,
  min,
  max,
  disabled,
  businessDays,
  ...field
}: FieldInfo & {
  value: string;
  onValueChange: (value: string) => void;
  min?: string;
  max?: string;
  disabled?: boolean;
  businessDays?: readonly string[];
}) {
  const [open, setOpen] = useState(false);
  const parse = (text: string | undefined) => {
    if (!text) return undefined;
    const date = new Date(`${text}T12:00:00Z`);
    return Number.isNaN(date.valueOf()) ||
      date.toISOString().slice(0, 10) !== text
      ? undefined
      : date;
  };
  const selected = parse(value);
  const iso = (date: Date) =>
    `${String(date.getUTCFullYear()).padStart(4, "0")}-${String(date.getUTCMonth() + 1).padStart(2, "0")}-${String(date.getUTCDate()).padStart(2, "0")}`;
  const range = [
    ...(min ? [{ before: parse(min)! }] : []),
    ...(max ? [{ after: parse(max)! }] : []),
  ];
  return (
    <FieldFrame {...field}>
      {(control) => (
        <div className="flex items-center gap-1">
          <Input
            {...control}
            value={value}
            onValueChange={(next) => onValueChange(next)}
            placeholder="YYYY-MM-DD"
            min={min}
            max={max}
            disabled={disabled}
            className="finstack-field finstack-numeric w-full rounded-sm border border-control-border bg-background px-2 focus-visible:outline-2 focus-visible:outline-ring"
          />
          <Popover.Root open={open} onOpenChange={setOpen}>
            <Popover.Trigger
              disabled={disabled}
              aria-label={`${field.label} calendar`}
              className="finstack-field rounded-sm border border-control-border px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
            >
              <svg
                aria-hidden="true"
                viewBox="0 0 16 16"
                className="size-3.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
              >
                <rect x="2.5" y="3.5" width="11" height="10" rx="1" />
                <path d="M5 2v3M11 2v3M3 7h10" />
              </svg>
            </Popover.Trigger>
            <Popover.Portal>
              <Popover.Positioner sideOffset={4}>
                <Popover.Popup className="rounded-md border border-control-border bg-card p-3 text-card-foreground shadow-[var(--elevation)]">
                  <Popover.Title className="text-sm">
                    {field.label}
                  </Popover.Title>
                  <DayPicker
                    mode="single"
                    timeZone="UTC"
                    selected={selected}
                    defaultMonth={selected}
                    onSelect={(date) => {
                      if (date) onValueChange(iso(date));
                      setOpen(false);
                    }}
                    disabled={range}
                    modifiers={{
                      businessDay:
                        businessDays
                          ?.map((text) => parse(text)!)
                          .filter(Boolean) ?? [],
                    }}
                    modifiersClassNames={{ businessDay: "underline" }}
                    classNames={{
                      root: "text-sm",
                      months: "relative",
                      month_caption: "flex justify-center p-2 font-medium",
                      nav: "flex justify-between",
                      button_previous:
                        "rounded-sm border border-control-border px-2",
                      button_next:
                        "rounded-sm border border-control-border px-2",
                      weekday: "text-xs text-muted-foreground",
                      day_button:
                        "finstack-field min-w-8 rounded-sm focus-visible:outline-2 focus-visible:outline-ring",
                      selected: "bg-primary text-primary-foreground",
                      disabled: "opacity-50",
                      outside: "text-muted-foreground",
                    }}
                  />
                </Popover.Popup>
              </Popover.Positioner>
            </Popover.Portal>
          </Popover.Root>
        </div>
      )}
    </FieldFrame>
  );
}
