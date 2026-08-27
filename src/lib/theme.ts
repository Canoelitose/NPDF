/**
 * Theme and accent colour.
 *
 * The theme follows the system unless the user picks one. The accent is a single
 * hue that retints the whole window, which is what the tokens are built around.
 */

export type ThemePreference = "system" | "dark" | "light";

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
  return Number.isFinite(stored) && stored >= 0 && stored < 360 ? stored : 265;
}

export function applyAccent(hue: number): void {
  const clamped = ((hue % 360) + 360) % 360;
  document.documentElement.style.setProperty("--accent-h", String(clamped));
  write(ACCENT_KEY, String(clamped));
}
