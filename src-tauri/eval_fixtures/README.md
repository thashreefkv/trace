# Eval fixtures

Labelled scenarios that drive the [Section 9 eval harness](../shared/src/eval.rs). Each fixture is an input + expected outcome; the runner executes the current app code against the fixture and scores the result so regressions surface before a release.

Fixtures live in the `eval_fixtures` table. The starter file [`seed.json`](./seed.json) is a paste-friendly JSON array of ~20 placeholder fixtures (5 per kind). Open Settings → Eval harness → "Import seed" and paste the file contents to create them all at once.

## Importing

The `Import seed` button in the UI accepts the same JSON shape as the `import_eval_fixtures` IPC command:

```json
{
  "fixtures": [ /* array of fixture objects */ ],
  "skip_existing": false
}
```

- `input_json` / `expectation_json` may be **either** a stringified JSON payload or a nested object. The seed file uses nested objects for readability.
- `skip_existing: true` deduplicates by `name`, useful for re-importing without creating duplicates.

## Fixture kinds

### `retrieval` — precision@K

Tests `brain::retrieve_brain_context` against a known ranked list.

```jsonc
{
  "kind": "retrieval",
  "name": "blocked-work",
  "input_json": {
    "query": "what is blocked right now",
    "focus_entity_id": null  // optional; anchors retrieval around a specific entity
  },
  "expectation_json": {
    "expected_entity_ids": ["del_abc", "del_xyz"],
    "top_k": 3                // K for precision@K, defaults to 3
  }
}
```

**Score**: `hits / observed_top_k`. **Pass** when ≥1 hit AND precision ≥ 0.5.

### `ask` — LLM-as-judge rubric

Runs `ask_search`, then asks a more capable Gemini model to score the answer against a 4-dimension rubric (clarity, factuality, citation_accuracy, tone). Each dimension is 0–1; the aggregate is their arithmetic mean.

```jsonc
{
  "kind": "ask",
  "name": "blocker-explanation",
  "input_json": {
    "question": "Why is the auth refactor blocked?",
    "context": null            // optional pre-context
  },
  "expectation_json": {
    "expected_facts": [
      "Waiting on security review",
      "Owner is Alice"
    ],
    "expected_citation_kinds": ["deliverable", "stakeholder"],
    "expected_citation_ids": [],   // optional; stricter than kinds
    "min_aggregate_score": 0.7,    // pass threshold; defaults to 0.7
    "judge_model": "pro"            // "pro" (default, Gemini Pro) or "flash"
  }
}
```

**Score**: judge `aggregate` ∈ [0, 1]. **Pass** when aggregate ≥ `min_aggregate_score`.

The judge call is tracked under `feature="eval_judge"` in `gemini_usage_log`. Pro judge costs roughly 10× Flash per fixture (~$0.01 vs ~$0.001 per run). Use `judge_model: "flash"` on fixtures where speed/cost matter more than nuance.

### `classification` — exact-match or LLM-as-judge

Reads the current `gmail_threads` row for a thread and checks each pinned dimension against the expected value. Two modes:

**Exact-match (default)**: every pinned dimension must match exactly (case-insensitive). Score = `matched / checked`; pass requires score = 1.0.

```jsonc
{
  "kind": "classification",
  "name": "action-required-thread",
  "input_json": { "thread_id": "<gmail thread id>" },
  "expectation_json": {
    "category": "work",              // optional
    "priority": "urgent",            // optional
    "intent": "request",             // optional
    "action_required": true,         // optional
    "thread_state": "needs_reply",   // optional
    "predicted_action": "reply"      // optional; usually paired with judge_soft
  }
}
```

**Soft-judge** (`judge_soft: true`): each pinned dimension is scored 0–1 by the same Gemini judge as Ask fixtures. Synonyms / paraphrases / semantically-equivalent free-form text pass. Use this when `intent` should accept both `"question"` and `"asking"`, or for free-form `predicted_action`.

```jsonc
{
  "kind": "classification",
  "name": "intent-with-synonyms",
  "input_json": { "thread_id": "<gmail thread id>" },
  "expectation_json": {
    "intent": "question",
    "predicted_action": "reply to ask clarifying question",
    "judge_soft": true,
    "judge_model": "pro",        // optional, "pro" (default) or "flash"
    "min_score": 0.7              // optional, default 0.7
  }
}
```

Soft-judge runs land in `gemini_usage_log` under `feature="eval_judge"` just like Ask judge calls.

### `promotion` — capture → task/deliverable/initiative

The fixture format is final, but the runner returns `{ status: "awaiting_section_4" }` until Section 4 ships `suggest_capture_promotion`.

```jsonc
{
  "kind": "promotion",
  "name": "task-hint",
  "input_json": {
    "capture_text": "ping Alex about Q2 pricing",
    "capture_id": null              // optional pre-existing capture
  },
  "expectation_json": {
    "expected_kind": "task",         // "task" | "deliverable" | "initiative"
    "expected_target_id": null       // optional; stronger signal when set
  }
}
```

## Authoring tips

- **Replace placeholder IDs** in [seed.json](./seed.json) with real entity IDs from your workspace. The placeholders (`del_REPLACE_ME_*`, `thread_REPLACE_ME_*`, `stk_REPLACE_ME_*`) are intentional speed bumps so you don't run evals against the wrong data.
- **Ask fixtures depend on workspace state.** A fixture for "Q2 launch is blocked" silently breaks when the blocker resolves. Treat fixtures as documentation of *what should be true now*; rotate them as the workspace evolves.
- **Pin a baseline** (star icon in the UI) after the first passing run so subsequent runs show a delta. The CI runner (`pnpm eval`) fails on `delta < --threshold-delta` (default -0.05).
- **Keep judge model choice deliberate.** Default to Pro for facts/citations, drop to Flash for tone/clarity-dominant fixtures where the cost saving is worth less precision.

## Running

- **UI**: Settings → Eval harness → "Run all" or per-fixture play button.
- **CLI**: `pnpm eval` (human-readable), `pnpm eval:ci` (JSON to stdout). Set `GEMINI_API_KEY` or pass `--api-key` for Ask judge calls.
- **Override DB path**: `pnpm eval -- --db /path/to/data.db --brain /path/to/brain.kuzu`.
- **Regression threshold**: `pnpm eval -- --threshold-delta -0.10` (a 10-point drop fails).
