# Contributing to Trace

Thanks for helping improve Trace. Focused issues and pull requests are easiest to review.

## Before you start

1. Search existing issues and pull requests.
2. For a large behavior or architecture change, open a proposal first.
3. Never include real workspace data, email addresses, API responses, access tokens, screenshots, or credentials in fixtures or bug reports.

## Local setup

Follow the requirements and installation steps in [README.md](README.md). Then run:

```bash
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml -p project-manager-shared
```

For a full Tauri build, stage the MCP sidecar first:

```bash
pnpm build:mcp
pnpm prepare:mcp-sidecar
pnpm tauri build
```

## Pull requests

- Keep one concern per pull request.
- Add or update tests for behavior changes.
- Follow [the Trace design system](.claude/design-system.md) for UI work.
- Treat all email, file, web, OAuth callback, and AI-produced content as untrusted.
- Use parameterized SQL and keep privileged operations behind explicit Tauri commands.
- Describe what you verified and any known limitations.

Security issues should follow [SECURITY.md](SECURITY.md), not the public issue tracker.
