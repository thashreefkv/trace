import { useEffect, useState } from "react";
import { Check, Clipboard, Eye, KeyRound, RefreshCw, Trash2 } from "lucide-react";
import {
  clearGeminiApiKey,
  getGeminiKeyStatus,
  getMcpConfigSnippet,
  getMcpInstallState,
  installMcpServer,
  openMcpLog,
  saveGeminiApiKey,
  testGeminiApiKey,
} from "../lib/ipc";
import type { GeminiKeyStatus, McpConfigSnippet, McpInstallState } from "../lib/types";

export function McpSettings() {
  const [installState, setInstallState] = useState<McpInstallState | null>(null);
  const [snippet, setSnippet] = useState<McpConfigSnippet | null>(null);
  const [geminiStatus, setGeminiStatus] = useState<GeminiKeyStatus | null>(null);
  const [geminiKey, setGeminiKey] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    void loadSettings();
  }, []);

  async function loadSettings() {
    try {
      setError(null);
      const [nextInstallState, nextSnippet, nextGeminiStatus] = await Promise.all([
        getMcpInstallState(),
        getMcpConfigSnippet(),
        getGeminiKeyStatus(),
      ]);
      setInstallState(nextInstallState);
      setSnippet(nextSnippet);
      setGeminiStatus(nextGeminiStatus);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleInstall() {
    try {
      setError(null);
      setMessage(null);
      setIsBusy(true);
      const nextInstallState = await installMcpServer();
      setInstallState(nextInstallState);
      setSnippet(await getMcpConfigSnippet());
      if (nextInstallState.last_error) {
        setError(nextInstallState.last_error);
      } else {
        setMessage("MCP server install refreshed.");
      }
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleCopySnippet() {
    if (!snippet) {
      return;
    }

    try {
      setError(null);
      await navigator.clipboard.writeText(snippet.snippet);
      setMessage("Claude Desktop snippet copied.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleSaveGeminiKey() {
    try {
      setError(null);
      setMessage(null);
      setIsBusy(true);
      setGeminiStatus(await saveGeminiApiKey(geminiKey));
      setGeminiKey("");
      setMessage("Gemini API key saved to Keychain.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleClearGeminiKey() {
    try {
      setError(null);
      setMessage(null);
      setGeminiStatus(await clearGeminiApiKey());
      setMessage("Gemini API key cleared.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleTestGeminiKey() {
    try {
      setError(null);
      setMessage(null);
      setIsBusy(true);
      const result = await testGeminiApiKey();
      if (result.ok) {
        setMessage(result.message);
      } else {
        setError(result.message);
      }
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleOpenLog() {
    try {
      setError(null);
      await openMcpLog();
    } catch (caught) {
      setError(String(caught));
    }
  }

  return (
    <div className="mx-auto min-h-full max-w-5xl px-5 py-6">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
            Claude Desktop
          </p>
          <h1 className="text-2xl font-semibold tracking-normal text-neutral-950 dark:text-neutral-50">
            MCP Settings
          </h1>
        </div>
        <button className="btn" onClick={() => void loadSettings()} type="button">
          <RefreshCw aria-hidden="true" size={16} />
          Refresh
        </button>
      </div>

      {error ? (
        <div className="mb-4 notice notice-error">
          {error}
        </div>
      ) : null}

      {message ? (
        <div className="mb-4 rounded-md border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-900 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-100">
          {message}
        </div>
      ) : null}

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_360px]">
        <section className="space-y-5">
          <div className="rounded-md border border-neutral-200 bg-white p-5 dark:border-neutral-800 dark:bg-neutral-900">
            <div className="mb-4 flex items-start justify-between gap-4">
              <div>
                <h2 className="text-base font-semibold text-neutral-950 dark:text-neutral-50">
                  Server install
                </h2>
                <p className="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
                  {installState?.installed ? "Installed" : "Not installed"}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <button className="btn" disabled={isBusy} onClick={() => void handleInstall()} type="button">
                  <RefreshCw aria-hidden="true" size={16} />
                  Reinstall
                </button>
                <button className="btn" onClick={() => void handleOpenLog()} type="button">
                  <Eye aria-hidden="true" size={16} />
                  View MCP Log
                </button>
              </div>
            </div>

            {installState ? (
              <dl className="grid gap-3 text-sm">
                <PathRow label="MCP binary" value={installState.binary_path} />
                <PathRow label="Database" value={installState.database_path} />
                <PathRow label="Log" value={installState.log_path} />
                <PathRow label="Snippet file" value={installState.snippet_path} />
                {installState.last_error ? (
                  <div className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-amber-900 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-100">
                    {installState.last_error}
                  </div>
                ) : null}
              </dl>
            ) : (
              <p className="text-sm text-neutral-500 dark:text-neutral-400">Loading install state...</p>
            )}
          </div>

          <div className="rounded-md border border-neutral-200 bg-white p-5 dark:border-neutral-800 dark:bg-neutral-900">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <div>
                <h2 className="text-base font-semibold text-neutral-950 dark:text-neutral-50">
                  Claude Desktop snippet
                </h2>
                <p className="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
                  Paste this into {snippet?.config_path ?? "Claude Desktop config"}.
                </p>
              </div>
              <button className="btn" disabled={!snippet} onClick={() => void handleCopySnippet()} type="button">
                <Clipboard aria-hidden="true" size={16} />
                Copy
              </button>
            </div>
            <pre className="max-h-80 overflow-auto rounded-md bg-neutral-950 p-4 text-xs leading-5 text-neutral-100">
              {snippet?.snippet ?? "Loading..."}
            </pre>
          </div>
        </section>

        <aside className="h-fit rounded-md border border-neutral-200 bg-white p-5 dark:border-neutral-800 dark:bg-neutral-900">
          <div className="mb-4 flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-md bg-neutral-100 text-neutral-700 dark:bg-neutral-800 dark:text-neutral-200">
              <KeyRound aria-hidden="true" size={16} />
            </div>
            <div>
              <h2 className="text-sm font-semibold">Gemini</h2>
              <p className="text-xs text-neutral-500 dark:text-neutral-400">
                {geminiStatus?.configured ? "Key configured" : "Fallback active"}
              </p>
            </div>
          </div>

          <div className="space-y-3">
            <label className="block space-y-1.5">
              <span className="field-label">API key</span>
              <input
                className="field-control"
                onChange={(event) => setGeminiKey(event.currentTarget.value)}
                placeholder="AIza..."
                type="password"
                value={geminiKey}
              />
            </label>
            <button
              className="btn btn-primary w-full"
              disabled={isBusy || !geminiKey.trim()}
              onClick={() => void handleSaveGeminiKey()}
              type="button"
            >
              <Check aria-hidden="true" size={16} />
              Save Key
            </button>
            <button
              className="btn w-full"
              disabled={isBusy || !geminiStatus?.configured}
              onClick={() => void handleTestGeminiKey()}
              type="button"
            >
              <RefreshCw aria-hidden="true" size={16} />
              Test Key
            </button>
            <button
              className="btn btn-danger w-full"
              disabled={isBusy || !geminiStatus?.configured}
              onClick={() => void handleClearGeminiKey()}
              type="button"
            >
              <Trash2 aria-hidden="true" size={16} />
              Clear Key
            </button>
          </div>
        </aside>
      </div>
    </div>
  );
}

interface PathRowProps {
  label: string;
  value: string;
}

function PathRow({ label, value }: PathRowProps) {
  return (
    <div>
      <dt className="field-label">{label}</dt>
      <dd className="mt-1 break-all font-mono text-xs text-neutral-700 dark:text-neutral-300">
        {value}
      </dd>
    </div>
  );
}
