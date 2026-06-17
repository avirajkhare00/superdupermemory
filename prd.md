# Superdupermemory — Product Requirements Document

**Status:** Draft v0.1
**Date:** 2026-06-17
**Owner:** Aviraj Khare
**Repo:** own Cargo workspace

---

## 1. Summary

Superdupermemory is a local-first, Rust-native memory and context layer for AI agents. It gives an agent persistent, evolving facts about a user, project, or session — across conversations and process restarts — and exposes that memory over MCP so any MCP-compatible client (Claude Code, Cursor, Codex CLI, etc.) can read and write it.

It is a standalone product with its own scope, storage model, and eval philosophy: grounding agents in *what's true about a user/session over time*, independent of any other tooling the owner builds or maintains.

## 2. Problem Statement

AI agents are sP1+r4632=1B5B32347E\P0+r2531\P0+r2638\P1+r6B62=7F\P0+r6B49\P1+r6B44=1B5B337E\P1+r6B68=1B4F48\P1+r4037=1B4F46\P1+r6B50=1B5B357E\P1+r6B4E=1B5B367E\tateless between sessions. Every conversation re-establishes context the user has already provided, and every agent invocation re-derives facts a previous invocation already worked out. Existing solutions (Supermemory, mem0, Zep, and similar) solve this but are either hosted SaaS, JS/TS-native, or both — none are a local-first, Rust-native option that fits naturally next to a Rust-based dev-tooling stack.

## 3. Goals

- **V1 goal:** a working extract → store → retrieve → MCP-expose loop, dogfooded on the owner's own agent workflows (Claude Code and other personal projects), with measurable recall quality via a self-built eval harness.
- **North star (multi-quarter, not v1):** a benchmarked, durable, locally-runnable memory layer that at least one external user or project would be stuck without if it disappeared. "World class" is treated as a falsifiable, benchmarked claim to earn, not a label to start with.

## 4. Non-Goals (v1)

- No knowledge-graph / entity-relationship modeling — flat fact storage with simple update/decay rules only.
- No hosted multi-tenant service, no consumer app, no browser extension.
- No third-party connectors (Gmail, Drive, Slack, etc.).
- No contradiction-resolution beyond "newer fact + explicit conflict flag wins, surfaced for review."
- No support for embedding models beyond one local default — pluggable architecture, but only one implementation shipped.

## 5. Target Users / Use Cases

| Priority | User | Use case |
|---|---|---|
| P0 | Owner's own agents | Persistent memory across Claude Code sessions and other personal projects — facts about ongoing projects, decisions made, preferences stated. |
| P1 | Other developers running local coding agents | Same use case, self-hosted, no SaaS dependency, via MCP. |
| P2 (later) | OSS contributors / integrators | Build on top of the storage/embedding traits for their own agent stacks. |

## 6. Scope — V1 Functional Requirements

- **`remember`** MCP tool: accepts free text or structured input, runs it through the extraction pipeline, stores resulting facts with timestamp and source.
- **`recall`** MCP tool: semantic query against stored facts, returns ranked results with confidence/recency metadata.
- **`forget`** MCP tool: explicit deletion by fact ID or query match — required from v1 for basic data-control hygiene, not deferred.
- **Extraction pipeline**: LLM call (local small model or API, configurable) that turns raw conversational text into discrete, attributable facts rather than storing raw chunks only.
- **Update logic**: new fact matching an existing fact's subject updates it; a timestamp + previous-value history is kept (no silent overwrites).
- **CLI**: `superdupermemory index`, `inspect`, `boot` — a broader CLI surface for humans, with a narrower, task-shaped MCP surface for agents.

## 7. Technical Requirements / Architecture

**Workspace layout:** `core` (extraction, update/decay logic), `store` (a `MemoryStore` trait + SQLite implementation), `embed` (an `Embedder` trait + local implementation), `server` (binary wiring the above into an MCP server).

**MCP SDK:** `rmcp` (official Rust SDK, tokio-based) as the default — the reference implementation, actively maintained, tokio-native.

**Storage:** SQLite via `rusqlite`, with the `sqlite-vec` extension for embedded vector search. Chosen over LanceDB for v1 because it's a single file, transactional, and the simplest path to a zero-config, local-first install. LanceDB stays an option if/when scale or multi-modal storage needs exceed what SQLite comfortably handles.

**Embeddings:** local-first via `candle`, behind the `Embedder` trait, with an Ollama-shellout implementation as a faster-to-build fallback if `candle` integration stalls. API-based embedding (OpenAI/Voyage/etc.) is explicitly a config option, not the default — the offline story is a deliberate differentiator.

**Async runtime:** `tokio`, consistent with the MCP SDK requirement.

**Packaging:** standalone binary distributed via Homebrew tap + GitHub release tarballs for macOS ARM64 and Linux x86_64.

**License:** Apache-2.0.

## 8. Success Metrics & Evaluation

- **Eval harness exists before any benchmark is published.** Synthetic multi-session recall/contradiction tasks, built in-house, in the spirit of LongMemEval / LoCoMo / ConvoMem — this is the regression gate for every future change.
- **Comparative benchmark vs. Supermemory** (and any other reachable incumbent) on the same harness, published even if the result isn't flattering.
- **Operational metrics published alongside accuracy:** retrieval latency (target: sub-300ms for v1, tightening over time), memory footprint, cost-per-fact-extracted if an API call is used, throughput under concurrent agent calls.
- **Adoption signal, not vanity metric:** the actual success criterion is "at least one real workflow breaks without it" — not star count or benchmark rank.

## 9. Non-Functional Requirements

- **Durability:** crash-safe writes, backup/restore commands, defined behavior on detected corruption (no silent data loss).
- **Security & privacy:** encryption at rest, explicit deletion (`forget` tool, not just soft-delete), audit log of what was stored/retrieved/deleted. This is treated as a v1-adjacent requirement, not a "later" item — the entire value of the product is sensitive personal/project data, and the security model needs to be defensible early, not bolted on after adoption.
- **Offline capability:** the core loop (extract, store, retrieve) must function with zero network calls when local embedding + local extraction model are configured.

## 10. Milestones / Roadmap

| Phase | Focus | Exit criteria |
|---|---|---|
| 0 | Bootstrap | Workspace scaffolded, traits defined, `rmcp` server compiles and responds to a trivial tool call. |
| 1 | Dogfood loop | `remember`/`recall`/`forget` working end-to-end on owner's own agents for 2+ weeks of real use. |
| 2 | Eval harness + benchmark | Harness built, baseline numbers recorded, comparative run vs. Supermemory published. |
| 3 | Durability + extraction contract | Crash-safe storage shipped; fact/update/decay rules documented as a versioned, testable spec. |
| 4 | Client integrations | Plugin/config support for Claude Code, Cursor, Codex CLI beyond hand-written MCP config. |
| 5 | Security hardening + OSS release | Encryption at rest, deletion audit, public repo with design-decision write-ups documenting key architecture choices. |
| 6 | Operational proof | Latency/footprint/cost numbers published; at least one external dependency established. |

## 11. Risks & Open Questions

- **Scope creep risk:** the temptation to chase Supermemory's full surface (graph, connectors, consumer app) before the core loop is proven. Mitigation: non-goals in Section 4 are binding until Phase 4+ is reached.
- **Bandwidth risk:** this competes for the same attention as other ongoing personal projects and active interviewing. A v1 with a narrower loop than planned is preferable to a stalled v2-scoped build.
- **Personal tool vs. public OSS positioning is not yet decided** — affects how much doc/API polish is required before Phase 5, and should be explicitly revisited once Phase 1 dogfooding is done.
- **Local embedding/extraction model quality is unproven** at this scope — Phase 1 dogfooding should surface whether the local-first constraint costs too much recall quality versus an API-based fallback.
- **MCP protocol is still evolving quickly** (transport options, auth, task primitives have all shipped or changed in the last year) — pin `rmcp` versions deliberately and budget time for migration rather than assuming protocol stability.

## 12. Open Decisions Before/During Build

- Confirm `rmcp` vs. an alternative Rust MCP SDK based on transport/feature needs.
- Confirm local extraction model choice (small local LLM vs. API call) for the fact-extraction step specifically — this is separate from the embedding model decision.
- Confirm whether `forget` needs cascading deletion semantics (e.g. does deleting a fact also delete facts derived from it) before Phase 1 ships.
