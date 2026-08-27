/**
 * The command palette.
 *
 * Everything the app can do is reachable from here, which is what keeps the rest
 * of the surface free of buttons.
 */

import { useEffect, useMemo, useRef, useState } from "react";

export interface Command {
  id: string;
  label: string;
  hint?: string;
  enabled: boolean;
  run: () => void;
}

interface Props {
  open: boolean;
  commands: Command[];
  onClose: () => void;
}

export function CommandPalette({ open, commands, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const available = commands.filter((command) => command.enabled);
    if (!needle) return available;
    return available.filter((command) => command.label.toLowerCase().includes(needle));
  }, [commands, query]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setCursor(0);
      // Focus after the entry animation has started, so the caret does not jump.
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    setCursor((current) => Math.min(current, Math.max(matches.length - 1, 0)));
  }, [matches.length]);

  if (!open) return null;

  return (
    <div className="palette__scrim" onMouseDown={onClose}>
      <div
        className="palette panel"
        role="dialog"
        aria-label="Kommandopalette"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="palette__input"
          value={query}
          placeholder="Was moechtest du tun?"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") onClose();
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setCursor((c) => Math.min(c + 1, matches.length - 1));
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              setCursor((c) => Math.max(c - 1, 0));
            }
            if (event.key === "Enter") {
              event.preventDefault();
              const chosen = matches[cursor];
              if (chosen) {
                onClose();
                chosen.run();
              }
            }
          }}
        />
        <ul className="palette__list">
          {matches.length === 0 && <li className="palette__empty">Nichts gefunden</li>}
          {matches.map((command, index) => (
            <li key={command.id}>
              <button
                className={`palette__item${index === cursor ? " palette__item--active" : ""}`}
                onMouseEnter={() => setCursor(index)}
                onClick={() => {
                  onClose();
                  command.run();
                }}
              >
                <span>{command.label}</span>
                {command.hint && <kbd className="palette__hint">{command.hint}</kbd>}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
