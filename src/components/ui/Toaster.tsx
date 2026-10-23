/**
 * Toasts du shell, en bas à droite : messages courts émis par l'app ou par un plugin
 * (`cadence.notify`). Disparaissent seuls après 4 s ; une erreur reste jusqu'à fermeture.
 */
import { useEffect } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";
import { useUiStore, type Toast, type ToastKind } from "@/stores/ui";

const tones: Record<ToastKind, { icon: IconName; color: string }> = {
  info: { icon: "warning", color: "text-blue" },
  success: { icon: "check", color: "text-green" },
  warning: { icon: "warning", color: "text-amber" },
  error: { icon: "warning", color: "text-red" },
};

export function Toaster() {
  const toasts = useUiStore((s) => s.toasts);
  return (
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-[340px] flex-col gap-2"
    >
      {toasts.map((toast) => (
        <ToastCard key={toast.id} toast={toast} />
      ))}
    </div>
  );
}

export function ToastCard({ toast }: { toast: Toast }) {
  const dismiss = useUiStore((s) => s.dismissToast);
  useEffect(() => {
    if (toast.kind === "error") return;
    const timer = setTimeout(() => dismiss(toast.id), 4000);
    return () => clearTimeout(timer);
  }, [toast, dismiss]);
  const tone = tones[toast.kind];
  return (
    <div className="pointer-events-auto flex items-start gap-2.5 rounded-card border border-line2 bg-panel px-3.5 py-3 shadow-modal">
      <span className={cn("mt-px flex", tone.color)}>
        <Icon name={tone.icon} size={15} />
      </span>
      <div className="min-w-0 grow">
        {toast.source && <div className="text-micro text-tx3">{toast.source}</div>}
        <p className="text-body-sm leading-[1.5] text-tx">{toast.message}</p>
      </div>
      <button
        type="button"
        aria-label="Fermer"
        onClick={() => dismiss(toast.id)}
        className="flex text-tx3 hover:text-tx"
      >
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}
