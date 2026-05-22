# Thikra

<p align="center">
  <strong>The Windows AI copilot that turns Kite Passport into a native product experience.</strong>
</p>

<p align="center">
  Thikra lives above your workflow, understands context, operates your desktop, and lets you install, sign up for, connect, and use Kite Passport without bouncing across docs, scripts, and portals.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D4?logo=windows&logoColor=white" alt="Windows 10/11" />
  <img src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React 19" />
  <img src="https://img.shields.io/badge/Rust-stable-CE422B?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Kite-Passport-native%20hub-0EA5E9" alt="Kite Passport native hub" />
  <img src="https://img.shields.io/badge/x402-ready-111827" alt="x402 ready" />
</p>

## What Thikra is

Thikra is a Windows desktop AI copilot designed for real work, not just chat. It opens instantly as an overlay, understands the screen and the current task, can search the web with citations, operate the desktop, and now gives Kite Passport a full native home inside the app.

This project's core idea is simple:

**Kite should feel like a product, not a pile of setup steps.**

So instead of stopping at a docs link or a raw CLI wrapper, Thikra turns Kite into:

- an in-app setup flow
- a native account and wallet surface
- session and spending controls
- x402 paid API execution
- shopping, cart, checkout, and order visibility
- AI-guided recovery when setup gets messy

## Why this stands out

Most AI apps can answer questions. Some can automate a desktop. Very few can carry a user from:

1. install
2. signup
3. wallet/session readiness
4. payment approval
5. paid API execution
6. shopping and order tracking

all inside one interface.

That is what Thikra does with Kite Passport.

## The Kite-native experience

### Native Kite Hub

Thikra includes a dedicated Kite hub inside the app with product-style sections for:

- **Account**: signup state, login state, pending signup code, logout, and current identity
- **Wallet**: payer address, balances, faucet actions, and token send flows
- **Sessions**: registered agent state, active sessions, budget status, approval state, and session selection
- **Activity**: recent wallet, session, payment, and order events
- **Shopping**: product search, cart contents, checkout readiness, orders, and delivery tracking
- **Developer / Payments**: MCP connection health, payer state, approval state, and x402 request visibility

This is not a raw config page. It is a proper operational surface for Kite inside a desktop AI app.

### End-to-end Passport onboarding

`/kite setup` is designed to carry a new user through the real journey:

- install Kite Agent Passport on Windows
- work around the current hosted installer issues with a native app-managed install path
- bootstrap `kpass` skills
- collect a signup email
- start Kite signup from inside the app
- resume with verification code entry
- guide the user into the Kite Portal when needed
- save and verify the MCP URL

The result is a setup flow that feels dramatically more approachable than "copy this script and hope it works."

### Wallets, sessions, and activity

Once connected, Thikra exposes the useful Kite primitives people actually need:

- retrieve wallet balances
- send tokens
- request testnet faucet funds
- inspect active and expired sessions
- select a current spending session
- review recent account activity

These capabilities are available both from the hub and from chat.

### Paid APIs with x402

Thikra can bridge from conversation to paid machine access:

1. user asks to use a paid endpoint
2. Thikra detects or receives the Kite intent
3. the first request is made
4. if the service returns `402 Payment Required`, Thikra negotiates the payment flow
5. payer details are fetched from Kite
6. the user explicitly confirms the payment
7. Thikra retries with the signed payment payload
8. the final service result comes back into chat

This makes x402 feel like a native assistant workflow instead of a separate developer ritual.

### Shopping, cart, checkout, and orders

The current build also maps Kite shopping capabilities into the interface:

- search products
- inspect cart state
- prepare checkout
- view orders
- track order and delivery status

That means Thikra is not only a "wallet demo." It is a real consumer-facing Kite experience.

## Hybrid UX: chat plus product surface

Thikra supports both explicit commands and smart routing.

### Explicit `/kite` command family

The app supports a broad Kite command family, including:

- `/kite setup --email ...`
- `/kite setup --code ...`
- `/kite login --email ...`
- `/kite login --code ...`
- `/kite logout`
- `/kite me`
- `/kite wallet`
- `/kite send --to ... --amount ... --asset ...`
- `/kite faucet --token ...`
- `/kite sessions`
- `/kite session create ...`
- `/kite session use --session-id ...`
- `/kite session status ...`
- `/kite activity`
- `/kite shop search --query "..."`
- `/kite cart`
- `/kite checkout --confirmed yes`
- `/kite orders`
- `/kite call --url ...`

### Hybrid auto-routing

Users do not have to memorize commands for obvious transactional intents.

Thikra can automatically route clear requests like:

- "what's my wallet balance?"
- "send 5 USDC to 0x..."
- "use this paid API"
- "show my recent activity"
- "what orders do I have?"
- "buy me a USB-C cable"

into Kite-backed flows.

That makes the system feel native and conversational without hiding the explicit power-user path.

## Agentic Kite recovery

One of the most important parts of this project is that Kite is not treated like a happy-path-only integration.

When setup or connection gets messy, Thikra can switch between three modes:

- **Deterministic mode** for normal operations
- **Agentic mode** when an AI-capable provider can help navigate setup or recovery
- **Guided mode** when full autopilot is unavailable but the user still needs actionable help

This matters because real users hit friction:

- missing CLI
- broken installer
- no MCP URL yet
- auth/session confusion
- payment approval pauses
- cloud vs local capability differences

Thikra is built to keep the user moving instead of dead-ending at an error string.

## Safety model

We wanted this to feel powerful without becoming reckless.

- payment approvals require explicit confirmation
- new secrets and sensitive values stay manual-entry
- MCP values are treated as sensitive
- the assistant can use already-saved state, but it does not invent credentials
- local and cloud AI modes are clearly separated

The result is an agentic experience that still respects user control.

## Everything else Thikra can do

Kite is the hero, but Thikra is a full desktop copilot:

- instant overlay summoned with a double-tap of `Ctrl`
- `Ctrl+Space` quick explain for highlighted text
- `/screen` for screen-aware help
- `/search` for cited agentic web research
- `/do` for desktop task execution
- support for local models through Ollama
- optional cloud providers like OpenRouter for stronger agentic behavior

## Demo flow for judges

If you want the cleanest high-signal demo, run this:

1. Open Thikra from anywhere with `Ctrl`
2. Ask a normal question to show the base assistant
3. Use `/screen` to analyze the current app
4. Use `/search` to fetch a cited result
5. Run `/kite setup --email you@example.com`
6. Show the native install, signup, and verification flow
7. Open the Kite hub and show Account, Wallet, Sessions, Activity, and Shopping
8. Ask "what's my wallet balance?" and show auto-routing into Kite
9. Ask to use a paid API and show the x402 approval flow
10. Ask to find or buy a product and show shopping/cart/orders

That sequence shows a complete story:

- assistant
- agent
- payments
- commerce
- real operational UX

## Architecture

Thikra is built as a Tauri v2 app with:

- **Rust backend** for native commands, Kite orchestration, hotkeys, setup logic, and IPC
- **React + TypeScript frontend** for the overlay UI, native hub, chat rendering, and streamed flows
- **SQLite** for local persistence
- **Ollama** for local inference
- **optional cloud providers** for stronger model and agent behavior

Important implementation areas:

- `src/App.tsx`
- `src/hooks/useOllama.ts`
- `src/hooks/useAgentMode.ts`
- `src/settings/tabs/AgentTab.tsx`
- `src-tauri/src/kite.rs`
- `src-tauri/src/agent.rs`
- `src-tauri/src/lib.rs`

## Run locally

### Prerequisites

- Windows 10 or Windows 11
- [Bun](https://bun.sh)
- [Rust](https://rustup.rs)
- [Ollama](https://ollama.com) for local inference

### Start the app

```powershell
bun install
bun run dev
```

### Frontend-only mode

```powershell
bun run frontend:dev
```

### Local model setup

```powershell
ollama pull gemma4:e2b
```

### Kite flow

Inside the desktop app:

1. Run `/kite setup --email you@example.com`
2. Follow the install and signup flow
3. Enter the verification code when prompted
4. Open the Kite Portal when needed
5. Paste the MCP URL into the Kite section
6. Verify the connection
7. Use the hub or ask natural language requests like "what's my balance?" or "show my orders"

## Why this can win

This submission does not treat Kite as a side quest.

It treats Kite as a native product layer and solves the hardest part of these integrations: making them usable by real people inside a real interface.

Thikra combines:

- a polished desktop AI shell
- native Passport onboarding
- wallet and session visibility
- x402 execution
- shopping and order flows
- hybrid chat plus dashboard UX
- agentic recovery instead of brittle happy-path demos

That combination is what makes it compelling.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
