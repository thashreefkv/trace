import { useEffect } from "react";

export interface BrainKeybindHandlers {
  cycleLeftRail: () => void;
  cycleRightRail: () => void;
  toggleFocus: () => void;
  exitFocusOrClear: () => void;
  openPalette: () => void;
  focusSearch: () => void;
  setMode?: (index: number) => void;
  recenter?: () => void;
  zoomIn?: () => void;
  zoomOut?: () => void;
}

// Brain page keyboard shortcuts. Bound at the document level but ignored when
// the user is typing in an input / textarea / contentEditable, so /  and 1..7
// don't fight with the search bar.
export function useBrainKeybinds(handlers: BrainKeybindHandlers): void {
  useEffect(() => {
    function isTypingTarget(el: EventTarget | null): boolean {
      if (!(el instanceof HTMLElement)) return false;
      const tag = el.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
      if (el.isContentEditable) return true;
      return false;
    }

    function onKeyDown(event: KeyboardEvent) {
      const meta = event.metaKey || event.ctrlKey;
      const target = event.target;
      const typing = isTypingTarget(target);

      // Esc always works (clear selection / exit focus). Most reliable in
      // typing contexts too, because it's a single key the browser doesn't
      // intercept.
      if (event.key === "Escape") {
        handlers.exitFocusOrClear();
        return;
      }

      // ⌘\ / Ctrl+\ — cycle left rail
      if (meta && event.key === "\\") {
        event.preventDefault();
        handlers.cycleLeftRail();
        return;
      }

      // ⌘. / Ctrl+. — cycle right inspector
      if (meta && event.key === ".") {
        event.preventDefault();
        handlers.cycleRightRail();
        return;
      }

      // ⌘⇧K / Ctrl+Shift+K — Brain command palette
      // (plain ⌘K is already bound to the global app palette upstream — we
      // disambiguate by requiring Shift so both can coexist.)
      if (meta && event.shiftKey && (event.key === "k" || event.key === "K")) {
        event.preventDefault();
        handlers.openPalette();
        return;
      }

      // Plain keys — skip while user is typing in a field.
      if (typing) return;

      if (event.key === "/") {
        event.preventDefault();
        handlers.focusSearch();
        return;
      }

      if (event.key === "f" || event.key === "F") {
        handlers.toggleFocus();
        return;
      }

      if (event.key === "c" || event.key === "C") {
        handlers.recenter?.();
        return;
      }

      if (event.key === "=" || event.key === "+") {
        handlers.zoomIn?.();
        return;
      }
      if (event.key === "-" || event.key === "_") {
        handlers.zoomOut?.();
        return;
      }

      // 1..7 mode switch
      if (handlers.setMode) {
        const code = event.key.charCodeAt(0);
        if (code >= 49 && code <= 55) {
          // '1' ... '7'
          handlers.setMode(code - 49);
        }
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handlers]);
}
