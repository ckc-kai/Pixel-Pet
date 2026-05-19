# CLAUDE.md

Operating manual for AI assistants (Claude Code, Cursor, etc.) working in this repo.
Read this before writing any code, opening any PR, or making any architectural decision.

## 0. File Roles

This repo splits its working docs by intent. Stay in your lane.

| File | Purpose | Update when |
|---|---|---|
| `CLAUDE.md` (this file) | **Dev rules + behavioral guardrails.** How we work, what we never do, agent/skill routing, PR format, anti-patterns. Plus tiny project anchors (mission, non-goals, privacy contract, perf budget) — kept here only because the AI must always have them in context. | Workflow, conventions, or guardrails change. |
| `docs/architecture.md` | **Project facts + design decisions.** Full product context, tech stack details, command list, crate layout, IPC, state machine, persistence, activity detection, performance enforcement. | An architecture decision is made or revised. |
| `docs/progress.md` | **Session-by-session log.** What changed, decisions made, open questions, next steps. Newest entry on top. | At the end of every working session. |
| `README.md` | **Public-facing product intro.** What Pixel Pet is, MVP scope, roadmap, license. Written for humans landing on the repo, not for AI. | Product-facing description changes. |

Rule of thumb when adding new content: a *behavioral rule* the AI must follow → `CLAUDE.md`. A *design fact / decision* about how the system is built → `docs/architecture.md`. A *what-I-did-today* note → `docs/progress.md`.

## 1. Project Anchors (minimal)

Short version. Full context, tech stack details, and commands live in `docs/architecture.md` §0.1.

- **What it is:** macOS-first desktop pixel pet. Reacts to broad activity patterns. Nudges rest via visual state, not popups.
- **Non-goals (load-bearing):** NOT a productivity tracker, time tracker, habit scorer, cloud service, or pro sprite editor. Drift toward any of those → stop and flag.
- **Privacy contract:** No cloud, no analytics, no telemetry, no keystroke recording, no screen capture, no productivity metrics. Activity detection answers exactly one question — "has the user been active recently?" Nothing more.
- **Stack (1-liner):** Tauri 2 + Rust (OS work) + React 19 / TS / Vite (UI), pnpm.

## 2. Performance Budget (HARD limits — behavioral guardrail)

This is a desktop pet. If it eats resources, it dies. Enforcement detail in `docs/architecture.md` §0.1.

| Metric | Target | Cap |
|---|---|---|
| Idle CPU | < 0.5% | < 1% |
| Active CPU (animating) | < 3% | < 5% |
| RAM | < 60 MB | < 80 MB |
| Binary size | < 15 MB | < 25 MB |
| Cold start | < 800 ms | < 1.5 s |

Activity detection ≥ 30 s polling, CSS-only animation, no idle timers, ask before any heavy dep.

## 3. Architecture Discipline

The foundation of this app — Rust ↔ frontend boundary, IPC contracts, state persistence, activity detection strategy — must stay solid because we will keep adding features (auto-start, PNG import/export, manual state editing, more presets, possibly cross-platform).

**Rules:**
- **Confirm before touching foundation.** Any change to Tauri commands, IPC types, persistence schema, crate layout, or the core state machine → STOP and discuss with the user before writing code.
- **No hardcoding** of identifiers, paths, sizes, timings, or state-machine transitions. Use config, enums, or typed constants.
- **Design for extension.** New pet states, new editor tools, new triggers should plug in, not require rewriting the core.
- **OS interaction stays in Rust.** Frontend never reads files, never polls input devices, never touches the OS directly. Always via Tauri commands.
- **Keep the Tauri command surface small and typed.** One thing per command. Shared types live in a single place (e.g. a generated TS file from Rust types).

Architecture decisions get recorded in `docs/architecture.md`. Update it when foundational decisions change.

## 4. Testing — MVP Pragmatic

Not 80 % coverage. But these are red lines that MUST have unit tests:

- **State machine** — every transition (`startup → working → stretch → tired → sleep`, meal triggers, idle recovery)
- **Activity detector** — throttling, debounce, idle-threshold logic
- **Pixel canvas serialization** — save/load round-trip, palette handling, dimensions
- **Persistence layer** — schema versioning, migration on load

UI is verified manually + a few Playwright smoke flows (first-run draw, state visual change). Don't write brittle DOM assertions for visual things.

Frameworks: `vitest` (frontend), `cargo test` (Rust), `@playwright/test` (E2E).

## 5. Development Workflow

### Branching
- **Never commit feature work to `main`.** Repo scaffolding/governance docs are the only exception.
- One branch per feature/fix. Naming:
  - `feat/<short-name>` — new feature
  - `fix/<short-name>` — bug fix
  - `refactor/<short-name>` — no behavior change
  - `perf/<short-name>` — performance
  - `chore/<short-name>` — tooling, config, deps
  - `docs/<short-name>` — docs only
- Keep branches short-lived. Rebase on `main` before opening a PR.

### Per-task flow
1. **Plan** — for anything non-trivial, run `/plan`. Restate requirements, enumerate risks, list unknowns. **Stop and ask the user about every unknown** — don't guess on:
   - product behavior decisions
   - architecture / foundation changes
   - new dependencies
   - new system permissions
   - performance trade-offs
   - privacy-adjacent code paths
2. **Search skills / plugins** — Before implementing, scan available skills (`tdd-workflow`, `security-scan`, `github-ops`, `rust-review`, etc.) and MCP plugins. Use existing tools before hand-rolling. Takes 30 seconds and often saves hours.
3. **Implement** — TDD where §4 demands it; otherwise just write clean code. Ask immediately when unsure mid-implementation; don't barrel through with a guess.
4. **Self-review** — lint, type check, tests. `code-reviewer` on non-trivial diffs. Rust → `rust-reviewer`. Frontend → `typescript-reviewer`. Perf-sensitive → `performance-optimizer`.
5. **Update `progress.md` + sync docs** — append a session entry (§7). If any architecture decision was made or a dev problem was resolved, update `docs/architecture.md` and/or this file before ending the session.
6. **Open PR** — see §6 for summary format. Run pre-push review first (see below).

### When to stop and ask
Whenever you encounter:
- ambiguous requirements
- multiple reasonable approaches with real trade-offs
- a risk worth surfacing (perf, privacy, UX, data loss, permission scope)
- a foundation / architecture touch
- a new dependency or new system permission
- anything hard to undo

→ **Ask. Don't decide for the user.**

### Pre-push review (NEVER run `git push` directly)

Before any push, run this checklist and **report the result to the user**. The user decides to push — not Claude.

**Automated checks:**
1. `cargo test --manifest-path src-tauri/Cargo.toml` — all green
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` — clean
3. `cargo fmt --manifest-path src-tauri/Cargo.toml --check` — clean
4. `pnpm lint` + `pnpm tsc --noEmit` — clean (when frontend files changed)
5. Grep for `unwrap()`, `expect()`, `println!`, `dbg!` in non-test production paths
6. All commit messages follow Conventional Commit format (`feat:`, `fix:`, `perf:`, `refactor:`, `chore:`, `docs:`)

**White-box review — four angles (I run these before every push):**
7. **Runnable** — new module's own tests pass; no panic paths in hot code; feature compiles end-to-end with the rest of the app
8. **Compatibility** — grep callers of changed public APIs; IPC command signatures unchanged or additive only; TypeScript bindings regenerated if commands changed; no cross-module type mismatches
9. **Performance** — compare new code against §2 budget; no blocking I/O on async executor; no JS animation loops added; no heavy dep introduced without approval
10. **Dead code** — clippy unused-item warnings absent; no orphaned `TODO`/`FIXME`/`unimplemented!`; commented-out code blocks removed before merge

Report all findings → ask user "OK to push?"

A `PreToolUse` hook in `.claude/settings.json` physically blocks Bash `git push` calls as a backstop.

### Testing split

| Role | Tester | When |
|---|---|---|
| **White-box** — code review, static analysis, unit/integration tests, the 10-point pre-push checklist above | Claude | Every PR, pre-push |
| **Black-box** — UX, visual fidelity, end-to-end feel as a real user | You | After the pixel editor feature is merged and the app opens for the first time |

When the pixel editor feature lands and the app is runnable, I will surface a reminder: **"App is now openable — time for your black-box session."**

### Living documentation

After every session where decisions were made:
- **Architecture decision** (new dep, IPC change, state design, etc.) → update `docs/architecture.md` before ending the session.
- **Dev problem corrected by user** (user says "do X not Y") → record the rule in the relevant CLAUDE.md section. Format: _Problem encountered → Rule going forward_. This prevents the same mistake recurring in future sessions.

## 6. Pull Request Summaries

Summary length scales with task complexity. Pick the tier honestly.

### Tier 1 — Trivial  *(≤ ~50 LOC, single concern, no risk)*
**Format:** 1–2 sentences.
> Example: "Fix typo in startup greeting copy. Pure string change in `pet/messages.ts`."

### Tier 2 — Normal  *(most features and fixes)*
**Format:** ~5–10 lines covering:
- **What** — one line describing the change
- **Tech** — libraries / APIs / patterns used
- **How** — brief approach (2–3 lines)
- **Test plan** — what was verified, manually and automated

### Tier 3 — Large / architectural / urgent  *(multi-file, foundation touch, new dep, perf-critical, security-adjacent)*
**Format:** full structured summary:
- **Context** — why this is needed
- **Decision** — approach chosen + alternatives rejected
- **Tech stack changes** — new deps, version bumps, API changes
- **Implementation flow** — step-by-step what was built
- **Risks & mitigations**
- **Test plan** — unit / integration / manual / perf checks
- **Rollback plan** — how to revert if something breaks
- **Follow-ups** — known TODOs not in this PR

Use Conventional Commit titles: `feat:`, `fix:`, `perf:`, `refactor:`, `chore:`, `docs:`.

## 7. progress.md — Session Log

Every working session ends with an append to `progress.md` so the next session (or the next assistant) can pick up cold without re-reading the repo.

**Detail scales with importance.** Big features, architecture changes, perf work, anything risky → use the full template below. Trivial chores (config tweaks, doc edits, governance files, typo fixes) → 2–3 lines is enough: scope + next. Don't pad small entries to look thorough.

**Format** (newest entry at the top):

```
## YYYY-MM-DD — <branch-name>
**Scope:** one line describing the goal
**Changed:**
- file/path/one.ts — what changed
- file/path/two.rs — what changed
**Decisions:**
- key decision and the reasoning
**Open questions / risks:**
- thing the user still needs to decide
**Next:**
- what should happen in the next session
```

Keep entries terse and durable — a logbook, not a transcript.

## 8. Privacy & Secrets

- **No telemetry, ever.** No analytics SDKs, no crash reporters that phone home without explicit per-install opt-in.
- **No raw input capture.** Activity detection reads "active / idle" — never the actual keys, never the actual mouse coords, never the screen.
- **`.gitignore` is part of privacy.** Personal data, local drawings used in dev, env files, secrets, and any directory matching `*-private/`, `personal/`, `secrets/`, `*.local` must stay out of the repo. When in doubt, add the pattern and ask.
- **Never commit:** `.env*` (except `.env.example`), API keys, tokens, signing certificates, notarization credentials, personal pet drawings used for testing, screenshots containing personal info.
- **New system permission?** Stop and discuss with the user before writing the manifest entry. Document why it's needed.

## 9. Anti-Patterns (DO NOT)

- ❌ Add AI/ML inference, productivity scoring, or time-tracking logic
- ❌ Phone home for anything
- ❌ Run activity detection in JS / frontend
- ❌ Use JS animation loops for the pet (CSS only)
- ❌ Default to dark mode without a design decision
- ❌ Ship generic shadcn/tailwind-template-looking UI for the pixel editor — Pixel Pet has character; the UI should too
- ❌ Hardcode pet states, sizes, or timings as magic numbers
- ❌ Touch foundation code without confirming with the user first
- ❌ Commit feature work straight to `main`
- ❌ Open a PR without updating `progress.md`

## 10. AI Agent / Skill Cheatsheet

| Situation | Use |
|---|---|
| New feature / refactor of any size | `/plan` then `architect` agent if foundation |
| After writing code | `code-reviewer` |
| Rust code changed | `rust-reviewer` |
| Frontend code changed | `typescript-reviewer` |
| Perf-sensitive change | `performance-optimizer` |
| Build broken | `rust-build-resolver` or `build-error-resolver` |
| Dead code suspected | `refactor-cleaner` |
| Security / permission-sensitive | `security-reviewer` |

---

**Golden rule:** when in doubt, stop and ask. Two cheap clarifying questions beat one expensive rebuild.
