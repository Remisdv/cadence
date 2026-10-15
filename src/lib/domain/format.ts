/** Formats d'affichage, en français, tels qu'écrits dans les maquettes. */

const dayMonth = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });
const time = new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit" });
const relative = new Intl.RelativeTimeFormat("fr", { numeric: "auto", style: "short" });
const decimal = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });

/** « 9 sept. » */
export function formatDay(iso: string): string {
  return dayMonth.format(new Date(iso));
}

/** « 9 sept. → 23 sept. » */
export function formatRange(start: string | null, end: string | null): string {
  if (!start || !end) return start ? formatDay(start) : end ? formatDay(end) : "";
  return `${formatDay(start)} → ${formatDay(end)}`;
}

/** « il y a 4 min », « il y a 3 h », « hier, 18:24 », « 12 sept. » */
export function formatAgo(iso: string, now: Date = new Date()): string {
  const date = new Date(iso);
  const seconds = Math.round((date.getTime() - now.getTime()) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 60) return "à l'instant";
  if (abs < 3600) return relative.format(Math.round(seconds / 60), "minute");
  if (date.toDateString() === now.toDateString()) {
    return relative.format(Math.round(seconds / 3600), "hour");
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return `hier, ${time.format(date)}`;
  return formatDay(iso);
}

/** Durée d'exécution « 04:12 » (mm:ss), « 1:04:12 » au-delà d'une heure. */
export function formatDuration(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const hh = Math.floor(s / 3600);
  const mm = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return hh > 0 ? `${hh}:${pad(mm)}:${pad(ss)}` : `${pad(mm)}:${pad(ss)}`;
}

/** « 2,4 » */
export function formatDecimal(n: number): string {
  return decimal.format(n);
}

/** « 61 % » (espace insécable fine, comme en typographie française). */
export function formatPercent(n: number): string {
  return `${Math.round(n)} %`;
}

/** « 6 j restants », « dernier jour », « terminé » */
export function formatDaysLeft(days: number | null): string {
  if (days === null) return "";
  if (days < 0) return "terminé";
  if (days === 0) return "dernier jour";
  return `${days} j restant${days > 1 ? "s" : ""}`;
}
