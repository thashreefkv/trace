import { FormEvent, useEffect, useRef, useState } from "react";
import { Check, X } from "lucide-react";
import { createCapture, hideCaptureWindow } from "../lib/ipc";
import type { CaptureKind } from "../lib/types";
import { captureKindLabels, captureKindOptions } from "../lib/types";

export function CaptureWindow() {
  const [kind, setKind] = useState<CaptureKind>("thought");
  const [body, setBody] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const textAreaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    textAreaRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        void hideCaptureWindow();
      }

      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        void saveCapture();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [body, isSaving, kind]);

  async function saveCapture() {
    const value = body.trim();
    if (!value || isSaving) {
      return;
    }

    try {
      setError(null);
      setIsSaving(true);
      await createCapture({ kind, body: value });
      setBody("");
      await hideCaptureWindow();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await saveCapture();
  }

  return (
    <main className="flex min-h-screen bg-white p-4 text-neutral-950 dark:bg-neutral-950 dark:text-neutral-50">
      <form className="flex min-h-0 flex-1 flex-col gap-3" onSubmit={handleSubmit}>
        <div className="flex items-center justify-between gap-3">
          <label className="min-w-0 flex-1">
            <span className="field-label">Kind</span>
            <select
              className="field-control mt-1 h-9 py-1"
              onChange={(event) => setKind(event.currentTarget.value as CaptureKind)}
              value={kind}
            >
              {captureKindOptions.map((option) => (
                <option key={option} value={option}>
                  {captureKindLabels[option]}
                </option>
              ))}
            </select>
          </label>
          <button
            aria-label="Dismiss"
            className="btn mt-5 h-9 w-9 px-0"
            onClick={() => void hideCaptureWindow()}
            type="button"
          >
            <X aria-hidden="true" size={16} />
          </button>
        </div>

        <label className="flex min-h-0 flex-1 flex-col">
          <span className="field-label">Capture</span>
          <textarea
            className="field-control mt-1 min-h-0 flex-1 resize-none"
            onChange={(event) => setBody(event.currentTarget.value)}
            ref={textAreaRef}
            required
            value={body}
          />
        </label>

        {error ? (
          <div className="mb-4 notice notice-error">
            {error}
          </div>
        ) : null}

        <div className="flex justify-end">
          <button className="btn btn-primary" disabled={isSaving || !body.trim()} type="submit">
            <Check aria-hidden="true" size={16} />
            Save
          </button>
        </div>
      </form>
    </main>
  );
}
