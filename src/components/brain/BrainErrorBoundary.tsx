import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";

interface BrainErrorBoundaryProps {
  children: ReactNode;
  onReset?: () => void;
}

interface BrainErrorBoundaryState {
  error: Error | null;
  info: ErrorInfo | null;
}

/**
 * React error boundary specific to the Brain explorer. Without one of these
 * any thrown error during render (e.g. a bad graphology insert or a Sigma
 * init failure) unmounts the whole router subtree and the user just sees a
 * blank page. Catching here keeps the rest of Settings reachable and gives
 * a copy-pastable error string for debugging.
 */
export class BrainErrorBoundary extends Component<
  BrainErrorBoundaryProps,
  BrainErrorBoundaryState
> {
  state: BrainErrorBoundaryState = { error: null, info: null };

  static getDerivedStateFromError(error: Error): Partial<BrainErrorBoundaryState> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.setState({ info });
    if (typeof console !== "undefined") {
      console.error("[brain] explorer crashed", error, info);
    }
  }

  reset = () => {
    this.setState({ error: null, info: null });
    this.props.onReset?.();
  };

  render() {
    if (!this.state.error) return this.props.children;
    const stackHead = this.state.error.stack?.split("\n").slice(0, 4).join("\n");
    return (
      <div className="flex h-full min-h-[420px] items-center justify-center p-8">
        <div className="w-full max-w-xl rounded-2xl border border-zinc-100 bg-white p-6 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
          <div className="flex items-start gap-3">
            <div className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-rose-50 text-rose-500">
              <AlertTriangle aria-hidden size={18} />
            </div>
            <div className="min-w-0 flex-1">
              <p className="page-kicker">Brain explorer</p>
              <h2 className="mt-1 text-[15px] font-semibold tracking-tight text-zinc-950">
                Something broke while rendering the graph
              </h2>
              <p className="mt-1 text-[12.5px] text-zinc-500">
                The rest of the app is fine. Try retrying — if it keeps happening, copy
                the trace below and share it.
              </p>

              <pre className="mt-3 max-h-44 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-zinc-100 bg-zinc-50 p-3 font-mono text-[11px] leading-relaxed text-zinc-700">
                {this.state.error.message}
                {stackHead ? `\n\n${stackHead}` : ""}
              </pre>

              <div className="mt-4 flex items-center gap-2">
                <button
                  className="inline-flex items-center gap-1.5 rounded-xl bg-zinc-900 px-3 py-1.5 text-[12px] font-medium text-white transition-colors hover:bg-zinc-800"
                  onClick={this.reset}
                  type="button"
                >
                  <RefreshCw size={12} />
                  Retry
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }
}
