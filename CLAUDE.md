# Trace contributor notes

## Design system (read before any UI work)

All UI work must follow the design system documented in `.claude/design-system.md`.

Key rules:
- Cards: `rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]`
- Inner sub-cards: `rounded-xl border border-zinc-100 bg-zinc-50 p-4`
- Shadows: only the custom shadow above (cards) and `shadow-2xl` (dialogs). Never `shadow-lg` or heavier on cards.
- Tabs: framer-motion `layoutId` sliding underline. See design-system.md for the full pattern.
- Animations: `AnimatePresence mode="wait"` + `initial/animate/exit` fade+Y. See design-system.md.
- Avatars: hash-based color from `AVATAR_PALETTE`. Always use `avatarColor(name)` + `initials(name)`. See design-system.md.
- Empty states: centered icon (`text-zinc-200`, size 24–36) + short label. See design-system.md.
- Loading: `animate-pulse rounded-xl bg-zinc-100` skeletons. No spinners.
- Typography: follow the scale in design-system.md exactly. `page-kicker` class for eyebrow labels.

## Tauri async rule (critical)

**Never use `tokio::spawn` anywhere in `src-tauri/src/`.  
Always use `tauri::async_runtime::spawn` instead.**

`tokio::spawn` panics at startup because Tauri's `.setup()` callback runs before the Tokio reactor exists. `tauri::async_runtime::spawn` works in both contexts — setup-time background tasks and inside `#[tauri::command]` handlers.

This applies to every new background sync function, command handler, or fire-and-forget task added to the app.

In the `project_manager_shared` crate, use `crate::runtime::spawn` (which delegates to `tokio::task::spawn` on the ambient runtime). Same rule: no raw `tokio::spawn` outside the `runtime` module.

## Database migrations (critical)

Schema is owned exclusively by `project_manager_shared::db::apply_migrations`, which reads files from `src-tauri/migrations/*.sql` and tracks applied steps in `_applied_migrations`. The frontend has no direct SQL capability.

To add a new migration: drop a new `00XX_<name>.sql` file in `src-tauri/migrations/`, add a `pub const X_SQL: &str = include_str!(...)` in `src-tauri/shared/src/db.rs`, and add a new `step_applied(pool, N).await` block in `apply_migrations`.
