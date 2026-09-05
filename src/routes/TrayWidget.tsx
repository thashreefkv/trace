import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, CheckCircle2, Mic, Plus, Settings } from "lucide-react";
import {
  createCapture,
  createMeeting,
  getMenuBarState,
  showMainWindow,
  updateDeliverableState,
} from "../lib/ipc";
import type { MenuBarState } from "../lib/types";

// ── Design tokens (Sonoma-aligned) ─────────────────────────────────────────
const SYS_FONT =
  "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif";
const SYS_DISPLAY =
  "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Helvetica Neue', sans-serif";

const BLUE = "#0A84FF";
const RED = "#FF453A";
const GREEN = "#30D158";
const INK = "#1d1d1f";
const INK_2 = "rgba(0,0,0,0.62)";
const INK_3 = "rgba(0,0,0,0.42)";
const INK_4 = "rgba(0,0,0,0.28)";
const HAIRLINE = "rgba(0,0,0,0.07)";
const SURFACE_HOVER = "rgba(0,0,0,0.05)";

const Divider = () => (
  <div style={{ height: 1, background: HAIRLINE, transform: "scaleY(0.5)" }} />
);

export function TrayWidget() {
  const [state, setState] = useState<MenuBarState | null>(null);
  const [captureInput, setCaptureInput] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [justSaved, setJustSaved] = useState(false);
  const [startingRecording, setStartingRecording] = useState(false);
  const [finishingId, setFinishingId] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void refreshState();
    const interval = setInterval(refreshState, 10_000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const html = document.documentElement;
    const body = document.body;
    html.style.background = "transparent";
    body.style.background = "transparent";
    body.style.margin = "0";
    body.style.overflow = "hidden";
    body.style.height = "100vh";
  }, []);

  // Dismiss handled natively by the NSPanel's window_did_resign_key delegate.

  async function refreshState() {
    try {
      setState(await getMenuBarState());
    } catch {}
  }

  async function handleCapture(e: React.FormEvent) {
    e.preventDefault();
    if (!captureInput.trim() || isSubmitting) return;
    setIsSubmitting(true);
    try {
      await createCapture({ kind: "thought", body: captureInput.trim() });
      setCaptureInput("");
      setJustSaved(true);
      setTimeout(() => setJustSaved(false), 1600);
    } catch {}
    finally {
      setIsSubmitting(false);
    }
  }

  async function handleFinish() {
    if (!state?.active_deliverable_id) return;
    setFinishingId(state.active_deliverable_id);
    try {
      await updateDeliverableState(state.active_deliverable_id, "shipped");
      await refreshState();
    } catch {}
    finally {
      setFinishingId(null);
    }
  }

  async function handleStartRecording() {
    setStartingRecording(true);
    try {
      const today = new Date().toISOString().slice(0, 10);
      const meeting = await createMeeting({
        title: "Untitled meeting",
        date: today,
        stakeholder_ids: [],
      });
      await showMainWindow(`/meetings/${meeting.id}`);
    } catch {}
    finally {
      setStartingRecording(false);
    }
  }

  if (!state) return null;

  const { active_deliverable_id, active_deliverable_title, shipped_this_week } =
    state;

  return (
    <div
      // Card fills the entire window — native macOS shadow handles depth.
      style={{
        width: "100vw",
        height: "100vh",
        boxSizing: "border-box",
        fontFamily: SYS_FONT,
        WebkitFontSmoothing: "antialiased",
        // Strong frosted glass — high opacity so the card reads as solid
        // against any background (Chrome page, IDE, dark wallpaper).
        background: "rgba(246, 246, 248, 0.94)",
        backdropFilter: "blur(40px) saturate(180%)",
        WebkitBackdropFilter: "blur(40px) saturate(180%)",
        borderRadius: 12,
        // Crisp 1px outer line + bright inner top edge for that "lit pane" look.
        boxShadow: [
          "inset 0 0 0 0.5px rgba(0,0,0,0.16)",
          "inset 0 1px 0 rgba(255,255,255,0.85)",
        ].join(", "),
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        color: INK,
      }}
    >
      {/* ─────────────── Header ─────────────── */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 13px 10px",
        }}
      >
        <span
          style={{
            fontFamily: SYS_DISPLAY,
            fontSize: 14,
            fontWeight: 600,
            color: INK,
            letterSpacing: "-0.24px",
          }}
        >
          Trace
        </span>
        <IconButton
          title="Settings"
          onClick={() => showMainWindow("/settings")}
        >
          <Settings size={13} strokeWidth={1.75} />
        </IconButton>
      </div>

      <Divider />

      {/* ─────────────── Working on ─────────────── */}
      <div style={{ padding: "11px 13px 12px" }}>
        <div
          style={{
            fontSize: 10,
            fontWeight: 600,
            letterSpacing: "0.05em",
            textTransform: "uppercase",
            color: INK_3,
            marginBottom: 6,
          }}
        >
          Working on
        </div>

        {active_deliverable_id ? (
          <div style={{ display: "flex", alignItems: "flex-start", gap: 8 }}>
            <p
              style={{
                fontSize: 13,
                fontWeight: 500,
                color: INK,
                lineHeight: 1.32,
                flex: 1,
                minWidth: 0,
                margin: 0,
                overflow: "hidden",
                display: "-webkit-box",
                WebkitLineClamp: 2,
                WebkitBoxOrient: "vertical",
                letterSpacing: "-0.08px",
              }}
            >
              {active_deliverable_title}
            </p>
            <div
              style={{
                display: "flex",
                gap: 5,
                flexShrink: 0,
                marginTop: 1,
              }}
            >
              <button
                onClick={handleFinish}
                disabled={finishingId === active_deliverable_id}
                title="Mark done"
                style={{
                  fontSize: 11,
                  fontWeight: 590,
                  color:
                    finishingId === active_deliverable_id ? INK_4 : "#fff",
                  background:
                    finishingId === active_deliverable_id
                      ? "rgba(0,0,0,0.06)"
                      : BLUE,
                  border: "none",
                  borderRadius: 6,
                  padding: "0 9px",
                  cursor: "pointer",
                  height: 22,
                  fontFamily: SYS_FONT,
                  letterSpacing: "-0.08px",
                  boxShadow:
                    finishingId === active_deliverable_id
                      ? "none"
                      : "inset 0 0.5px 0 rgba(255,255,255,0.22), 0 1px 1.5px rgba(0,0,0,0.12)",
                  transition: "filter 0.12s",
                }}
                onMouseEnter={(e) => {
                  if (finishingId !== active_deliverable_id)
                    e.currentTarget.style.filter = "brightness(1.08)";
                }}
                onMouseLeave={(e) =>
                  (e.currentTarget.style.filter = "brightness(1)")
                }
              >
                Done
              </button>
              <IconButton
                title="Open"
                onClick={() =>
                  showMainWindow(`/deliverables/${active_deliverable_id}`)
                }
              >
                <ArrowUpRight size={11} strokeWidth={2.4} />
              </IconButton>
            </div>
          </div>
        ) : (
          <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
            <CheckCircle2
              size={14}
              strokeWidth={1.6}
              style={{ color: INK_4, flexShrink: 0 }}
            />
            <span style={{ fontSize: 13, color: INK_3 }}>
              No active deliverable
            </span>
          </div>
        )}
      </div>

      <Divider />

      {/* ─────────────── Record meeting ─────────────── */}
      <RowButton onClick={handleStartRecording} disabled={startingRecording}>
        <div
          style={{
            width: 28,
            height: 28,
            borderRadius: 7,
            background: "rgba(255,69,58,0.13)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Mic
            size={13}
            strokeWidth={2}
            style={{
              color: RED,
              animation: startingRecording ? "pulse 1s infinite" : "none",
            }}
          />
        </div>
        <div style={{ minWidth: 0 }}>
          <div
            style={{
              fontSize: 13,
              fontWeight: 500,
              color: INK,
              lineHeight: 1.3,
              letterSpacing: "-0.08px",
            }}
          >
            {startingRecording ? "Opening…" : "Record meeting"}
          </div>
          <div style={{ fontSize: 11, color: INK_3, marginTop: 1 }}>
            Capture &amp; extract action items
          </div>
        </div>
      </RowButton>

      <Divider />

      {/* ─────────────── Quick capture ─────────────── */}
      <form
        onSubmit={handleCapture}
        style={{ padding: "9px 13px", position: "relative" }}
      >
        <input
          ref={inputRef}
          type="text"
          placeholder="Capture a thought…"
          value={captureInput}
          onChange={(e) => setCaptureInput(e.target.value)}
          style={{
            width: "100%",
            height: 30,
            paddingLeft: 11,
            paddingRight: 36,
            borderRadius: 7,
            border: "0.5px solid rgba(0,0,0,0.16)",
            background: "rgba(255,255,255,0.65)",
            fontSize: 13,
            color: INK,
            fontFamily: SYS_FONT,
            outline: "none",
            boxSizing: "border-box",
            boxShadow: "inset 0 0.5px 1px rgba(0,0,0,0.05)",
            transition:
              "border-color 0.12s, background 0.12s, box-shadow 0.12s",
          }}
          onFocus={(e) => {
            e.target.style.border = `0.5px solid ${BLUE}`;
            e.target.style.background = "rgba(255,255,255,0.95)";
            e.target.style.boxShadow = `0 0 0 3px rgba(10,132,255,0.20)`;
          }}
          onBlur={(e) => {
            e.target.style.border = "0.5px solid rgba(0,0,0,0.16)";
            e.target.style.background = "rgba(255,255,255,0.65)";
            e.target.style.boxShadow = "inset 0 0.5px 1px rgba(0,0,0,0.05)";
          }}
        />
        <button
          type="submit"
          disabled={!captureInput.trim() || isSubmitting}
          style={{
            position: "absolute",
            right: 17,
            top: "50%",
            transform: "translateY(-50%)",
            background: justSaved
              ? "transparent"
              : captureInput.trim()
              ? BLUE
              : "transparent",
            border: "none",
            borderRadius: 5,
            width: 22,
            height: 22,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: captureInput.trim() ? "pointer" : "default",
            color: justSaved
              ? GREEN
              : captureInput.trim()
              ? "#fff"
              : INK_4,
            transition: "all 0.15s",
            boxShadow:
              captureInput.trim() && !justSaved
                ? "0 1px 1.5px rgba(0,0,0,0.12)"
                : "none",
          }}
        >
          {justSaved ? (
            <CheckCircle2 size={13} strokeWidth={2.4} />
          ) : (
            <Plus size={13} strokeWidth={2.4} />
          )}
        </button>
      </form>

      <Divider />

      {/* ─────────────── Footer ─────────────── */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "8px 13px 10px",
          marginTop: "auto",
        }}
      >
        <span style={{ fontSize: 11.5, color: INK_3 }}>
          {shipped_this_week > 0 ? (
            <>
              <span style={{ fontWeight: 600, color: INK_2 }}>
                {shipped_this_week}
              </span>{" "}
              done this week
            </>
          ) : (
            "Nothing done yet this week"
          )}
        </span>
        <button
          onClick={() => showMainWindow("/deliverables")}
          style={{
            background: "none",
            border: "none",
            fontSize: 12,
            fontWeight: 500,
            color: BLUE,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: 2,
            padding: "2px 4px",
            marginRight: -4,
            borderRadius: 4,
            fontFamily: SYS_FONT,
            transition: "background 0.12s",
          }}
          onMouseEnter={(e) =>
            (e.currentTarget.style.background = SURFACE_HOVER)
          }
          onMouseLeave={(e) =>
            (e.currentTarget.style.background = "transparent")
          }
        >
          Open
          <ArrowUpRight size={11} strokeWidth={2.4} />
        </button>
      </div>
    </div>
  );
}

// ── Reusable subcomponents ─────────────────────────────────────────────────

function IconButton({
  children,
  onClick,
  title,
}: {
  children: React.ReactNode;
  onClick: () => void;
  title: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      style={{
        background: "rgba(0,0,0,0.06)",
        border: "none",
        borderRadius: 5,
        width: 22,
        height: 22,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        cursor: "pointer",
        color: INK_2,
        transition: "background 0.12s, color 0.12s",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "rgba(0,0,0,0.11)";
        e.currentTarget.style.color = INK;
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "rgba(0,0,0,0.06)";
        e.currentTarget.style.color = INK_2;
      }}
    >
      {children}
    </button>
  );
}

function RowButton({
  children,
  onClick,
  disabled,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        width: "100%",
        padding: "10px 13px",
        background: "transparent",
        border: "none",
        cursor: disabled ? "default" : "pointer",
        textAlign: "left",
        fontFamily: SYS_FONT,
        opacity: disabled ? 0.55 : 1,
        transition: "background 0.1s",
      }}
      onMouseEnter={(e) => {
        if (!disabled) e.currentTarget.style.background = SURFACE_HOVER;
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
      }}
    >
      {children}
    </button>
  );
}
