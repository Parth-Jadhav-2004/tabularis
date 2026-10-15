import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { SlidersHorizontal, X } from "lucide-react";
import { useTranslation } from "react-i18next";

export function ChatContextMenu({ children, count = 0 }: { children: ReactNode; count?: number }) {
  const { t } = useTranslation();
  const id = useId();
  const [open, setOpen] = useState(false);
  const container = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => { setOpen(false); trigger.current?.focus(); }, []);
  useEffect(() => {
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !container.current?.contains(event.target)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.isComposing) { event.preventDefault(); event.stopPropagation(); close(); }
    };
    document.addEventListener("pointerdown", dismiss);
    document.addEventListener("keydown", escape, true);
    return () => { document.removeEventListener("pointerdown", dismiss); document.removeEventListener("keydown", escape, true); };
  }, [open, close]);
  return <div ref={container} className="relative shrink-0">
    <button ref={trigger} type="button" aria-label={t("aiChat.context.options")} title={t("aiChat.context.options")}
      aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen((value) => !value)}
      className="inline-flex items-center gap-1 rounded-lg p-2 text-secondary hover:bg-surface-secondary hover:text-primary focus-ring">
      <SlidersHorizontal size={16} aria-hidden="true" />
      {count > 0 && <span className="text-xs">{count}</span>}
    </button>
    {open && <section id={id} aria-label={t("aiChat.context.options")}
      className="absolute bottom-full left-0 z-20 mb-3 max-h-80 w-72 max-w-[calc(100vw-3rem)] overflow-y-auto rounded-xl border border-strong bg-elevated shadow-xl">
      <div className="flex items-center justify-between px-3 pb-2 pt-3">
        <h3 className="text-sm font-medium text-primary">{t("aiChat.context.options")}</h3>
        <button type="button" aria-label={t("common.close")} onClick={close} className="rounded p-1 text-secondary hover:text-primary focus-ring"><X size={14} aria-hidden="true" /></button>
      </div>
      {children}
    </section>}
  </div>;
}
