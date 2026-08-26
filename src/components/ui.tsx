import type { ReactNode } from "react";
import type { Priority } from "../ipc/types";

export function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <span className="font-mono text-[10px] uppercase tracking-[0.17em] text-ink-3">{children}</span>
  );
}

export function Chip({ children, tone = "plain" }: { children: ReactNode; tone?: "plain" | "accent" | "warn" | "rest" | "alert" }) {
  const cls = {
    plain: "bg-surface-3 text-ink-2",
    accent: "bg-accent-soft text-accent-ink",
    warn: "bg-warn-soft text-warn",
    rest: "bg-rest-soft text-rest-ink",
    alert: "bg-alert-soft text-alert",
  }[tone];
  return (
    <span className={`rounded-[5px] px-[7px] py-[2px] font-mono text-[10.5px] tracking-[0.04em] ${cls}`}>
      {children}
    </span>
  );
}

export function Button({
  children, onClick, variant = "default", hint, disabled, className = "",
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "default" | "primary" | "ghost";
  hint?: string;
  disabled?: boolean;
  className?: string;
}) {
  const base =
    "rounded-[7px] px-[13px] py-1.5 text-[13px] font-medium transition-colors disabled:opacity-[.42] disabled:cursor-default";
  const look = {
    default: "border border-line-2 bg-surface hover:bg-surface-3",
    primary: "border border-accent bg-accent text-white hover:brightness-[1.08]",
    ghost: "border border-transparent text-ink-2 hover:bg-surface-3",
  }[variant];
  return (
    <button type="button" onClick={onClick} disabled={disabled} className={`${base} ${look} ${className}`}>
      {children}
      {hint && <span className="ml-[5px] font-mono text-[10px] opacity-55">{hint}</span>}
    </button>
  );
}

/**
 * The priority marker, shared by the queue and the popover so one task cannot
 * read as two priorities. Colour alone was not enough at 5px — `warn` and
 * `ink-3` are both muted greens in this palette — so Low is drawn hollow: the
 * three are separable by shape as well as by hue.
 */
export function PriorityDot({ priority }: { priority: Priority }) {
  const look = {
    High: "bg-alert",
    Medium: "bg-warn",
    Low: "bg-transparent border border-line-2",
  }[priority];
  return (
    <span
      title={`${priority} priority`}
      aria-label={`${priority} priority`}
      className={`h-[7px] w-[7px] shrink-0 rounded-full ${look}`}
    />
  );
}
