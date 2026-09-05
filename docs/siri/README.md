# "Hey Siri, Ask Trace" — Shortcut setup

Trace exposes a tiny authenticated HTTP API over your Tailscale network so
Apple Shortcuts (and therefore Siri on iPhone, Mac, Watch, and CarPlay) can
ask questions and drop captures into your inbox. This file walks through
building the two Shortcuts by hand.

There's no `.shortcut` file to import — Apple signs those through iCloud and
they can't be authored from source. Each Shortcut below takes ~2 minutes to
build once.

## Prerequisites

1. **Tailscale** installed and signed in on this Mac **and** the iPhone you
   want to use. (Same tailnet — that's the default.)
2. **Trace** running on the Mac. Quit and relaunch once after installing
   Tailscale so the remote HTTP server picks up the interface.
3. **Settings → Connections → Siri & Apple Shortcuts** shows
   "Reachable over Tailscale". If it shows "Tailscale not detected", the
   server hasn't started — relaunch Trace.

## What you'll copy from Settings

Open Trace → Settings → Connections → Siri & Apple Shortcuts:

- **Tailscale URL** — e.g. `http://my-mac.taild123abc.ts.net:8421`. Click
  Copy.
- **Bearer token** — click Reveal, then Copy. Treat like a password.

Have both on the clipboard one at a time as you go through the steps below.

## Shortcut 1 — "Ask Trace"

This is the one you'll trigger with **"Hey Siri, Ask Trace"**.

1. Open **Shortcuts** (on Mac or iPhone — they sync via iCloud).
2. **+** to create a new Shortcut. Rename it **Ask Trace** in the title bar.
3. Add action: **Dictate Text**.
   - Tap the language chip and set whatever you speak in.
   - Tap **Stop Listening** → **On Tap** (so Siri waits for you to finish).
4. Add action: **Get Contents of URL**.
   - URL: `<Tailscale URL>/ask` — paste your URL, append `/ask`.
   - Tap **Show More**:
     - **Method**: POST
     - **Request Body**: JSON
     - Add field: key `question`, value: tap the variable picker → **Dictated
       Text**.
     - **Headers**: add
       - `Authorization` = `Bearer <paste your token>`
       - `Content-Type` = `application/json`
   - Tap the action header → **Timeout** → set to **120 seconds**. Long Ask
     turns with tool calls can take 30–60s; the default 60s is too tight.
5. Add action: **Get Dictionary Value**.
   - Get: **Value**
   - Key: `answer`
   - Dictionary: the **Contents of URL** variable.
6. Add action: **Speak Text**.
   - Text: the **Dictionary Value** from step 5.
   - Tap **Show More** → **Wait Until Finished** = off (so the result sheet
     can render alongside the speech).
7. Add action: **Show Result**.
   - Set the value to the same **Dictionary Value**. This gives you the full
     answer on screen after Siri's spoken summary.
8. Tap **Done** / save.

Test from the same device: tap **Ask Trace** in the Shortcuts app. Siri
prompts; speak a question like "what shipped this week"; the answer is read
back and shown on screen.

Then test by voice: **"Hey Siri, Ask Trace"** → speak your question.

You can also say the question in one shot — Siri will fill the Dictate
prompt with whatever followed the Shortcut name: **"Hey Siri, Ask Trace,
what's blocked right now"**.

## Shortcut 2 — "Trace This"

This drops a thought into your Capture inbox so you can promote it later.

1. **+** new Shortcut, name **Trace This**.
2. Add **Dictate Text** (same as above).
3. Add **Get Contents of URL**:
   - URL: `<Tailscale URL>/capture`
   - Method: POST, Request Body: JSON
   - Field `body` = **Dictated Text**
   - Headers: `Authorization: Bearer <token>` and `Content-Type:
     application/json`
   - Timeout: 30s is plenty (capture writes are fast).
4. Add **Show Notification**:
   - Title: "Captured to Trace"
   - Body: the **Dictated Text** variable.

Triggers: **"Hey Siri, Trace This"** → speak your thought → done.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| "There was a problem running the Shortcut" with no detail | Tailscale tunnel is down on the iPhone. Open the Tailscale app and toggle it. |
| `401 Unauthorized` shown by Shortcuts | Token mismatch. Re-copy from Settings; if you clicked "Regenerate token" recently, update the Shortcut header. |
| Long pause then "request timed out" | Increase the **Timeout** on the Get Contents of URL action to 180s. |
| "Tailscale not detected" in Settings | Trace launched before Tailscale was up. Quit and reopen Trace. |
| Works on Mac, not on iPhone | iPhone isn't on the same tailnet, or Tailscale is paused. Both devices must show each other in the Tailscale machine list. |

## Security notes

- The HTTP server binds **only** to the Tailscale interface and 127.0.0.1.
  It does not listen on Wi-Fi / Ethernet, so devices on the same SSID can't
  see the port.
- The bearer token is a 256-bit random value stored in macOS Keychain. Legacy
  flat-file tokens are migrated to Keychain and removed when first read. If you
  suspect a leak (screenshot, accidental commit, copy-paste into the wrong app),
  click **Regenerate token** in Settings and update your Shortcuts.
- HTTPS is not used inside the tailnet — Tailscale's WireGuard tunnel
  already provides end-to-end encryption between your devices.
- If your Mac sleeps, requests will hang or time out. Enable
  System Settings → Battery → **Wake for network access** if you want
  requests from the iPhone to wake the Mac.
