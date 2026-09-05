# Security policy

## Supported versions

Trace is pre-release software. Security fixes are applied to the latest commit on `main`; older commits and locally modified builds are not supported.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability or accidentally exposed secret.

Use the repository's [private security advisory form](https://github.com/thashreefkv/trace/security/advisories/new) and include:

- the affected component and commit;
- a concise reproduction or proof of concept;
- the likely impact;
- any suggested mitigation; and
- whether the issue is already public or under active exploitation.

Reports involving credential exposure should identify the credential type without pasting the live value. Revoke exposed credentials before sending diagnostic material.

## Security boundaries

Trace is a local-first desktop application, but optional features communicate with Google APIs, Gemini, and user-selected public URLs. The Siri/Shortcuts API is intended only for localhost and a private Tailscale network and still requires its bearer token.

The following are not security boundaries:

- AI-generated text or decisions;
- a machine account that is already compromised;
- intentionally installed, locally modified builds; or
- content a user explicitly exports from Trace.
