import { useEffect, useRef } from "react";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import { bracketMatching, StreamLanguage } from "@codemirror/language";
import { baseKindLabels } from "../../lib/brain/kinds";

interface BrainCypherEditorProps {
  value: string;
  onChange: (next: string) => void;
  onRun: () => void;
  /** Optional list of node labels from the Kuzu schema; falls back to entity kinds. */
  schemaLabels?: string[];
  schemaRelations?: string[];
}

const CYPHER_KEYWORDS = new Set([
  "MATCH",
  "OPTIONAL",
  "WHERE",
  "RETURN",
  "WITH",
  "UNWIND",
  "ORDER",
  "BY",
  "LIMIT",
  "SKIP",
  "AND",
  "OR",
  "NOT",
  "IN",
  "AS",
  "DISTINCT",
  "TRUE",
  "FALSE",
  "NULL",
  "CALL",
  "YIELD",
  "EXISTS",
  "STARTS",
  "ENDS",
  "CONTAINS",
]);

const CYPHER_FUNCTIONS = new Set([
  "count",
  "sum",
  "avg",
  "min",
  "max",
  "collect",
  "size",
  "labels",
  "type",
  "id",
  "toLower",
  "toUpper",
  "trim",
]);

const cypherLanguage = StreamLanguage.define({
  startState: () => ({ inString: false }),
  token(stream, state) {
    if (state.inString) {
      if (stream.skipTo('"')) {
        stream.next();
        state.inString = false;
      } else {
        stream.skipToEnd();
      }
      return "string";
    }
    if (stream.eatSpace()) return null;
    const ch = stream.peek();
    if (ch === '"') {
      stream.next();
      state.inString = true;
      return "string";
    }
    if (ch === "/" && stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/[0-9]+(\.[0-9]+)?/)) return "number";
    if (stream.match(/[(){}[\]]/)) return "bracket";
    if (stream.match(/[-=<>!+*/%]/)) return "operator";
    if (stream.match(/:/)) return "punctuation";
    const wordMatch = stream.match(/[A-Za-z_][A-Za-z0-9_]*/);
    if (wordMatch && typeof wordMatch !== "boolean") {
      const word = wordMatch[0];
      const upper = word.toUpperCase();
      if (CYPHER_KEYWORDS.has(upper)) return "keyword";
      if (CYPHER_FUNCTIONS.has(word)) return "function";
      // PascalCase / snake_case labels following ':' look like types.
      return "variable";
    }
    stream.next();
    return null;
  },
});

const editorTheme = EditorView.theme(
  {
    "&": {
      fontFamily:
        "ui-monospace, 'JetBrains Mono', 'Fira Code', SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: "12px",
      lineHeight: "1.55",
      color: "#1f2937",
    },
    ".cm-content": { caretColor: "#0ea5e9", padding: "6px 0" },
    ".cm-line": { padding: "0" },
    ".cm-scroller": { overflowX: "auto" },
    "&.cm-focused": { outline: "none" },
    ".cm-cursor": { borderLeftColor: "#0ea5e9" },
    ".cm-tooltip": {
      borderRadius: "10px",
      border: "1px solid rgb(244, 244, 245)",
      background: "white",
      boxShadow: "0 10px 30px rgba(0,0,0,0.10)",
      padding: "4px",
    },
    ".cm-tooltip-autocomplete > ul > li": {
      borderRadius: "6px",
      padding: "4px 8px",
      fontSize: "11.5px",
    },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
      background: "#0ea5e9",
      color: "white",
    },
  },
  { dark: false },
);

const tokenHighlight = EditorView.theme({
  ".ͼ1": { color: "#7c3aed" },
});
void tokenHighlight;

export function BrainCypherEditor({
  value,
  onChange,
  onRun,
  schemaLabels,
  schemaRelations,
}: BrainCypherEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onRunRef = useRef(onRun);
  const valueRef = useRef(value);
  const schemaRef = useRef({ labels: schemaLabels, relations: schemaRelations });

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  useEffect(() => {
    onRunRef.current = onRun;
  }, [onRun]);
  useEffect(() => {
    schemaRef.current = { labels: schemaLabels, relations: schemaRelations };
  }, [schemaLabels, schemaRelations]);

  useEffect(() => {
    if (!hostRef.current) return;

    const labels = () =>
      schemaRef.current.labels && schemaRef.current.labels.length > 0
        ? schemaRef.current.labels
        : Object.keys(baseKindLabels);

    const relations = () => schemaRef.current.relations ?? [];

    const completion = (ctx: CompletionContext) => {
      const word = ctx.matchBefore(/[A-Za-z_:]+/);
      if (!word) return null;
      const text = ctx.state.doc.toString();
      const before = text.slice(0, word.from);
      const tail = before.slice(-32);

      // After "(n:" suggest node labels.
      if (/[(,]\s*[A-Za-z_]*\s*:$/.test(tail) || /[(,]\s*[A-Za-z_]+\s*:$/.test(tail)) {
        return null;
      }
      if (/:[A-Za-z_]*$/.test(word.text)) {
        const prefix = word.text.replace(/^:/, "");
        const options = labels()
          .filter((label) => label.startsWith(prefix))
          .map((label) => ({ label: `:${label}`, type: "type" }));
        return { from: word.from, options, validFor: /^:[A-Za-z_]*$/ };
      }
      // After "[r:" suggest relation types.
      if (/\[\s*[A-Za-z_]*\s*$/.test(tail)) {
        return {
          from: word.from,
          options: relations().map((r) => ({ label: r, type: "interface" })),
          validFor: /^[A-Za-z_]*$/,
        };
      }
      // Generic keyword completion.
      const opts: Array<{ label: string; type: string }> = [];
      for (const kw of CYPHER_KEYWORDS) {
        if (kw.startsWith(word.text.toUpperCase())) {
          opts.push({ label: kw, type: "keyword" });
        }
      }
      for (const fn of CYPHER_FUNCTIONS) {
        if (fn.startsWith(word.text.toLowerCase())) {
          opts.push({ label: fn, type: "function" });
        }
      }
      if (opts.length === 0) return null;
      return { from: word.from, options: opts };
    };

    const extensions: Extension[] = [
      history(),
      bracketMatching(),
      autocompletion({ override: [completion], activateOnTyping: true }),
      cypherLanguage,
      editorTheme,
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        {
          key: "Mod-Enter",
          run: () => {
            onRunRef.current();
            return true;
          },
        },
      ]),
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          const next = update.state.doc.toString();
          valueRef.current = next;
          onChangeRef.current(next);
        }
      }),
    ];

    const view = new EditorView({
      state: EditorState.create({ doc: value, extensions }),
      parent: hostRef.current,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Editor lifecycle is owned by the host element; value is synced via the
    // imperative effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reflect external value updates (e.g. autosaved query restore).
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (value === valueRef.current) return;
    valueRef.current = value;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
    });
  }, [value]);

  return <div className="min-h-[28px] w-full" ref={hostRef} />;
}
