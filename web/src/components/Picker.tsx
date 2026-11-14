import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

export interface PickerItem {
  id: string;
  name: string;
  subtitle?: string;
  meta?: ReactNode;
  search?: string;
}

interface PickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  items: PickerItem[];
  selected?: string;
  placeholder: string;
  emptyLabel: string;
  onSelect: (id: string) => void;
  footer?: ReactNode;
}

export function Picker({
  open,
  onOpenChange,
  title,
  description,
  items,
  selected,
  placeholder,
  emptyLabel,
  onSelect,
  footer,
}: PickerProps) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      const selectedIndex = Math.max(
        0,
        items.findIndex((item) => item.id === selected),
      );
      setIndex(selectedIndex);
    }
  }, [open, items, selected]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) =>
      `${item.name} ${item.id} ${item.subtitle ?? ""} ${item.search ?? ""}`
        .toLowerCase()
        .includes(needle),
    );
  }, [items, query]);

  useEffect(() => {
    setIndex((current) => Math.min(current, Math.max(filtered.length - 1, 0)));
  }, [filtered.length]);

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-index="${index}"]`);
    node?.scrollIntoView({ block: "nearest" });
  }, [index]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(560px,calc(100vw-2rem))] max-w-[560px] gap-0 overflow-hidden p-0 sm:max-w-[560px]">
        <DialogHeader className="space-y-1 border-b border-line px-5 py-4">
          <DialogTitle className="text-[15px]">{title}</DialogTitle>
          {description && (
            <DialogDescription className="text-[12px] text-dim">{description}</DialogDescription>
          )}
        </DialogHeader>
        <div className="relative border-b border-line px-4 py-3">
          <MagnifyingGlass
            size={15}
            weight="bold"
            className="pointer-events-none absolute left-7 top-1/2 -translate-y-1/2 text-faint"
          />
          <Input
            autoFocus
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setIndex((i) => Math.min(i + 1, filtered.length - 1));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setIndex((i) => Math.max(i - 1, 0));
              } else if (event.key === "Enter") {
                event.preventDefault();
                const item = filtered[index];
                if (item) {
                  onSelect(item.id);
                  onOpenChange(false);
                }
              }
            }}
            placeholder={placeholder}
            className="h-9 border-0 bg-white/[0.04] pl-9 focus-visible:ring-2 focus-visible:ring-brand/60"
            aria-label={placeholder}
          />
        </div>
        <div ref={listRef} className="max-h-[52vh] overflow-y-auto p-2">
          {filtered.length === 0 && (
            <p className="px-3 py-8 text-center text-[12.5px] text-faint">{emptyLabel}</p>
          )}
          {filtered.map((item, i) => (
            <button
              key={item.id}
              type="button"
              data-index={i}
              onClick={() => {
                onSelect(item.id);
                onOpenChange(false);
              }}
              onMouseEnter={() => setIndex(i)}
              className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left transition-colors ${
                i === index ? "bg-white/[0.06]" : "hover:bg-white/[0.04]"
              } ${item.id === selected ? "text-ink" : "text-ink/90"}`}
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] font-medium">{item.name}</span>
                {item.subtitle && (
                  <span className="mono block truncate text-[10.5px] text-faint">{item.subtitle}</span>
                )}
              </span>
              {item.meta}
            </button>
          ))}
        </div>
        {footer}
      </DialogContent>
    </Dialog>
  );
}
