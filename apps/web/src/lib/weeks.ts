/** ISO week keys as the server writes them: `2026-W07`. */

function mondayOf(key: string): Date {
  const match = /^(\d{4})-W(\d{2})$/.exec(key);
  if (!match) {
    throw new Error(`not an ISO week: ${key}`);
  }
  const year = Number(match[1]);
  const week = Number(match[2]);
  // Week 1 is the week with January 4th in it.
  const jan4 = new Date(Date.UTC(year, 0, 4));
  const monday = new Date(jan4);
  monday.setUTCDate(jan4.getUTCDate() - ((jan4.getUTCDay() + 6) % 7) + (week - 1) * 7);
  return monday;
}

export function weekKey(date: Date): string {
  const d = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
  d.setUTCDate(d.getUTCDate() + 3 - ((d.getUTCDay() + 6) % 7));
  const year = d.getUTCFullYear();
  const week = 1 + Math.round((d.getTime() - mondayOf(`${year}-W01`).getTime()) / 604_800_000);
  return `${year}-W${String(Math.min(week, 53)).padStart(2, "0")}`;
}

/** The `count` weeks ending with `last`, oldest first, each with its Monday. */
export function weeksEnding(last: string, count: number): { key: string; monday: Date }[] {
  const end = mondayOf(last);
  return Array.from({ length: count }, (_, i) => {
    const monday = new Date(end);
    monday.setUTCDate(end.getUTCDate() - (count - 1 - i) * 7);
    return { key: weekKey(monday), monday };
  });
}
