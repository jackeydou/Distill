import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { cx } from "./ui";

export type Column = { key: string; label: string; value: number };

const PLOT_HEIGHT = 160;

/**
 * One series of columns (notes per week). Single hue, no legend: the section title names
 * it. Each column is its own hover and focus target; the peak carries a direct label and
 * every value is also in the table below the chart.
 */
export function Columns({ columns, unit }: { columns: Column[]; unit: string }) {
  const [active, setActive] = useState<number | null>(null);
  const max = Math.max(1, ...columns.map((c) => c.value));
  const peak = columns.reduce(
    (best, c, i) => (c.value > (columns[best]?.value ?? 0) ? i : best),
    0,
  );
  const tick = Math.max(1, Math.ceil(columns.length / 6));

  return (
    <figure>
      <div className="relative" style={{ height: PLOT_HEIGHT + 24 }}>
        <div className="absolute inset-x-0 top-0 border-t border-line" style={{ top: 20 }} />
        <div
          className="absolute inset-x-0 border-t border-line-strong"
          style={{ top: PLOT_HEIGHT + 20 }}
        />
        <div className="absolute inset-x-0 flex items-end" style={{ top: 20, height: PLOT_HEIGHT }}>
          {columns.map((c, i) => {
            const height = (c.value / max) * PLOT_HEIGHT;
            return (
              <button
                key={c.key}
                type="button"
                className="group relative flex h-full flex-1 items-end justify-center px-px outline-none"
                aria-label={`${c.label}：${c.value} ${unit}`}
                onPointerEnter={() => setActive(i)}
                onPointerLeave={() => setActive(null)}
                onFocus={() => setActive(i)}
                onBlur={() => setActive(null)}
              >
                <span
                  className={cx(
                    "block w-full max-w-6 rounded-t-[2px] transition-colors duration-150",
                    c.value > 0 ? "bg-brand" : "",
                    active === i && "bg-brand-strong",
                    "group-focus-visible:outline-2 group-focus-visible:outline-offset-2 group-focus-visible:outline-focus",
                  )}
                  style={{ height: c.value > 0 ? Math.max(height, 2) : 0 }}
                />
                {i === peak && c.value > 0 && active === null && (
                  <span
                    className="absolute text-xs text-ink tabular-nums"
                    style={{ bottom: height + 4 }}
                  >
                    {c.value}
                  </span>
                )}
                {active === i && (
                  <span
                    className="pointer-events-none absolute z-10 rounded-panel border border-line-strong bg-panel px-2 py-1 text-left whitespace-nowrap"
                    style={{ bottom: height + 6 }}
                  >
                    <span className="block text-sm font-bold text-ink tabular-nums">
                      {c.value} {unit}
                    </span>
                    <span className="block text-xs text-ink-faint">{c.label}</span>
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <div className="absolute inset-x-0 flex" style={{ top: PLOT_HEIGHT + 24 }}>
          {columns.map((c, i) => (
            <span key={c.key} className="flex-1 text-center text-[11px] text-ink-faint">
              {i % tick === 0 || i === columns.length - 1 ? c.label : ""}
            </span>
          ))}
        </div>
      </div>
      <DataTable
        head={["周", unit]}
        rows={columns.map((c) => [c.label, c.value])}
        className="mt-8"
      />
    </figure>
  );
}

export type Bar = { key: string; label: string; value: number };

/** Horizontal bars for a ranking (tags, projects), value at the tip. */
export function BarList({
  bars,
  link,
}: {
  bars: Bar[];
  link?: (bar: Bar) => { to: string; params?: Record<string, string> };
}) {
  const max = Math.max(1, ...bars.map((b) => b.value));
  return (
    <ul className="space-y-1">
      {bars.map((b) => {
        const target = link?.(b);
        const label = (
          <span className="block truncate text-sm text-ink" title={b.label}>
            {b.label}
          </span>
        );
        return (
          <li
            key={b.key}
            className="grid grid-cols-[minmax(0,10rem)_minmax(0,1fr)] items-center gap-3"
          >
            {target ? (
              <Link
                to={target.to}
                params={target.params}
                className="min-w-0 hover:[&>span]:text-brand-ink"
              >
                {label}
              </Link>
            ) : (
              label
            )}
            <span className="flex items-center gap-2">
              <span
                className="block h-3 rounded-r-[2px] bg-brand"
                style={{ width: `calc(${(b.value / max) * 100}% - 2.5rem)` }}
              />
              <span className="text-xs text-ink-muted tabular-nums">{b.value}</span>
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function DataTable({
  head,
  rows,
  className,
}: {
  head: string[];
  rows: (string | number)[][];
  className?: string;
}) {
  return (
    <details className={cx("text-sm", className)}>
      <summary className="cursor-pointer text-ink-faint hover:text-ink">数据表</summary>
      <table className="mt-2 w-full max-w-sm">
        <thead>
          <tr className="border-b border-line text-left text-xs text-ink-faint">
            {head.map((h) => (
              <th key={h} className="py-1 font-normal">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={String(row[0])} className="border-b border-line">
              {row.map((cell, i) => (
                <td key={head[i]} className={cx("py-1", i > 0 && "tabular-nums")}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}
