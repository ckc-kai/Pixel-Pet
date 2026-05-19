# CLAUDE.md

Operating manual for AI assistants (Claude Code, Cursor, etc.) working in this repo.
Read this before writing any code, opening any PR, or making any architectural decision.

## 1. Project Context

Pixel Pet is a macOS-first desktop companion: a hand-drawn pixel pet that lives on the desktop, reacts to broad activity patterns, and nudges rest through visual state changes — no popups, no metrics, no productivity pressure.

**Non-goals are load-bearing.** Pixel Pet is NOT a productivity tracker, time tracker, habit scorer, cloud service, or professional sprite editor. If a feature drifts toward any of those, stop and flag it.

**Privacy is non-negotiable.** No cloud, no analytics, no telemetry, no keystroke recording, no screen capture, no productivity metrics. Activity detection answers exactly one question: "has the user been active recently?" Nothing more.

## 2. Tech Stack

- **Shell:** Tauri 2.x
- **Backend:** Rust — all OS-level work (activity detection, file I/O, tray, window mgmt)
- **Frontend:** React 19 + TypeScript + Vite — UI only (pixel editor, pet render, settings)
- **Package manager:** pnpm
- **State:** React built-ins first. Introduce Zustand / Jotai only when prop drilling or context becomes painful — discuss before adding.

### Common commands
- Dev: `pnpm tauri dev`
- Build: `pnpm tauri build`
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml`
- Frontend tests: `pnpm test`
- Lint: `pnpm lint`
- Type check: `pnpm tsc --noEmit`

(Commands materialize as the project scaffolds — update this section when they change.)

## 3. Architecture Discipline

The foundation of this app — Rust ↔ frontend boundary, IPC contracts, state persistence, activity detection strategy — must stay solid because we will keep adding features (auto-start, PNG import/export, manual state editing, more presets, possibly cross-platform).

**Rules:**
- **Confirm before touching foundation.** Any change to Tauri commands, IPC types, persistence schema, crate layout, or the core state machine → STOP and discuss with the user before writing code.
- **No hardcoding** of identifiers, paths, sizes, timings, or state-machine transitions. Use config, enums, or typed constants.
- **Design for extension.** New pet states, new editor tools, new triggers should plug in, not require rewriting the core.
- **OS interaction stays in Rust.** Frontend never reads files, never polls input devices, never touches the OS directly. Always via Tauri commands.
- **Keep the Tauri command surface small and typed.** One thing per command. Shared types live in a single place (e.g. a generated TS file from Rust types).

Architecture decisions get recorded in `docs/architecture.md` (created in a dedicated planning session). Update it when foundational decisions change.

## 4. Performance Budget (HARD limits)

This is a desktop pet. If it eats resources, it dies.

| Metric | Target | Cap |
|---|---|---|
| Idle CPU | < 0.5% | < 1% |
| Active CPU (animating) | < 3% | < 5% |
| RAM | < 60 MB | < 80 MB |
| Binary size | < 15 MB | < 25 MB |
| Cold start | < 800 ms | < 1.5 s |

**Rules:**
- Activity detection polls or subscribes at low frequency (≥ 30 s default; never per-frame).
- Animation uses CSS `transform` / `opacity` / `clip-path` only. No JS `requestAnimationFrame` loops driving layout.
- No background timers running when the pet is idle and unchanged.
- No heavy dependencies without approval. Rule of thumb: any frontend dep > 50 KB gzipped, or any Rust crate pulling > 20 transitive deps — ask first.

## 5. Testing — MVP Pragmatic

Not 80 % coverage. But these are red lines that MUST have unit tests:

- **State machine** — every transition (`startup → working → stretch → tired → sleep`, meal triggers, idle recovery)
- **Activity detector** — throttling, debounce, idle-threshold logic
- **Pixel canvas serialization** — save/load round-trip, palette handling, dimensions
- **Persistence layer** — schema versioning, migration on load

UI is verified manually + a few Playwright smoke flows (first-run draw, state visual change). Don't write brittle DOM assertions for visual things.

Frameworks: `vitest` (frontend), `cargo test` (Rust), `@playwright/test` (E2E).

## 6. Development Workflow

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
2. **Implement** — TDD where §5 demands it; otherwise just write clean code. Ask immediately when unsure mid-implementation; don't barrel through with a guess.
3. **Self-review** — lint, type check, tests. `code-reviewer` on non-trivial diffs. Rust → `rust-reviewer`. Frontend → `typescript-reviewer`. Perf-sensitive → `performance-optimizer`.
4. **Update `progress.md`** — append a session entry (§8).
5. **Open PR** — see §7 for summary format.

### When to stop and ask
Whenever you encounter:
- ambiguous requirements
- multiple reasonable approaches with real trade-offs
- a risk worth surfacing (perf, privacy, UX, data loss, permission scope)
- a foundation / architecture touch
- a new dependency or new system permission
- anything hard to undo

→ **Ask. Don't decide for the user.**

## 7. Pull Request Summaries

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

## 8. progress.md — Session Log

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

## 9. Privacy & Secrets

- **No telemetry, ever.** No analytics SDKs, no crash reporters that phone home without explicit per-install opt-in.
- **No raw input capture.** Activity detection reads "active / idle" — never the actual keys, never the actual mouse coords, never the screen.
- **`.gitignore` is part of privacy.** Personal data, local drawings used in dev, env files, secrets, and any directory matching `*-private/`, `personal/`, `secrets/`, `*.local` must stay out of the repo. When in doubt, add the pattern and ask.
- **Never commit:** `.env*` (except `.env.example`), API keys, tokens, signing certificates, notarization credentials, personal pet drawings used for testing, screenshots containing personal info.
- **New system permission?** Stop and discuss with the user before writing the manifest entry. Document why it's needed.

## 10. Anti-Patterns (DO NOT)

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

## 11. AI Agent / Skill Cheatsheet

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
