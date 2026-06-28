# Vision — GenUI-D: Decoupled Generative UI Framework

> **Status: FUTURE / INSPIRATION. Not built. Not current state.**
> This is the architectural vision for the future **zanto web** frontend. None of
> it exists in the codebase today. It is parked here so it isn't lost and so the
> current-state docs in [`docs/architecture/`](../architecture/overview.md) stay
> strictly about what exists. When any of this is actually built, it graduates
> into `docs/architecture/` with code references — until then it lives here as
> intent only.
>
> Accompanied by a NotebookLM diagram ("Architecture of the Generative UI Agentic
> Shell"): a central "Generative UI Agentic Shell" hub wired to Shell Server
> (Fastify), Shell Client (Svelte 5), Orchestration (MCP & AG-UI), Registry
> (Artifacts & Manifests), Micro-Apps, Plugin Discovery, and CDN-based component
> loading. Save the image alongside this doc when capturing assets.

---

## How this relates to today's zanto (honest read)

The vision is a **Node/Fastify + Svelte web stack** — a different stack from the
current Rust `zanto-core`. Treat it as the *frontend/runtime* layer that would sit
above (or alongside) the core, not a replacement for it. Mapping the overlap so the
bridge is explicit:

**Already true in `zanto-core` (the vision inherits these, doesn't invent them):**
- **MCP as the tool/data adapter** — tools are already defined via `rmcp`
  ([tools.md](../architecture/tools.md)). The "host maintains 1:1 MCP connections"
  model is the natural extension of the current single-process tool router.
- **Human-in-the-loop approval before mutation** — the `Approver` gate
  ([permissions.md](../architecture/permissions.md)) *is* the "explicit HITL
  approval flow." The vision's contribution is to **visualize** the proposed plan,
  not to introduce the gate.
- **ReAct-style loop** — the current `chat()` loop (reason → tool call → observe →
  repeat) is exactly this, minus a plan-visualization surface.
- **Readable agent state** — sessions/messages are already persisted and
  inspectable ([data-model.md](../architecture/data-model.md)).

**Genuinely new, large, and not yet started:**
- Decoupled UI artifact **registry** + CDN-hosted ESM micro-apps.
- **AG-UI protocol** as the shared rendering/state contract (zanto has no UI
  protocol today — outputs are plain text).
- Three UI tiers (Static / Declarative / Open-Ended) + iframe isolation.
- Svelte 5 reactive shell, Chat / Chat+ / Chatless surfaces.
- Enterprise layer: SSO/RBAC, Portkey-style observability, PGVector RAG.

**Tension to resolve before adopting wholesale:**
- The performance KPIs (4ms cold start, <1GB peak) are **Rust-native** numbers.
  The vision pairs them with a **Fastify/Node** shell — those don't automatically
  hold for Node. If those KPIs are real requirements, the orchestration node may
  need to stay Rust (i.e. `zanto-core` exposed over an API), with Svelte as a thin
  client. Decide deliberately; don't inherit the benchmark and the runtime as if
  they came together.
- Enterprise items (Portkey, RBAC, model whitelisting, PII anonymization) are
  far past the current single-user, local-first posture. Sequence them last, or
  fork an "enterprise" track — they shouldn't gate the personal/local MVP.

---

## What zanto actually adopts (the scoped takeaway)

The GenUI-D web stack below is a **tried, separate web architecture** — it is **not
built in this project**. zanto borrows a narrow, deliberate slice of it; the rest is
reference only.

**Borrowed (inspiration for the future zanto desktop client):**
- **Shell shape** — a future Tauri + Svelte desktop app: a **left-pane navigation
  of "solutions" (verticals)**, with the main area = chat + a **canvas** for
  rendered artifacts.
- **Sub-app / micro-app model** — each vertical is a **left-pane link**, a
  self-contained *solution*, not a separately built application.
- **Artifact system** — rich outputs (sheets, HTML dashboards) rendered in the
  canvas, opened in workspace context.

**Dropped (not built here):** the Fastify shell server, CDN/ESM micro-app delivery,
the AG-UI wire protocol, and the entire enterprise layer (SSO/RBAC, Portkey,
PGVector RAG). If/when remote delivery or multi-tenant governance is ever needed,
revisit — but it is out of scope for the local-first MVP.

### The core idea: a vertical = a configured context over one governed core

Clicking a left-pane solution **switches context** and activates that vertical's
**definitive flow**. Every vertical runs on the *same* `zanto-core` already
documented in [`docs/architecture/`](../architecture/overview.md) — same tool
router, same HITL `Approver` gate, same session store. The vertical only supplies
configuration:

| Vertical supplies | Backed by (current core / planned) |
|---|---|
| **Skill** — system prompt + workflow prose for the domain | planned skill loader (`*.md` injected into system prompt) |
| **Data stores** it owns | planned data tools over a workspace-local store |
| **Input** adapters — CSV / PDF / Excel / chat / workspace files / attachments | planned parser tools |
| **Output** artifacts — sheets, docs, CSV, HTML dashboard | planned artifact type + canvas renderer |

A vertical is therefore **a configured context, not a new app**. The governed
execution layer is constant; the brain, the data shape, and the I/O adapters are
what change per solution. (This is the same thesis the chrome proof story makes —
swappable brain, constant governed core — applied to the product surface.)

### First sub-app: Live Finance

The first left-pane solution. Switching to it activates: the finance skill, finance
data stores, the import/categorize/report flows, and a dashboard artifact rendered
on the canvas — with all the input/output formats and user-controlled
rearrangement/views previously brainstormed. It is the first vertical *and* the
template that proves the sub-app pattern generalizes to later verticals with zero
core changes.

> Build-order note: none of the four "backed by (planned)" rows above exist yet.
> They are the concrete backlog this vision implies — data tools → skill loader →
> artifact type → finance skill → desktop shell. Sequenced work, not a single drop.

---

## Project Plan: Decoupled Generative UI Framework (GenUI-D)

*(Preserved verbatim as provided — the source vision.)*

### 1. Architectural Vision and Core Objectives

GenUI-D marks the transition from traditional "screens you navigate" to "outcomes
you request." In the agentic era, interfaces are no longer static paths
pre-designed by developers; they are dynamic artifacts of an agent's reasoning
process. This framework is architected for delegation rather than simple
automation. While automation requires users to drive step-by-step, delegation
allows users to set high-level goals, monitor agentic plans, and intervene safely.

**Core Objectives**

- **Absolute Grounding:** Every generative element must be anchored to the AG-UI
  protocol. This shared contract ensures that agent reasoning, intent, and state
  are visible and renderable, moving AI from "impressive demo" to "trusted tool."
- **Decoupled Discovery:** A strict separation of concerns where the Svelte shell
  and UI artifacts exist independently. Agents discover and load capabilities from
  a centralized registry without requiring shell redeployment.
- **Low-Latency Execution:** Targeting the 4ms cold-start benchmark established by
  Rust-native frameworks. By minimizing orchestration overhead, we eliminate the
  "latency tax" typically associated with agentic loops.

### 2. Foundation: Fastify Shell Server & Artifact Registry

The backend is powered by Fastify, acting as the high-performance orchestration
node. Its primary roles are Plugin Discovery and serving as an Artifact Registry.

**Generative UI Specification Registry** — hosts three tiers of UI artifacts to
balance brand consistency with creative freedom:

| UI Type | Supported Specs | Strengths | Weaknesses |
|---|---|---|---|
| Static | AG-UI, CopilotChat | Maximum visual polish, reliability, strict brand governance | Engineering intensive; manual component authoring |
| Declarative | A2UI, Open-JSON-UI | Scalable, multi-renderer via structured JSON (cards, lists) | Custom UI patterns may be impossible; inconsistent interpretation causes visual drift |
| Open-Ended | MCP Apps | Unlimited creativity; arbitrary HTML or iframes | Hard to secure; styling/brand consistency challenging |

**MCP: The Universal Adapter** — the framework mandates the Model Context Protocol
(MCP) as the communication standard. The shell server acts as the host
application, maintaining 1:1 connections with specific MCP servers. This makes the
LLM independent of the data integration strategy; data sources (filesystems, Slack,
databases) are abstracted behind a unified interface.

### 3. Core Client: Svelte 5 Chat-Centric Shell

**State Management Strategy**

- **Reactive Runes:** `$state` and `$derived` to handle the flux of agent-generated
  data.
- **Context API for Per-User Data:** all per-user state isolated via the Context
  API to prevent data leakage and ensure SSR compatibility.
- **`app/state` for Global Calculations:** global, non-sensitive calculations may
  bypass the Context API via reactive SvelteKit imports like `app/state`, provided
  they store zero per-user data.
- **Modular Isolation:** avoid shared modules exporting state, to keep state
  contextual to the specific component tree during server-side execution.

**Interaction Surfaces**

1. **Chat (Threaded):** conversational flow where GenUI appears inline as cards or
   tool responses.
2. **Chat+ (Co-Creator Workspace):** multi-pane layout — sidebar for dialogue, a
   dynamic canvas for evolving shared artifacts (Figma-AI style).
3. **Chatless (Integrated UI):** agent operates in the background via APIs,
   injecting generative UI into a native interface (e.g. M365 Copilot inline
   editing) without a visible chat thread.

### 4. Micro-App Model: Vertical Team Autonomy

- **Delivery:** teams deploy UI artifacts to the Fastify registry as standalone
  ESM units; the shell fetches them at runtime — updates without core deployments.
- **Shared Contract (AG-UI):** all micro-apps implement AG-UI as the shared
  primitive for context synchronization and event handling, ensuring even
  open-ended artifacts expose a readable state the shell can interpret.
- **Isolation:** "Open-Ended" UI (HTML/MCP Apps) is strictly isolated within
  iframes, preventing CSS leakage so "unlimited creativity" doesn't compromise the
  host.

### 5. AI Orchestration & MCP Integration

The agent is the primary architect of the layout. Using a ReAct-style loop, the
agent selects tools and determines the optimal UI structure.

**MCP Client Lifecycle**

1. **Initialization:** exchange protocol versions and capability declarations.
2. **Message Exchange:** JSON-RPC 2.0 request-response for calling tools and
   reading resources.
3. **Termination:** clean disconnection and resource disposal.

**Human-in-the-Loop (HITL)** — the UI visualizes the agent's reasoning and proposed
plan and implements an explicit approval flow, requiring the user to authorize tool
execution before the agent acts on external systems or sensitive data.

### 6. Enterprise Scaling, Governance, and Observability

- **Governance:** enterprise auth (Azure AD, AWS Cognito), RBAC, and model
  whitelisting to restrict high-cost/unapproved models.
- **Observability (Portkey integration):** centralized layer tracking 40+
  operational metrics — cost allocation by department, response latencies, token
  usage. Virtual API keys with budget thresholds; automated PII anonymization.
- **Knowledge Integration:** RAG using PostgreSQL + PGVector, Hybrid Search
  (BM25 + CrossEncoder) for high-precision retrieval.

### 7. Implementation Roadmap: Phased Rollout

- **Phase 1 — Foundation/Core:** Fastify shell; Svelte 5 rune-based state; basic
  MCP stdio transport.
- **Phase 2 — Registry:** CDN for ESM hosting; Artifact Discovery API; AG-UI
  integration for state sync.
- **Phase 3 — AI Orchestration:** ReAct planning engine; A2UI declarative
  rendering; HITL approval UI components.
- **Phase 4 — Enterprise Scaling:** SSO; Portkey-style observability; Hybrid
  Search RAG; semantic caching.

### 8. Performance Benchmarks and Quality Assurance

**Non-Negotiable Performance KPIs** (targeting Rust-native efficiency vs. ~5,146 MB
peak "Python overhead"):

- **Cold Start:** 4ms.
- **Peak Memory:** < 1,046 MB (~5x efficiency over standard Python frameworks).
- **Success Rate:** 100% for ReAct-style tool calls.

**Testing Workflow**

1. **Isolated Testing:** individual MCP client functions and capability discovery.
2. **Standardization Testing:** AG-UI protocol adherence across micro-apps.
3. **End-to-End Integration:** full LLM-to-UI loops — agent reasoning → correct
   rendering, and HITL flows correctly interrupting tool execution.
