export const SOURCE_COLOUR: Record<string, string> = {
  github: "var(--info)",
  wago: "var(--good)",
  wowinterface: "var(--attention)",
  curseforge: "var(--act)",
};

export const sourceColour = (s: string | null | undefined) => (s && SOURCE_COLOUR[s]) || "var(--text-3)";

const TINTS = [
  ["var(--accent-tint)", "var(--accent-text)"],
  ["var(--info-tint)", "var(--info)"],
  ["var(--attn-tint)", "var(--attention)"],
  ["var(--good-tint)", "var(--good)"],
];

/** A stable tile tint per addon name. */
export function tint(name: string): [string, string] {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return TINTS[h % TINTS.length] as [string, string];
}

export const monogram = (name: string) => (name.match(/[A-Za-z0-9]/)?.[0] ?? "?").toUpperCase();

export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return "";
  const t = new Date(iso).getTime();
  const s = Math.max(0, Math.round((now - t) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  if (d === 1) return "yesterday";
  if (d < 14) return `${d} days ago`;
  if (d < 60) return `${Math.round(d / 7)} weeks ago`;
  return shortDate(iso);
}

export function shortDate(iso: string | null | undefined): string {
  if (!iso) return "";
  return new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** "Yesterday 21:14", "Today 09:02", "3 Nov 18:20". */
export function when(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const today = new Date();
  const y = new Date(); y.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return `Today ${time}`;
  if (d.toDateString() === y.toDateString()) return `Yesterday ${time}`;
  return `${shortDate(iso)} ${time}`;
}

export function size(bytes: number | null | undefined): string {
  if (!bytes) return "";
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function count(n: number | null | undefined): string {
  if (n == null) return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
  if (n >= 1000) return `${Math.round(n / 1000)}K`;
  return String(n);
}

export const plural = (n: number, one: string, many = one + "s") => `${n} ${n === 1 ? one : many}`;

/** Flavour tile colour and letter (sidebar, Settings). */
export function flavourTile(flavour: string): { bg: string; letter: string } {
  switch (flavour) {
    case "forever": return { bg: "var(--nx-coral)", letter: "F" };
    case "classic-era": return { bg: "var(--nx-plum)", letter: "E" };
    case "anniversary": return { bg: "#3f4a7a", letter: "A" };
    default: return { bg: "#3a3860", letter: "R" };
  }
}
