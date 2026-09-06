# Trace Design System

Clean, minimal, shadow-forward. Zinc palette as the base. Every page should feel like the same app.

---

## Shared primitives (import these, do not hand-roll)

| Symbol | File | Notes |
|---|---|---|
| `<Avatar>` | `src/components/Avatar.tsx` | Hash-based color, xs/sm/md/lg |
| `<StatePill>` | `src/components/StatePill.tsx` | Deliverable + initiative + thread states |
| `<EmptyState>` | `src/components/EmptyState.tsx` | inline / page / hero variants |
| `<Dialog>` / `<DialogConfirm>` | `src/components/ui/Dialog.tsx` | Radix-backed, focus trap, escape |
| `<SidePanel>` | `src/components/ui/SidePanel.tsx` | Right-side drawer, 24/32/48rem |
| `<BottomSheet>` | `src/components/ui/BottomSheet.tsx` | Bottom-anchored sheet, 90vh default |
| `MOTION`, `fadeY`, `fadeIn`, `streamPulse` | `src/lib/motion.ts` | All framer-motion constants |
| `avatarColor`, `initials` | `src/lib/avatar.ts` | Avatar helpers |
| `prepareSortable`, `compareDeliverablesForLens` | `src/lib/sortDeliverables.ts` | O(n) pre-coerce sort |

---

## Shadows & Elevation

Three levels only. Never use `shadow-lg`, `shadow-xl`, or `shadow-md` on cards.

```
card (default)   shadow-[0_2px_12px_rgba(0,0,0,0.06)]   rounded-2xl
card (hover)     shadow-[0_4px_20px_rgba(0,0,0,0.09)]   on hover
dialog / modal   shadow-2xl
inner section    no shadow, border border-zinc-100
```

---

## CSS Utility Classes

Defined in `src/styles/index.css`. Use these instead of Tailwind primitives.

```
.card          rounded-2xl border border-zinc-100 bg-white + card shadow + hover shadow
.card-inner    rounded-xl border border-zinc-100 bg-zinc-50 p-4  (nested sub-cards)
.card-flat     same as .card but no shadow (for cards inside cards)
.btn           base button
.btn-primary   sky-600 background, white text
.btn-secondary zinc-200 border, white bg, zinc-700 text
.btn-ghost     transparent, hover bg-zinc-100
.btn-sm        h-7 px-3 text-[12px]
.btn-lg        h-10 px-5 text-sm
.btn-danger    red destructive action
.field-control input/select base style — ALWAYS use this on inputs
.skeleton      animate-pulse rounded-xl bg-zinc-100
.empty-state   flex min-h-[18rem] flex-col items-center justify-center rounded-2xl border border-dashed border-zinc-200 bg-white px-5 py-12 text-center
.page-kicker   text-[11px] font-semibold uppercase tracking-wider text-zinc-400
.motion-panel  pre-existing card class with shadow (do not rename)
```

---

## Card Anatomy

All primary content cards follow this pattern:

```tsx
<section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
  {/* optional header */}
  <div className="border-b border-zinc-100 px-5 py-4">
    <h2 className="text-sm font-semibold text-zinc-950">Title</h2>
    <p className="mt-1 text-xs text-zinc-400">Subtitle or description</p>
  </div>
  {/* content */}
  <div className="p-5">...</div>
</section>
```

Inner sub-cards (digest items, stat blocks):
```tsx
<div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
```

Dividers between list rows:
```tsx
<div className="divide-y divide-zinc-50">
```

---

## Color Palette

Base: zinc. Accent: sky for interactive/primary, violet for AI/smart features.
**Never use `neutral-`, `gray-`, or `slate-` — zinc only.**

```
Background       bg-white
Page bg          bg-zinc-50 (or just white)
Borders          border-zinc-100 (cards), border-zinc-200 (inputs)
Dividers         divide-zinc-50
Muted text       text-zinc-400
Body text        text-zinc-500 / text-zinc-600
Strong text      text-zinc-900 / text-zinc-950

Primary action   bg-sky-600 hover:bg-sky-700 text-white
Danger           btn-danger (existing class)
AI/Gemini        text-violet-400 / bg-violet-50 text-violet-600

Success state    bg-emerald-50 text-emerald-700
Warning state    bg-amber-50 text-amber-700
Info state       bg-sky-50 text-sky-700
Muted state      bg-zinc-100 text-zinc-400
```

---

## Avatar System

Use the `<Avatar>` component — never hand-roll avatar divs.

```tsx
import { Avatar } from "../components/Avatar";

// In list rows
<Avatar name={person.name} size="sm" />   // h-9 w-9 rounded-full text-[11px]

// In profile headers
<Avatar name={person.name} size="lg" />   // h-14 w-14 rounded-2xl text-lg

// In chips / tight stacks
<Avatar name={person.name} size="xs" className="border-2 border-white" />
```

Sizes: `xs` (h-5 w-5), `sm` (h-9 w-9), `md` (h-10 w-10), `lg` (h-14 w-14 rounded-2xl).

If you need just colors or initials (e.g. for a custom chip):
```tsx
import { avatarColor, initials } from "../lib/avatar";

const color = avatarColor(name); // { bg: "bg-violet-100", text: "text-violet-700" }
const label = initials(name);    // "JD"
```

---

## StatePill

Use `<StatePill>` — never hand-roll inline state badges.

```tsx
import { StatePill } from "../components/StatePill";

<StatePill kind="deliverable" state={d.state} />
<StatePill kind="initiative" status={i.status} />
<StatePill kind="thread" state={thread.state} />
```

Color map:
- `shipped / live / done` → `bg-emerald-50 text-emerald-700`
- `in_review` → `bg-sky-50 text-sky-700`
- `drafting / todo / in_progress` → `bg-amber-50 text-amber-700`
- `backlog / paused / pending` → `bg-zinc-100 text-zinc-600`
- `killed / parked / archived` → `bg-zinc-100 text-zinc-400`

---

## Empty States

Use the `<EmptyState>` component — never hand-roll empty states.

```tsx
import { EmptyState } from "../components/EmptyState";
```

Three variants:

**`inline`** — for empty panels, list sections (default):
```tsx
<EmptyState
  variant="inline"
  icon={Package}
  title="No deliverables yet"
  description="Linked deliverables will appear here."
/>
```

**`page`** — for routes with zero items (uses `.empty-state` CSS class with dashed border):
```tsx
<EmptyState
  variant="page"
  icon={Inbox}
  title="Inbox at zero"
  description="New captures from the tray widget land here."
  cta={{ label: "Capture something", onClick: handleCapture, primary: true }}
/>
```

**`hero`** — for flagship routes, accepts a custom SVG illustration:
```tsx
<EmptyState
  variant="hero"
  illustration={<MountainIllustration />}  // inline SVG, zinc-200 strokes
  title="No initiatives yet"
  description="Group deliverables by the goal they ladder up to."
  cta={{ label: "Create your first initiative", onClick: handleCreate, primary: true }}
/>
```

Custom SVG illustrations: zinc-200 strokes, no fill, 1.5px stroke-width, rounded caps. Keep them inline JSX in the route file (~30-50 lines). `viewBox="0 0 200 140"` with `className="mx-auto mb-5 h-28 w-auto text-zinc-200"`.

---

## Loading Skeletons

Use skeletons for all list/panel loads. **Never show "Loading…" text. Never show spinners inside lists.**
Spinners are only valid inside primary action buttons (Save, Delete, etc.) during mutations.

```tsx
// List skeleton (use .skeleton class)
{isLoading ? (
  <div className="space-y-2 p-5">
    {Array.from({ length: 4 }).map((_, i) => (
      <div key={i} className="skeleton h-12" />
    ))}
  </div>
) : ...}

// Document/content skeleton
<div className="space-y-3 p-6">
  <div className="skeleton h-7 w-1/3" />
  <div className="skeleton h-4 w-full" />
  <div className="skeleton h-4 w-5/6" />
</div>
```

---

## Dialog / Modal Architecture

Four sanctioned patterns. **No new `fixed inset-0` overlays outside `src/components/ui/`.**

### `<Dialog>` — confirmations, creation forms, pickers

```tsx
import { Dialog, DialogConfirm } from "../components/ui/Dialog";

<Dialog
  open={isOpen}
  onOpenChange={(o) => { if (!o) handleClose(); }}
  title="New initiative"
  kicker="Create"
  size="md"      // sm=28rem, md=34rem, lg=44rem, xl=56rem
>
  <Dialog.Body>
    <input className="field-control w-full" ... />
  </Dialog.Body>
  <Dialog.Footer>
    <Dialog.Cancel onClick={handleClose}>Cancel</Dialog.Cancel>
    <Dialog.Action variant="primary" onClick={handleSubmit}>Save</Dialog.Action>
  </Dialog.Footer>
</Dialog>

// Destructive confirmation shortcut
<DialogConfirm
  open={isOpen}
  onOpenChange={(o) => { if (!o) handleClose(); }}
  title="Delete deliverable?"
  description="This action cannot be undone."
  confirmLabel="Delete"
  destructive
  onConfirm={handleDelete}
/>
```

### `<SidePanel>` — right-side detail/edit view

```tsx
import { SidePanel } from "../components/ui/SidePanel";

<SidePanel open={isOpen} onOpenChange={setIsOpen} title="Edit" width="wide">
  {/* inner content stays untouched */}
</SidePanel>
```

Widths: `narrow=24rem`, `default=32rem`, `wide=48rem`.

### `<BottomSheet>` — large rich-UI surfaces

```tsx
import { BottomSheet } from "../components/ui/BottomSheet";

<BottomSheet open={isOpen} onOpenChange={setIsOpen} height="90vh">
  {/* existing sheet inner content */}
</BottomSheet>
```

### Inline edit toggle — in-place editing within detail pages

`isEditing` state toggled inline. No overlay. Used in `InitiativeDetail`, `DeliverableDetail`, `AskWorkspace` turns.

**Z-index layers:**
```
Toasts          z-[60]
Dialogs         z-50
SidePanels      z-50
BottomSheets    z-40
Dropdowns       z-10
```

Backdrop: always `bg-zinc-950/20 backdrop-blur-sm`.

---

## Tab Bar Pattern

Use `MOTION.spring` for the indicator and `fadeY` for content — never inline objects.

```tsx
import { motion, AnimatePresence } from "framer-motion";
import { MOTION, fadeY } from "../lib/motion";

// Tab bar
<div className="relative border-b border-zinc-100">
  <nav className="flex overflow-x-auto">
    {tabs.map((tab) => (
      <button
        className="relative shrink-0 px-5 py-3.5 text-sm transition-colors"
        key={tab.id}
        onClick={() => setActiveTab(tab.id)}
        type="button"
      >
        <span className={activeTab === tab.id
          ? "font-semibold text-zinc-950"
          : "text-zinc-400 hover:text-zinc-700"}>
          {tab.label}
        </span>
        {tab.count != null && tab.count > 0 && (
          <span className="ml-1.5 rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
            {tab.count}
          </span>
        )}
        {activeTab === tab.id && (
          <motion.div
            className="absolute bottom-0 left-0 right-0 h-0.5 bg-zinc-900"
            layoutId="tab-indicator-UNIQUE"   // ← unique per page
            transition={MOTION.spring}
          />
        )}
      </button>
    ))}
  </nav>
</div>

// Tab content
<AnimatePresence mode="wait">
  <motion.div key={activeTab} {...fadeY}>
    {/* content */}
  </motion.div>
</AnimatePresence>
```

Each tab bar on a page needs a **unique `layoutId`** (e.g. `"tab-indicator-stakeholder"`, `"tab-indicator-deliverable"`).

---

## Animation Defaults

All constants live in `src/lib/motion.ts`. Import and spread — never write inline objects.

```tsx
import { MOTION, fadeY, fadeIn, streamPulse } from "../lib/motion";

// Page/section entry (no exit)
<motion.div {...fadeIn}>

// Tab content or AnimatePresence section (with exit)
<motion.div {...fadeY}>

// Sliding indicator (spring)
transition={MOTION.spring}

// AI streaming pulse (opacity loop)
<motion.span {...streamPulse} className="...">

// Hover transitions on interactive elements
className="transition-colors duration-150"
```

---

## Typography Scale

```
page-kicker     text-[11px] font-semibold uppercase tracking-wider text-zinc-400
page title      text-2xl font-semibold tracking-tight text-zinc-950
section title   text-sm font-semibold text-zinc-950
card label      text-[11px] font-semibold uppercase tracking-wider text-zinc-400
body            text-sm leading-6 text-zinc-600
secondary       text-xs text-zinc-500
tertiary        text-xs text-zinc-400
big number      text-3xl font-bold text-zinc-950
```

---

## Badge / Chip Patterns

Use `<StatePill>` for state badges. For raw chip patterns:

```tsx
// Type/category chip
<span className="rounded-md bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold text-zinc-600">
  Label
</span>

// Count badge on tabs or list items
<span className="rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
  {count}
</span>

// AI / violet chip
<span className="rounded-md bg-violet-50 px-1.5 py-0.5 text-[10px] font-semibold text-violet-600">
  Label
</span>
```

---

## Performance Rules

1. **All list-row components must be wrapped in `React.memo`.**
2. **Callbacks passed to memoized children must be wrapped in `useCallback`** — otherwise the memo is defeated on every render.
3. **Derived lists (sorts, filters) must use `useMemo`.**
4. **Sort deliverables with `prepareSortable` + `compareDeliverablesForLens`** from `src/lib/sortDeliverables.ts` — pre-coerces timestamps once (O(n)) instead of O(n log n) Date.parse calls inside the comparator.

```tsx
import { prepareSortable, compareDeliverablesForLens } from "../lib/sortDeliverables";

const sorted = useMemo(
  () => prepareSortable(rawDeliverables).sort(compareDeliverablesForLens),
  [rawDeliverables],
);
```

---

## Stat Display

Two patterns: inline strip (compact) and digest card (prominent).

**Inline strip** — used in profile headers:
```tsx
<div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-zinc-500">
  <span><span className="font-semibold text-zinc-900">{count}</span> label</span>
  <span className="text-zinc-200">·</span>
  <span><span className="font-semibold text-zinc-900">{value}</span> label</span>
</div>
```

**Digest card** — used in overview/summary tabs:
```tsx
<div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
  <div className="mb-3 flex items-center gap-2">
    <IconName className="text-zinc-400" size={13} />
    <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
      Label
    </span>
  </div>
  <div className="flex items-baseline gap-1">
    <span className="text-3xl font-bold text-zinc-950">{value}</span>
    <span className="text-sm text-zinc-400">/unit</span>
  </div>
  <p className="mt-1.5 text-xs text-zinc-500">Supporting detail</p>
</div>
```

---

## Sidebar List Pattern

Left sidebar with search + card list. Used for stakeholders, deliverables, etc.

```tsx
<aside className="min-w-0">
  {/* Header */}
  <div className="mb-4 flex items-end justify-between gap-3">
    <div>
      <p className="page-kicker">Section name</p>
      <h1 className="text-2xl font-semibold tracking-normal text-zinc-950">Title</h1>
    </div>
    <div className="flex gap-2">
      <button className="btn h-8 w-8 px-0" type="button"><RefreshCw size={14} /></button>
      <button className="btn btn-primary h-8 w-8 px-0" type="button"><Plus size={14} /></button>
    </div>
  </div>

  {/* Search */}
  <div className="relative mb-3">
    <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" size={13} />
    <input className="field-control h-9 pl-9 text-sm" placeholder="Search…" />
  </div>

  {/* List items */}
  <nav className="space-y-1.5">
    <Link className={[
      "group flex items-center gap-3 rounded-xl border p-3 transition-all duration-150",
      selected
        ? "border-zinc-200 bg-white shadow-sm"
        : "border-transparent hover:border-zinc-100 hover:bg-white hover:shadow-sm",
    ].join(" ")} to="...">
      <Avatar name={name} size="sm" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold text-zinc-900">{name}</p>
        <p className="truncate text-xs text-zinc-400">{subtitle}</p>
      </div>
      <span className="shrink-0 rounded-full bg-sky-50 px-2 py-0.5 text-[11px] font-semibold text-sky-600">
        {count}
      </span>
    </Link>
  </nav>
</aside>
```

---

## Page Layout

Two-column split for detail pages:

```tsx
<div className="mx-auto grid min-h-full max-w-7xl gap-5 px-5 py-6 xl:grid-cols-[288px_minmax(0,1fr)]">
  <aside>...</aside>
  <main>...</main>
</div>
```

Single-column full pages: `max-w-5xl mx-auto px-5 py-6`
