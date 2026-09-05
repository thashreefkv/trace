import { forwardRef, useEffect, useImperativeHandle } from "react";
import { useEditor, EditorContent, type Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  Heading1,
  Heading2,
  Italic,
  Link2,
  List,
  ListOrdered,
  Quote,
  Strikethrough,
  Underline as UnderlineIcon,
} from "lucide-react";

export interface RichTextEditorHandle {
  setHtml: (html: string) => void;
  appendText: (text: string) => void;
  focus: () => void;
  clear: () => void;
  getEditor: () => Editor | null;
}

interface Props {
  value: string;
  onChange: (html: string, text: string) => void;
  placeholder?: string;
}

export const RichTextEditor = forwardRef<RichTextEditorHandle, Props>(
  function RichTextEditor({ value, onChange, placeholder }, ref) {
    const editor = useEditor({
      extensions: [
        StarterKit,
        Underline,
        Link.configure({
          openOnClick: false,
          HTMLAttributes: {
            class: "text-sky-600 underline underline-offset-2",
            rel: "noopener noreferrer",
            target: "_blank",
          },
        }),
        TextAlign.configure({ types: ["heading", "paragraph"] }),
      ],
      content: value || "",
      editorProps: {
        attributes: {
          class:
            "prose prose-sm max-w-none focus:outline-none min-h-[280px] px-5 py-4 text-zinc-800 leading-7",
          "data-placeholder": placeholder ?? "Write your reply…",
        },
      },
      onUpdate: ({ editor }) => {
        onChange(editor.getHTML(), editor.getText());
      },
    });

    // Keep editor in sync if `value` is replaced externally (e.g. AI Draft fills).
    useEffect(() => {
      if (!editor) return;
      const current = editor.getHTML();
      if (value !== current && value !== undefined) {
        editor.commands.setContent(value || "", { emitUpdate: false });
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [value, editor]);

    useImperativeHandle(
      ref,
      () => ({
        setHtml: (html: string) => {
          editor?.commands.setContent(html, { emitUpdate: true });
        },
        appendText: (text: string) => {
          if (!editor) return;
          editor
            .chain()
            .focus("end")
            .insertContent(text.replace(/\n\n+/g, "</p><p>").replace(/\n/g, "<br/>"))
            .run();
        },
        focus: () => {
          editor?.commands.focus("end");
        },
        clear: () => {
          editor?.commands.clearContent(true);
        },
        getEditor: () => editor,
      }),
      [editor],
    );

    return (
      <div className="flex h-full flex-col">
        {/* Toolbar */}
        <div className="sticky top-0 z-10 flex shrink-0 flex-wrap items-center gap-0.5 border-b border-zinc-100 bg-white px-3 py-1.5">
          <ToolGroup>
            <ToolButton
              active={editor?.isActive("bold")}
              icon={<Bold size={13} />}
              label="Bold (⌘B)"
              onMouseDown={() => editor?.chain().focus().toggleBold().run()}
            />
            <ToolButton
              active={editor?.isActive("italic")}
              icon={<Italic size={13} />}
              label="Italic (⌘I)"
              onMouseDown={() => editor?.chain().focus().toggleItalic().run()}
            />
            <ToolButton
              active={editor?.isActive("underline")}
              icon={<UnderlineIcon size={13} />}
              label="Underline (⌘U)"
              onMouseDown={() => editor?.chain().focus().toggleUnderline().run()}
            />
            <ToolButton
              active={editor?.isActive("strike")}
              icon={<Strikethrough size={13} />}
              label="Strikethrough"
              onMouseDown={() => editor?.chain().focus().toggleStrike().run()}
            />
          </ToolGroup>

          <Divider />

          <ToolGroup>
            <ToolButton
              active={editor?.isActive("heading", { level: 1 })}
              icon={<Heading1 size={13} />}
              label="Heading 1"
              onMouseDown={() =>
                editor?.chain().focus().toggleHeading({ level: 1 }).run()
              }
            />
            <ToolButton
              active={editor?.isActive("heading", { level: 2 })}
              icon={<Heading2 size={13} />}
              label="Heading 2"
              onMouseDown={() =>
                editor?.chain().focus().toggleHeading({ level: 2 }).run()
              }
            />
          </ToolGroup>

          <Divider />

          <ToolGroup>
            <ToolButton
              active={editor?.isActive("bulletList")}
              icon={<List size={13} />}
              label="Bullet list"
              onMouseDown={() => editor?.chain().focus().toggleBulletList().run()}
            />
            <ToolButton
              active={editor?.isActive("orderedList")}
              icon={<ListOrdered size={13} />}
              label="Numbered list"
              onMouseDown={() =>
                editor?.chain().focus().toggleOrderedList().run()
              }
            />
            <ToolButton
              active={editor?.isActive("blockquote")}
              icon={<Quote size={13} />}
              label="Quote"
              onMouseDown={() => editor?.chain().focus().toggleBlockquote().run()}
            />
          </ToolGroup>

          <Divider />

          <ToolGroup>
            <ToolButton
              active={editor?.isActive({ textAlign: "left" })}
              icon={<AlignLeft size={13} />}
              label="Align left"
              onMouseDown={() =>
                editor?.chain().focus().setTextAlign("left").run()
              }
            />
            <ToolButton
              active={editor?.isActive({ textAlign: "center" })}
              icon={<AlignCenter size={13} />}
              label="Align center"
              onMouseDown={() =>
                editor?.chain().focus().setTextAlign("center").run()
              }
            />
            <ToolButton
              active={editor?.isActive({ textAlign: "right" })}
              icon={<AlignRight size={13} />}
              label="Align right"
              onMouseDown={() =>
                editor?.chain().focus().setTextAlign("right").run()
              }
            />
          </ToolGroup>

          <Divider />

          <ToolButton
            active={editor?.isActive("link")}
            icon={<Link2 size={13} />}
            label="Add link"
            onMouseDown={() => {
              const previous = editor?.getAttributes("link").href as
                | string
                | undefined;
              const url = window.prompt("Link URL", previous ?? "https://");
              if (url === null) return;
              if (url === "") {
                editor?.chain().focus().extendMarkRange("link").unsetLink().run();
                return;
              }
              editor
                ?.chain()
                .focus()
                .extendMarkRange("link")
                .setLink({ href: url })
                .run();
            }}
          />
        </div>

        {/* Editor body */}
        <div className="min-h-0 flex-1 overflow-y-auto bg-white">
          <EditorContent editor={editor} />
        </div>
      </div>
    );
  },
);

function ToolGroup({ children }: { children: React.ReactNode }) {
  return <div className="flex items-center gap-0.5">{children}</div>;
}

function Divider() {
  return <span className="mx-1 h-4 w-px bg-zinc-200" />;
}

function ToolButton({
  active,
  icon,
  label,
  onMouseDown,
}: {
  active?: boolean;
  icon: React.ReactNode;
  label: string;
  onMouseDown: () => void;
}) {
  return (
    <button
      aria-label={label}
      className={`flex h-7 w-7 items-center justify-center rounded-md transition-colors ${
        active
          ? "bg-zinc-100 text-zinc-900"
          : "text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
      }`}
      onMouseDown={(event) => {
        event.preventDefault();
        onMouseDown();
      }}
      title={label}
      type="button"
    >
      {icon}
    </button>
  );
}
