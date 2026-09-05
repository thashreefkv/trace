// Google Docs API types (mirroring the Rust structs)
interface GDocsTextStyle {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strikethrough?: boolean;
  link?: { url?: string };
}
interface GDocsParagraphStyle {
  namedStyleType?: string;
}
interface GDocsBullet {
  listId: string;
  nestingLevel?: number;
}
interface GDocsTextRun {
  content: string;
  textStyle?: GDocsTextStyle;
}
interface GDocsParagraphElement {
  textRun?: GDocsTextRun;
  startIndex?: number;
  endIndex?: number;
}
interface GDocsParagraph {
  elements: GDocsParagraphElement[];
  paragraphStyle?: GDocsParagraphStyle;
  bullet?: GDocsBullet;
}
interface GDocsStructuralElement {
  paragraph?: GDocsParagraph;
}
export interface GDocsDocument {
  documentId: string;
  title: string;
  body: { content: GDocsStructuralElement[] };
  revisionId?: string;
}

// TipTap node types
type TipTapMark = { type: string; attrs?: Record<string, unknown> };
type TipTapTextNode = { type: "text"; text: string; marks?: TipTapMark[] };
type TipTapNode = {
  type: string;
  attrs?: Record<string, unknown>;
  content?: (TipTapNode | TipTapTextNode)[];
};
export type TipTapDoc = { type: "doc"; content: TipTapNode[] };

function namedStyleToHeadingLevel(style?: string): number | null {
  const map: Record<string, number> = {
    HEADING_1: 1,
    HEADING_2: 2,
    HEADING_3: 3,
    HEADING_4: 4,
    HEADING_5: 5,
    HEADING_6: 6,
  };
  return style ? (map[style] ?? null) : null;
}

export function gdocsToTipTap(doc: GDocsDocument): TipTapDoc {
  const nodes: TipTapNode[] = [];

  for (const el of doc.body.content) {
    if (!el.paragraph) continue;
    const para = el.paragraph;
    const style = para.paragraphStyle?.namedStyleType;
    const headingLevel = namedStyleToHeadingLevel(style);
    const isBullet = !!para.bullet;

    const inlineContent: TipTapTextNode[] = para.elements
      .filter((e) => e.textRun)
      .map((e) => {
        const run = e.textRun!;
        const text = run.content.replace(/\n$/, "");
        if (!text) return null;
        const marks: TipTapMark[] = [];
        if (run.textStyle?.bold) marks.push({ type: "bold" });
        if (run.textStyle?.italic) marks.push({ type: "italic" });
        if (run.textStyle?.underline) marks.push({ type: "underline" });
        if (run.textStyle?.strikethrough) marks.push({ type: "strike" });
        if (run.textStyle?.link?.url)
          marks.push({ type: "link", attrs: { href: run.textStyle.link.url } });
        return {
          type: "text",
          text,
          ...(marks.length ? { marks } : {}),
        } as TipTapTextNode;
      })
      .filter(Boolean) as TipTapTextNode[];

    if (inlineContent.length === 0) {
      nodes.push({
        type: headingLevel ? "heading" : "paragraph",
        attrs: headingLevel ? { level: headingLevel } : undefined,
        content: [],
      });
      continue;
    }

    if (headingLevel) {
      nodes.push({ type: "heading", attrs: { level: headingLevel }, content: inlineContent });
    } else if (isBullet) {
      nodes.push({ type: "__listItem__", content: [{ type: "paragraph", content: inlineContent }] });
    } else {
      nodes.push({ type: "paragraph", content: inlineContent });
    }
  }

  // Wrap consecutive __listItem__ nodes in bulletList
  const wrapped: TipTapNode[] = [];
  let i = 0;
  while (i < nodes.length) {
    if (nodes[i].type === "__listItem__") {
      const items: TipTapNode[] = [];
      while (i < nodes.length && nodes[i].type === "__listItem__") {
        items.push({ type: "listItem", content: nodes[i].content });
        i++;
      }
      wrapped.push({ type: "bulletList", content: items });
    } else {
      wrapped.push(nodes[i]);
      i++;
    }
  }

  return {
    type: "doc",
    content: wrapped.length ? wrapped : [{ type: "paragraph", content: [] }],
  };
}

// ── TipTap → batchUpdate requests ─────────────────────────────────────────────
// Full-replace strategy: delete all existing body content, then insert new content.
// Simpler than diffing; fine for docs < ~50 KB.

export function tipTapToRequests(doc: TipTapDoc): object[] {
  const requests: object[] = [];
  let index = 1;

  function insertText(text: string, style: GDocsTextStyle = {}) {
    requests.push({ insertText: { location: { index }, text } });
    const styleKeys = Object.keys(style).filter((k) => k !== "link");
    if (styleKeys.length > 0) {
      requests.push({
        updateTextStyle: {
          range: { startIndex: index, endIndex: index + text.length },
          textStyle: style,
          fields: styleKeys.join(","),
        },
      });
    }
    index += text.length;
  }

  function processNode(node: TipTapNode | TipTapTextNode) {
    if (node.type === "text") {
      const n = node as TipTapTextNode;
      const style: GDocsTextStyle = {};
      for (const mark of n.marks ?? []) {
        if (mark.type === "bold") style.bold = true;
        if (mark.type === "italic") style.italic = true;
        if (mark.type === "underline") style.underline = true;
        if (mark.type === "strike") style.strikethrough = true;
        if (mark.type === "link") style.link = { url: mark.attrs?.href as string };
      }
      insertText(n.text, style);
      return;
    }

    const n = node as TipTapNode;

    if (n.type === "paragraph" || n.type === "heading") {
      const beforeIndex = index;
      for (const child of n.content ?? []) processNode(child);
      insertText("\n");
      if (n.type === "heading") {
        const level = (n.attrs?.level as number) ?? 1;
        const styleMap: Record<number, string> = {
          1: "HEADING_1",
          2: "HEADING_2",
          3: "HEADING_3",
          4: "HEADING_4",
          5: "HEADING_5",
          6: "HEADING_6",
        };
        requests.push({
          updateParagraphStyle: {
            range: { startIndex: beforeIndex, endIndex: index },
            paragraphStyle: { namedStyleType: styleMap[level] ?? "HEADING_1" },
            fields: "namedStyleType",
          },
        });
      }
      return;
    }

    if (n.type === "bulletList") {
      for (const item of n.content ?? []) {
        const beforeIndex = index;
        for (const child of (item as TipTapNode).content ?? []) processNode(child);
        insertText("\n");
        requests.push({
          createParagraphBullets: {
            range: { startIndex: beforeIndex, endIndex: index },
            bulletPreset: "BULLET_DISC_CIRCLE_SQUARE",
          },
        });
      }
      return;
    }

    for (const child of n.content ?? []) processNode(child);
  }

  for (const node of doc.content) processNode(node);
  return requests;
}
