import type { ReactNode } from "react";

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
  return <span className={`rounded px-1.5 py-0.5 font-mono text-[10.5px] ${cls}`}>{children}</span>;
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
    "rounded-md px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-40 disabled:cursor-default";
  const look = {
    default: "border border-line-2 bg-surface hover:bg-surface-3",
    primary: "border border-accent bg-accent text-white hover:brightness-110",
    ghost: "text-ink-2 hover:bg-surface-3",
  }[variant];
  return (
    <button type="button" onClick={onClick} disabled={disabled} className={`${base} ${look} ${className}`}>
      {children}
      {hint && <span className="ml-1.5 font-mono text-[10px] opacity-55">{hint}</span>}
    </button>
  );
}
