import { Link } from "@tanstack/react-router";
import { Check, Copy, LoaderCircle } from "lucide-react";
import { type ButtonHTMLAttributes, type ReactNode, useState } from "react";
import { ApiError } from "../api/bridge";

export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "quiet" | "danger";
};

export function Button({ variant = "quiet", className, ...props }: ButtonProps) {
  return (
    <button
      type="button"
      className={cx(
        "inline-flex min-h-8 items-center gap-1.5 rounded-[1px] border px-3 text-sm transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50",
        variant === "primary" &&
          "border-brand-strong bg-brand-strong text-on-brand hover:bg-brand-ink hover:border-brand-ink",
        variant === "quiet" &&
          "border-line-strong bg-panel text-ink hover:bg-panel-hover hover:border-ink-faint",
        variant === "danger" &&
          "border-line-strong bg-panel text-error hover:border-error hover:bg-error-soft",
        className,
      )}
      {...props}
    />
  );
}

export function PageHeader({
  title,
  eyebrow,
  children,
}: {
  title: ReactNode;
  eyebrow?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header className="mb-8">
      {eyebrow && <div className="mb-2 text-sm text-ink-faint">{eyebrow}</div>}
      <h1 className="text-[1.875rem] leading-tight sm:text-4xl">{title}</h1>
      {children && <div className="mt-3 text-ink-muted">{children}</div>}
    </header>
  );
}

export function Section({
  title,
  action,
  children,
}: {
  title: ReactNode;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="mt-10 first:mt-0">
      <div className="mb-4 flex items-baseline justify-between gap-4 border-b border-line pb-2">
        <h2 className="text-xl">{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
}

export function Panel({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div className={cx("rounded-panel border border-line bg-panel", className)}>{children}</div>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return (
    <p className="rounded-panel border border-dashed border-line px-4 py-6 text-center text-sm text-ink-faint">
      {children}
    </p>
  );
}

export function Loading() {
  return (
    <div className="flex items-center gap-2 py-10 text-sm text-ink-faint" role="status">
      <LoaderCircle className="size-4 animate-spin" aria-hidden />
      载入中…
    </div>
  );
}

export function ErrorBox({ error }: { error: unknown }) {
  const message = error instanceof Error ? error.message : String(error);
  const notFound = error instanceof ApiError && error.status === 404;
  return (
    <div
      role="alert"
      className="rounded-panel border border-error/40 bg-error-soft px-4 py-3 text-sm text-error"
    >
      {notFound ? "没有找到：" : "出错了："}
      {message}
    </div>
  );
}

/** How many times a topic was asked. Two or more is the signal Distill exists for. */
export function AskCount({ count, className }: { count: number; className?: string }) {
  const repeated = count >= 2;
  return (
    <span
      className={cx(
        "inline-flex shrink-0 items-center rounded-[1px] px-1.5 py-px text-xs tabular-nums",
        repeated ? "bg-warning-soft text-warning" : "text-ink-faint",
        className,
      )}
      title={`问了 ${count} 次`}
    >
      {repeated ? `问了 ${count} 次` : "1 次"}
    </span>
  );
}

export function TagChip({ tag }: { tag: string }) {
  return (
    <Link
      to="/tags/$tag"
      params={{ tag }}
      className="inline-flex items-center rounded-[1px] border border-line px-1.5 py-px text-xs text-ink-muted transition-colors duration-150 hover:border-brand hover:text-brand-ink"
    >
      #{tag}
    </Link>
  );
}

export function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="inline-flex size-8 shrink-0 items-center justify-center rounded-[1px] text-ink-faint transition-colors duration-150 hover:bg-panel-hover hover:text-ink"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        });
      }}
    >
      {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
    </button>
  );
}

export function Stat({ label, value, hint }: { label: string; value: ReactNode; hint?: string }) {
  return (
    <div className="rounded-panel border border-line bg-panel px-4 py-3">
      <div className="text-xs uppercase tracking-wider text-ink-faint">{label}</div>
      <div className="mt-1 text-3xl font-bold text-ink">{value}</div>
      {hint && <div className="mt-1 text-xs text-ink-faint">{hint}</div>}
    </div>
  );
}
