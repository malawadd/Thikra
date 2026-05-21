# Thikra



<p align="center">
  A floating AI copilot for Windows that can think, search, see your screen, operate your desktop, and now transact through Kite Passport.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D4?logo=windows&logoColor=white" alt="Windows 10/11" />
  <img src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React 19" />
  <img src="https://img.shields.io/badge/Rust-stable-CE422B?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Kite-Passport%20Mode%201-0EA5E9" alt="Kite Passport Mode 1" />
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue.svg" alt="Apache 2.0" />
</p>

## What is Thikra?

Thikra is a Windows desktop AI assistant designed to live above your workflow instead of pulling you out of it. You summon it from anywhere, ask a question, attach your screen, run an agentic search, delegate a desktop task, or trigger a paid x402 call through Kite without leaving the app you were already using.

This project began as a Windows adaptation of Thuki and has been shaped into a more ambitious product direction for hackathon use: an always-available desktop companion that combines:

- local-first AI
- optional cloud models
- desktop agent automation
- live search with citations
- screen-aware context
- Kite Passport-powered payments and x402 actions

For this submission, the hero story is simple: **Thikra turns Kite into something you can actually use from an AI desktop interface, not just a docs flow.**

## Why this matters

Most AI assistants can talk.

Very few can:

- live on your desktop
- see what you are working on
- recover from setup friction
- navigate users through operational flows
- and then complete paid API actions through an agent wallet

Thikra is built around that full loop.

With `/kite`, the assistant is not only explaining how Kite works. It can help the user get connected, inspect state, retrieve a payer address, request payment approval, and complete x402-style paid requests from the same interface they use for everything else.

## The experience

### 1. Instant overlay AI

Double-tap `Ctrl` to open Thikra from anywhere on Windows. It appears as a lightweight overlay so the user can ask without switching tabs or opening a browser chatbot.

### 2. Context-aware help

Users can:

- quote selected text
- paste or attach images
- use `/screen` to send a fresh screenshot
- use `Ctrl+Space` to instantly explain highlighted text

### 3. Agentic work

Users can type `/do` and let Thikra operate the desktop for multi-step tasks. This uses the app's existing desktop agent loop, with confirmations for sensitive actions.

### 4. Agentic search

Users can type `/search` to run a local iterative search pipeline that fetches sources, reads pages, and returns grounded answers with citations.

### 5. Kite inside the same interface

Users can type `/kite` and move into Passport and payment flows without leaving the product.

## Kite integration

Thikra integrates Kite Passport in the currently supported Mode 1 shape: **MCP + OAuth + user-owned Passport**.

This implementation is intentionally honest about the current platform model:

- users still create their Passport account in Kite's ecosystem
- users still create their Kite agent in Kite's Portal
- users still paste the MCP URL provided by Kite
- Thikra handles the connection, status checks, payment flow, x402 retries, and setup guidance inside the app

### Supported `/kite` commands

- `/kite setup`
- `/kite connect`
- `/kite status`
- `/kite payer`
- `/kite approve --payee <addr> --amount <amount> --token <symbol> [--merchant <name>]`
- `/kite call --url <https://...> [--method GET|POST] [--body <json>] [--merchant <name>]`

### What `/kite setup` does

`/kite setup` is the onboarding entrypoint.

It can:

- check whether the Kite CLI is installed
- guide the user to the official Kite install flow
- open the Kite Portal or docs
- explain the invite-only/testnet reality of Passport onboarding
- tell the user exactly where to paste the MCP URL
- verify the saved Kite connection from inside the app

### What makes the integration special

The interesting part is not just "we added a payment command."

The interesting part is that Thikra treats Kite as part of an agent workflow:

- deterministic when standard Kite operations are enough
- agentic when setup or connection hits friction
- advisory when the current AI provider cannot safely drive screenshot-based automation

That means `/kite` is not only a command router. It is an orchestration layer.

## Agentic `/kite` mode

- **Deterministic mode:** normal setup, status, payer, approval, and x402 execution
- **Agentic desktop mode:** Thikra can escalate into its desktop agent when setup or recovery gets messy
- **Advisory fallback mode:** if the connected provider cannot support screenshot-driven automation, Thikra falls back to guided troubleshooting instead of dead-ending

### What the agent can do

When Kite setup or recovery becomes messy, Thikra can help:

- open the right Kite pages
- drive the installer flow
- diagnose missing CLI or missing MCP configuration
- guide OAuth/session recovery
- verify connection state
- resume the requested Kite action once the blocking issue is resolved

### Safety boundaries

We were careful not to make "agentic" mean reckless.

- New secrets and sensitive setup values are still entered manually by the user
- Payment approvals require explicit confirmation
- Paid x402 retries do not happen silently
- The agent can use already-saved configuration, but it does not invent user credentials

This makes the system feel helpful without becoming unsafe.

## How Kite payments work in Thikra

The x402 flow is implemented as a real app path, not a fake prompt demo.

High-level flow:

1. User runs `/kite call --url ...`
2. Thikra makes the initial request
3. If the service returns `402 Payment Required`, Thikra extracts the payment requirements
4. Thikra fetches the payer address from Kite
5. Thikra pauses for explicit user approval
6. Thikra asks Kite to approve/sign the payment
7. Thikra retries the service call with the payment payload
8. The final response is returned in chat

This means the product can bridge from conversational intent to paid machine-to-machine access in one interface.

## Architecture

Thikra is a Tauri v2 desktop app with:

- **Rust backend** for native commands, hotkeys, persistence, Kite integration, agent orchestration, and IPC
- **React + TypeScript frontend** for the overlay UI, chat experience, settings, and streamed event rendering
- **SQLite** for local persistence
- **Ollama** for local inference by default
- **Optional cloud providers** for OpenRouter, Anthropic, or OpenAI

### Important subsystems

- `src/App.tsx`
  Main UI orchestration and slash-command routing
- `src/hooks/useOllama.ts`
  Streamed backend communication for normal chat, search, and Kite flows
- `src/hooks/useAgentMode.ts`
  Desktop agent state and completion handling
- `src-tauri/src/agent.rs`
  The native desktop agent loop reused by `/do` and agentic `/kite`
- `src-tauri/src/kite.rs`
  Kite Passport setup, status, approval, x402 orchestration, and agentic escalation
- `src-tauri/src/lib.rs`
  App bootstrapping, command registration, shared state, and Tauri setup

## Privacy and trust model

Thikra is designed to be practical about privacy.

- Local mode works through Ollama on the user's machine
- Conversation data stays local in SQLite
- Cloud mode is optional
- Screenshot-based agent behavior is gated by provider capability and consent
- Kite setup values that may contain sensitive auth material are not treated like normal display config

For Kite specifically:

- the MCP URL is treated as sensitive
- new sensitive values remain manual-entry steps
- financial actions require confirmation

## Demo script for judges

If you want the fastest path to understanding the product, this is the ideal demo:

1. Open Thikra with `Ctrl`
2. Ask a normal AI question
3. Use `/screen` to analyze the current app
4. Use `/search` to fetch live cited information
5. Use `/do` for a short desktop automation task
6. Use `/kite setup` to show guided Kite onboarding
7. Use `/kite status` to verify readiness
8. Use `/kite payer` to retrieve the Passport payer address
9. Use `/kite call --url ...` to demonstrate the x402 payment flow with confirmation

That sequence shows that Thikra is not a single-feature hack. It is a cohesive AI operating layer with Kite embedded into it.

## Getting started

### Prerequisites

- Windows 10 or Windows 11
- [Bun](https://bun.sh)
- [Rust](https://rustup.rs)
- [Ollama](https://ollama.com) for local inference
- Optional: Docker Desktop for sandbox and local search services

### Run from source

```powershell
bun install
bun run dev
```

### Frontend-only dev server

```powershell
bun run frontend:dev
```

### Local model setup

Install Ollama, then pull a model:

```powershell
ollama pull gemma4:e2b
```

Thikra will connect to Ollama at `http://127.0.0.1:11434` by default.

### Search sandbox

`/search` depends on the local search stack described in [docs/agentic-search.md](docs/agentic-search.md).
Use that guide to bring up the search services before demoing the search flow.

### Kite setup

Inside the desktop app:

1. Open Thikra
2. Run `/kite setup`
3. Follow the guided setup flow
4. Install Kite CLI if needed
5. Open Kite Portal
6. Create or access the Passport/agent in Kite's system
7. Paste the MCP URL into Thikra Settings
8. Verify the connection

After that, use `/kite status`, `/kite payer`, `/kite approve`, and `/kite call`.

## Commands users will care about

- `/screen` capture the current screen as context
- `/do` run a desktop agent task
- `/think` enable deeper reasoning
- `/search` run agentic search with citations
- `/kite` run Kite Passport and payment actions
- `/translate`, `/rewrite`, `/tldr`, `/refine`, `/bullets`, `/todos` for fast utility workflows

## Why we think this can win

Hackathon projects often do one of two things:

- they build a nice UI around a model
- or they build a protocol integration with little product framing

Thikra tries to do both well.

It gives Kite a real user-facing home:

- a desktop-native experience
- a conversational interface
- agentic recovery when flows fail
- explicit safety controls for money and secrets
- and a credible path from setup, to connection, to payment-backed API usage

We think that combination is differentiated.
