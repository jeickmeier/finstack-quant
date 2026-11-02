"use client";
import { useState, type ReactNode } from "react";
/** A disclosure whose content, and so its native calls, mounts when first opened. */
export function LazySection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>{title}</summary>
      {open && children}
    </details>
  );
}
