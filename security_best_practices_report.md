# Trace security best-practices review

**Review date:** 5 September 2026  
**Target:** sanitized public-release source tree  
**Application:** Trace 0.1.0  
**Languages reviewed:** Rust, TypeScript, and JavaScript configuration  
**Overall result:** no known secret remains in the candidate public tree; all critical and high-severity source findings identified in this review were remediated.

## Executive summary

This review was performed before converting Trace from a private development repository into a public repository. The work combined manual source inspection, credential-pattern scanning, dependency review, permission-boundary analysis, and executable verification.

The original private history was not suitable for publication. It contained an embedded desktop OAuth credential, internal planning material, organization-specific fixture data, and stale browser artifacts. The publication process therefore uses a new root history containing only the sanitized tree. The old private history is retained separately and must never be pushed to the public repository.

The hardening pass made these material changes:

- removed embedded OAuth configuration and implemented the installed-app flow with PKCE and state validation;
- moved Gemini, Google OAuth, and Siri credentials into macOS Keychain;
- added migration and deletion of legacy flat-file credentials;
- removed frontend raw-SQL access and narrowed Tauri opener permissions;
- added production and development Content Security Policies;
- sanitized untrusted email HTML and disabled remote tracking content;
- constrained dynamic navigation and remote preview hosts;
- hardened agent URL fetching against SSRF, DNS rebinding, unsafe redirects, and oversized responses;
- constrained the Siri / Shortcuts API to authenticated localhost and Tailscale listeners with bounded inputs;
- removed unsafe macOS runtime entitlements;
- upgraded vulnerable dependencies and removed an abandoned geometry dependency;
- pinned third-party GitHub Actions and the Git-based Rust dependency to immutable commits;
- added CI, full-history Gitleaks scanning, Dependabot, a security policy, and contribution guidance; and
- removed private planning and tool-output artifacts from the public tree.

This is a best-practices review, not a formal penetration test or guarantee of security.

## Scope and method

### In scope

- React and TypeScript rendering and navigation;
- Tauri capabilities, CSP, native commands, and macOS entitlements;
- Rust HTTP, OAuth, credential, file-opening, email, Drive, Calendar, Gemini, and MCP boundaries;
- dependency manifests and locks;
- Git-tracked and candidate untracked public files;
- CI and repository security configuration; and
- public documentation for unsafe operational guidance.

### Methods

1. Inspected every credential read, write, refresh, disconnect, and migration path.
2. Inspected dynamic HTML rendering, iframe use, Markdown links, operating-system open calls, and remote previews.
3. Inspected every network listener and arbitrary-URL fetch path.
4. Reviewed Tauri permissions, CSP, and macOS entitlements.
5. Compared dependency graphs and advisories, then updated reachable vulnerable packages.
6. Scanned the candidate public file set with Gitleaks 8.29.1 using full redaction.
7. Ran targeted scans for common provider-token formats, private-key headers, embedded Google client identifiers, credential assignments, bearer values, absolute user paths, personal email domains, and ignored secret-file patterns.
8. Built the frontend, checked TypeScript, checked the complete Rust workspace, and ran all Rust tests.

The scan intentionally evaluates the exact candidate public file set, excluding ignored local files, build products, dependency directories, and the private legacy-history archive.

## Findings

| ID | Severity | Finding | Status |
| --- | --- | --- | --- |
| TRACE-SEC-001 | Critical | Embedded Google OAuth client secret and client ID in source and private Git history | Resolved in public tree and excluded from public history |
| TRACE-SEC-002 | High | API keys and bearer/OAuth tokens stored in plaintext support files | Resolved |
| TRACE-SEC-003 | High | Desktop OAuth did not use PKCE or callback state validation | Resolved |
| TRACE-SEC-004 | High | Agent URL retrieval permitted SSRF and unsafe redirect behavior | Resolved |
| TRACE-SEC-005 | High | Untrusted email HTML, remote tracking content, and unsafe embedded navigation | Resolved |
| TRACE-SEC-006 | High | Overbroad frontend SQL and URL-opening capabilities | Resolved |
| TRACE-SEC-007 | Medium | Dynamic links and Drive previews were insufficiently validated | Resolved |
| TRACE-SEC-008 | Medium | Siri / Shortcuts API lacked robust size limits and safe response behavior | Resolved |
| TRACE-SEC-009 | Medium | Unsafe macOS runtime entitlements expanded executable-memory and library-loading privileges | Resolved |
| TRACE-SEC-010 | High | Private planning documents, personal context, and stale browser artifacts were tracked | Resolved in public tree and excluded from public history |
| TRACE-SEC-011 | High | Reachable JavaScript and Rust dependency advisories | Resolved for the supported build graph |
| TRACE-SEC-012 | Medium | Drive disconnect did not clear credentials after Keychain migration | Resolved |
| TRACE-SEC-013 | Medium | CI dependencies were tag-pinned and no automated history secret scan existed | Resolved |

## Remediation details

### TRACE-SEC-001 — Embedded OAuth credential

**Risk:** A desktop binary cannot keep a client secret confidential. A committed value also remains recoverable from Git history after deletion from the latest file.

**Resolution:**

- Trace now reads only the public Desktop-app client ID from **TRACE_GOOGLE_CLIENT_ID**.
- No client secret is accepted or sent to the token endpoint.
- Gmail, Calendar, and Drive share the hardened installed-app implementation.
- The public repository is created from a sanitized root commit rather than the legacy private history.

**Evidence:**

- [src-tauri/shared/src/oauth.rs:39](src-tauri/shared/src/oauth.rs#L39)
- [src-tauri/shared/src/oauth.rs:51](src-tauri/shared/src/oauth.rs#L51)
- [src-tauri/shared/src/gmail/oauth.rs:36](src-tauri/shared/src/gmail/oauth.rs#L36)
- [src-tauri/shared/src/google_calendar/legacy.rs:37](src-tauri/shared/src/google_calendar/legacy.rs#L37)
- [src-tauri/shared/src/google_drive/legacy.rs:88](src-tauri/shared/src/google_drive/legacy.rs#L88)

**Operational follow-up:** Invalidate the former OAuth client secret in Google Cloud. The sanitized desktop flow does not use it, and it is not part of the candidate public history, but provider-side invalidation remains the correct defense-in-depth action.

### TRACE-SEC-002 — Plaintext credentials

**Risk:** Same-user applications, backups, diagnostics, or accidental commits could expose plaintext credential files.

**Resolution:**

- Production macOS builds use the native Keychain backend through the Rust keyring crate.
- Credential operations share one save/read/clear abstraction.
- Legacy flat-file values are migrated on first access and the old file is removed.
- The private application-support directory is set to mode 0700 on Unix.
- The non-macOS/test fallback is owner-only mode 0600 and is not the supported production path.
- Gemini, Siri, Gmail, Calendar, and Drive use this abstraction.

**Evidence:**

- [src-tauri/shared/src/keychain.rs:29](src-tauri/shared/src/keychain.rs#L29)
- [src-tauri/shared/src/keychain.rs:65](src-tauri/shared/src/keychain.rs#L65)
- [src-tauri/shared/src/keychain.rs:141](src-tauri/shared/src/keychain.rs#L141)
- [src-tauri/shared/src/keychain.rs:155](src-tauri/shared/src/keychain.rs#L155)
- [src-tauri/src/db.rs:28](src-tauri/src/db.rs#L28)

### TRACE-SEC-003 — OAuth PKCE and callback integrity

**Risk:** Missing PKCE and state validation can allow intercepted authorization codes or forged callback requests to be accepted.

**Resolution:**

- Every authorization attempt receives a cryptographically random state and verifier.
- The S256 challenge is sent to Google and the verifier is sent during exchange.
- The callback listener accepts loopback peers only.
- Only GET callbacks are accepted, the request line is capped at 8 KiB, and the returned state must match.
- Callback HTML receives no-store, CSP, nosniff, and no-referrer headers.
- Provider error details are not reflected into the browser response.

**Evidence:**

- [src-tauri/shared/src/oauth.rs:51](src-tauri/shared/src/oauth.rs#L51)
- [src-tauri/shared/src/oauth.rs:85](src-tauri/shared/src/oauth.rs#L85)
- [src-tauri/shared/src/oauth.rs:153](src-tauri/shared/src/oauth.rs#L153)
- [src-tauri/shared/src/oauth.rs:176](src-tauri/shared/src/oauth.rs#L176)

### TRACE-SEC-004 — Server-side request forgery

**Risk:** An agent-controlled URL fetcher could access localhost, LAN services, cloud metadata, reserved networks, or a safe hostname that later redirects or resolves privately.

**Resolution:**

- Only HTTP and HTTPS URLs without embedded credentials are accepted.
- Local names and non-public IPv4/IPv6 ranges are rejected.
- DNS is resolved before the request; every returned address must be public.
- The approved address is pinned into the request client.
- Automatic redirects are disabled. Up to five manual redirects are revalidated.
- HTTPS redirects cannot downgrade to HTTP.
- Requests have a 20-second timeout.
- Only readable text content types are accepted.
- Declared and streamed response bodies are capped at 2 MiB.
- Returned content is sanitized, provenance-wrapped, and logged when suspicious.

**Evidence:**

- [src-tauri/shared/src/gemini/tools.rs:609](src-tauri/shared/src/gemini/tools.rs#L609)
- [src-tauri/shared/src/gemini/tools.rs:624](src-tauri/shared/src/gemini/tools.rs#L624)
- [src-tauri/shared/src/gemini/tools.rs:642](src-tauri/shared/src/gemini/tools.rs#L642)
- [src-tauri/shared/src/gemini/tools.rs:672](src-tauri/shared/src/gemini/tools.rs#L672)
- [src-tauri/shared/src/gemini/tools.rs:711](src-tauri/shared/src/gemini/tools.rs#L711)
- [src-tauri/shared/src/gemini/tools.rs:762](src-tauri/shared/src/gemini/tools.rs#L762)

### TRACE-SEC-005 — Email HTML and embedded content

**Risk:** Email is attacker-controlled HTML. Rendering it directly can enable script execution, tracking requests, form submission, unsafe navigation, and abusive embedding.

**Resolution:**

- DOMPurify processes email HTML with active and embedding elements forbidden.
- Non-inline images lose their source, preventing tracking-pixel loads.
- Links are limited to validated HTTPS and constrained mail links.
- The email body runs in a sandboxed source-document iframe without script permission.
- Its document CSP blocks all network loads except inline data images and local styling.
- Link clicks are intercepted and revalidated before opening.
- General attachment previews use a host allowlist, a restricted CSP, referrer suppression, and iframe sandboxing.

**Evidence:**

- [src/routes/EmailWorkspace.tsx:2883](src/routes/EmailWorkspace.tsx#L2883)
- [src/routes/EmailWorkspace.tsx:2912](src/routes/EmailWorkspace.tsx#L2912)
- [src/routes/EmailWorkspace.tsx:3981](src/routes/EmailWorkspace.tsx#L3981)
- [src/components/AttachmentSheet/AttachmentSheet.tsx:71](src/components/AttachmentSheet/AttachmentSheet.tsx#L71)
- [src/components/AttachmentSheet/AttachmentSheet.tsx:177](src/components/AttachmentSheet/AttachmentSheet.tsx#L177)

### TRACE-SEC-006 — Desktop capability scope

**Risk:** A compromised webview with raw SQL or broad opener access could read or alter the entire workspace or invoke unsafe external handlers.

**Resolution:**

- The frontend SQL plugin and Rust SQL plugin were removed.
- The main capability now exposes core operations, file selection, and HTTPS URL opening only.
- Privileged database operations remain behind typed Tauri commands.
- Production and development CSPs constrain scripts, connections, images, frames, objects, forms, base URLs, and frame ancestors.

**Evidence:**

- [src-tauri/capabilities/default.json:1](src-tauri/capabilities/default.json#L1)
- [src-tauri/tauri.conf.json:22](src-tauri/tauri.conf.json#L22)
- [src-tauri/src/lib.rs:12](src-tauri/src/lib.rs#L12)

### TRACE-SEC-007 — Dynamic navigation

**Risk:** Untrusted records or AI output could supply dangerous schemes, protocol-relative URLs, credential-bearing URLs, or unexpected preview hosts.

**Resolution:**

- Shared frontend validation accepts normalized HTTPS URLs without credentials.
- Citation and Markdown links now reject unsafe schemes, HTTP, and protocol-relative external routes.
- File and Drive commands validate allowed schemes, hosts, identifiers, and local file semantics before opening.
- Tauri's opener capability independently allows HTTPS only.

**Evidence:**

- [src/lib/urlSafety.ts:1](src/lib/urlSafety.ts#L1)
- [src/routes/AskWorkspace/Citations.tsx:326](src/routes/AskWorkspace/Citations.tsx#L326)
- [src/routes/AskWorkspace/Citations.tsx:429](src/routes/AskWorkspace/Citations.tsx#L429)
- [src-tauri/src/commands/files.rs:233](src-tauri/src/commands/files.rs#L233)
- [src-tauri/src/commands/files.rs:358](src-tauri/src/commands/files.rs#L358)

### TRACE-SEC-008 — Siri / Shortcuts API

**Risk:** A private-network API can still leak data or consume paid AI requests if it binds too broadly, accepts an exposed token, reflects internal errors, or allows unbounded requests.

**Resolution:**

- The server does not start unless a Tailscale address is detected.
- Listeners bind only to that Tailscale address and localhost.
- Every route, including health, passes through bearer authentication.
- The fixed-length token is compared without early byte mismatch exit.
- Bodies are limited to 64 KiB; question, context, and capture fields have tighter character limits.
- Remote capture accepts only the thought kind.
- Responses include no-store, nosniff, and restrictive CSP headers.
- Internal errors are mapped to generic API messages.

**Evidence:**

- [src-tauri/src/http_api.rs:44](src-tauri/src/http_api.rs#L44)
- [src-tauri/src/http_api.rs:123](src-tauri/src/http_api.rs#L123)
- [src-tauri/src/http_api.rs:171](src-tauri/src/http_api.rs#L171)
- [src-tauri/src/http_api.rs:201](src-tauri/src/http_api.rs#L201)
- [src-tauri/src/http_api.rs:281](src-tauri/src/http_api.rs#L281)
- [src-tauri/src/http_api.rs:349](src-tauri/src/http_api.rs#L349)

### TRACE-SEC-009 — macOS entitlements

**Risk:** JIT, unsigned executable memory, and disabled library validation unnecessarily weaken the hardened-runtime boundary for this application.

**Resolution:** Those entitlements were removed. The remaining entitlement permits microphone access for user-invoked voice and meeting features.

**Evidence:**

- [src-tauri/Entitlements.plist:1](src-tauri/Entitlements.plist#L1)

### TRACE-SEC-010 — Private artifacts and Git history

**Risk:** Internal planning notes and browser snapshots can expose names, strategy, filesystem paths, work context, or application state even when they do not contain formal API keys.

**Resolution:**

- Private planning material and stale browser artifacts were removed from the candidate public tree.
- Local-only copies are stored below the ignored **.private/** directory.
- Environment files, credentials, certificates, databases, build artifacts, sidecars, coverage, and local tool output are ignored.
- Publication uses a new root commit so removed files and credentials do not remain in reachable public history.

**Evidence:**

- [.gitignore:1](.gitignore#L1)
- [.env.example:1](.env.example#L1)
- [.github/workflows/ci.yml:57](.github/workflows/ci.yml#L57)

### TRACE-SEC-011 — Dependency advisories

**Risk:** Several prior versions had published XSS, denial-of-service, parser, or protocol advisories; one geometry package was abandoned.

**Resolution:**

- React Router and Tiptap packages were upgraded.
- Ammonia, plist/quick-xml, and quinn-proto were upgraded.
- The abandoned hull package was removed and replaced by a small local monotone-chain implementation.
- The production JavaScript audit reports no known vulnerability.
- The supported macOS Cargo feature graph does not compile the RSA crate described in the remaining lockfile-only caveat below.

**Evidence:**

- [package.json:1](package.json#L1)
- [src/components/brain/CommunitiesOverlay.tsx:1](src/components/brain/CommunitiesOverlay.tsx#L1)
- [src-tauri/shared/Cargo.toml:1](src-tauri/shared/Cargo.toml#L1)
- [src-tauri/Cargo.toml:1](src-tauri/Cargo.toml#L1)

### TRACE-SEC-012 — Drive disconnect

**Risk:** After moving tokens into Keychain, one older UI command deleted only database rows and legacy token files. A disconnected account could leave usable Keychain credentials behind.

**Resolution:** The duplicate command was removed. All UI disconnect flows call the shared Drive disconnect operation, which clears both access and refresh entries through the credential abstraction and removes non-secret cache metadata.

**Evidence:**

- [src/lib/files.ts:305](src/lib/files.ts#L305)
- [src-tauri/src/commands/drive.rs:71](src-tauri/src/commands/drive.rs#L71)
- [src-tauri/shared/src/google_drive/legacy.rs:76](src-tauri/shared/src/google_drive/legacy.rs#L76)
- [src-tauri/shared/src/google_drive/legacy.rs:258](src-tauri/shared/src/google_drive/legacy.rs#L258)

### TRACE-SEC-013 — CI and supply chain

**Risk:** Mutable action tags and missing automated secret detection weaken release integrity.

**Resolution:**

- Actions are pinned to immutable commit hashes.
- Workflow permissions default to read-only repository contents.
- Frontend and Rust jobs have explicit timeouts.
- Gitleaks scans complete Git history on every pull request and main-branch push.
- Dependabot tracks npm, Cargo, and GitHub Actions weekly.
- The Git-based Tauri panel dependency is pinned to an immutable revision.

**Evidence:**

- [.github/workflows/ci.yml:1](.github/workflows/ci.yml#L1)
- [.github/dependabot.yml:1](.github/dependabot.yml#L1)
- [src-tauri/Cargo.toml:40](src-tauri/Cargo.toml#L40)

## Verification record

| Check | Result |
| --- | --- |
| TypeScript compiler | Passed |
| Production frontend build | Passed |
| Production npm dependency audit | No known vulnerabilities |
| Complete locked Rust workspace check | Passed |
| Shared Rust library tests | 94 passed, 0 failed |
| MCP server tests | 5 passed, 0 failed |
| Native application tests | 7 passed, 0 failed |
| Total Rust tests | 106 passed, 0 failed |
| Candidate public-tree Gitleaks 8.29.1 scan | No leaks found |
| Targeted provider-token and private-key scan | No matches |
| Targeted hardcoded credential-assignment scan | No matches |
| Targeted absolute user-path and personal-email scan | No sensitive match |
| Git whitespace/error check | Passed |

The frontend build reports a non-failing large-chunk warning. That is a performance optimization opportunity, not a security failure.

## Accepted and remaining risks

### Provider-side invalidation

The former OAuth client secret must be invalidated in its Google Cloud project. It is absent from the candidate public tree and public history, and the new desktop flow does not use it. Repository analysis cannot prove provider-side invalidation.

### Lockfile-only RSA advisory

Cargo lockfile scanners report the Marvin timing advisory against **rsa 0.9.10**, pulled into lock metadata by **sqlx-macros-core → sqlx-mysql**. Trace configures SQLx with default features disabled and SQLite only. Both **cargo tree -i rsa** and **cargo tree -i sqlx-mysql** report no reachable package in the supported macOS workspace graph. There is no fixed RSA release listed for that advisory at review time.

This is accepted as a lockfile-only advisory for the current supported build. Re-evaluate it whenever SQLx changes, another database backend is enabled, or a fixed upstream release becomes available.

### Local data at rest

The SQLite database, graph projection, draft attachments, and audit logs are not encrypted by a Trace-specific key. They rely on macOS account isolation, filesystem mode 0700, FileVault, backups, and physical security. Credentials use Keychain.

### CSS policy

The webview CSP permits inline styles because the current React/Tailwind and source-document rendering paths require them. Scripts remain restricted to self. Removing inline style permission would require a broader rendering refactor.

### External processors

Gemini and Google integrations send task-specific data to provider APIs after configuration. Users remain responsible for provider terms, organization policy, account security, and selecting appropriate data.

### AI and imported content

Prompt wrapping, sanitation, review gates, and confirmation reduce risk but cannot make AI output trustworthy. Imported email, documents, web content, calendar text, MCP input, and model output must continue to be treated as untrusted.

### Release engineering

The default local macOS build is ad-hoc signed and not notarized. A distributable release requires a separate signing, hardened-runtime, notarization, updater, and release-integrity review.

### Independent assurance

This review was source-based and test-based. It did not include an external penetration test, fuzzing campaign, provider-account audit, binary reproducibility analysis, or notarized release assessment.

## Publication requirements

Before making the repository public:

1. publish only the sanitized root history;
2. verify the old private commits are not ancestors of public main;
3. run Gitleaks against the complete new public history;
4. confirm the public tree contains no ignored private files or generated sidecar binary;
5. invalidate the former OAuth client secret in Google Cloud;
6. enable GitHub secret scanning and push protection where the account plan supports them; and
7. keep the legacy archive private or delete it only after a separately verified backup.

## Conclusion

The candidate public tree is materially safer than the legacy private repository and passes the automated checks listed above. No critical or high-severity source finding identified by this review remains open in the candidate tree. The two most important operational controls are preserving the clean-history boundary and invalidating the former provider-side credential.
