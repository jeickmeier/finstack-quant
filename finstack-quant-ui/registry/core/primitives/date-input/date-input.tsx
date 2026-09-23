"use client";
import { useState } from "react";
import {
  InputGroup,
  InputGroupInput,
  InputGroupAddon,
  InputGroupButton,
} from "@/components/ui/input-group";
import { Calendar } from "@/components/ui/calendar";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverTitle,
} from "@/components/ui/popover";
import { CalendarIcon } from "lucide-react";
import {
  FieldFrame,
  type FieldInfo,
} from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { useSurfaceAttributes } from "@/components/finstack/shared/primitives/surface/surface";
/** Supplied ISO text and a stock calendar; native validation owns financial date rules. */
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
  const surface = useSurfaceAttributes();
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
        <Popover open={open} onOpenChange={setOpen}>
          <InputGroup>
            <InputGroupInput
              {...control}
              value={value}
              onChange={(event) => onValueChange(event.target.value)}
              placeholder="YYYY-MM-DD"
              min={min}
              max={max}
              disabled={disabled}
            />
            <InputGroupAddon align="inline-end">
              <PopoverTrigger
                disabled={disabled}
                aria-label={`${field.label} calendar`}
                render={<InputGroupButton size="icon-xs" />}
              >
                <CalendarIcon />
              </PopoverTrigger>
            </InputGroupAddon>
          </InputGroup>
          <PopoverContent {...surface} align="end" className="w-auto p-0">
            <PopoverTitle className="sr-only">{field.label}</PopoverTitle>
            <Calendar
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
                  businessDays?.map((text) => parse(text)!).filter(Boolean) ??
                  [],
              }}
              modifiersClassNames={{ businessDay: "underline" }}
            />
          </PopoverContent>
        </Popover>
      )}
    </FieldFrame>
  );
}
