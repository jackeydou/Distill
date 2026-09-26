import { Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { cx } from "./ui";

type Theme = "system" | "light" | "dark";
const KEY = "distill-theme";

function stored(): Theme {
  const value = localStorage.getItem(KEY);
  return value === "light" || value === "dark" ? value : "system";
}

/** Sets `data-theme` on <html>, following the system setting unless the user picked one. */
export function applyTheme(theme: Theme = stored()): void {
  const dark =
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

const OPTIONS: { value: Theme; label: string; Icon: typeof Sun }[] = [
  { value: "light", label: "浅色", Icon: Sun },
  { value: "system", label: "跟随系统", Icon: Monitor },
  { value: "dark", label: "深色", Icon: Moon },
];

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(stored);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") {
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  return (
    <fieldset className="flex gap-0.5">
      <legend className="sr-only">主题</legend>
      {OPTIONS.map(({ value, label, Icon }) => (
        <button
          key={value}
          type="button"
          aria-pressed={theme === value}
          title={label}
          aria-label={label}
          onClick={() => {
            if (value === "system") {
              localStorage.removeItem(KEY);
            } else {
              localStorage.setItem(KEY, value);
            }
            setTheme(value);
          }}
          className={cx(
            "inline-flex size-8 items-center justify-center rounded-[1px] transition-colors duration-150",
            theme === value ? "bg-card text-ink" : "text-ink-faint hover:text-ink",
          )}
        >
          <Icon className="size-4" aria-hidden />
        </button>
      ))}
    </fieldset>
  );
}
