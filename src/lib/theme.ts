/**
 * Theme and accent colour.
 *
 * The theme follows the system unless the user picks one. The accent is a single
 * hue that retints the whole window, which is what the tokens are built around.
 */

export type ThemePreference = "system" | "dark" | "light";

/** The hue the palette in tokens.css is built around. */
export const DEFAULT_ACCENT = 265;

const THEME_KEY = "npdf.theme";
const ACCENT_KEY = "npdf.accent";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    // A locked down web view can refuse storage. That is not a reason to fail.
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Ignore, the setting simply does not survive a restart.
  }
}

export function loadTheme(): ThemePreference {
  const stored = read(THEME_KEY);
  return stored === "dark" || stored === "light" ? stored : "system";
}

export function applyTheme(preference: ThemePreference): void {
  const root = document.documentElement;
  if (preference === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", preference);
  }
  write(THEME_KEY, preference);
}

export function loadAccent(): number {
  const stored = Number(read(ACCENT_KEY));
  return Number.isFinite(stored) && stored >= 0 && stored < 360 ? stored : DEFAULT_ACCENT;
}

/**
 * Remember the accent the user picked.
 *
 * It does not retint the window yet, and that is on purpose rather than a
 * missing line. The palette in tokens.css carries literal colour values,
 * because WebKitGTK does not substitute a custom property into the hue slot of
 * hsl() and reads it as zero, which turns the whole window red. Retinting has
 * to compute the finished colours here and write them out as tokens. That work
 * belongs with the settings screen; until then the palette is fixed at the
 * default hue and this only records the preference.
 */
export function applyAccent(hue: number): void {
  const clamped = ((hue % 360) + 360) % 360;
  write(ACCENT_KEY, String(clamped));
}
