You are orchestrating a comprehensive, multi-stage architectural review of the `app/src/` crate in this Rust/Iced desktop application. Your goal is to produce a **complete refactor plan document** (`REFACTOR_PLAN.md`) that covers every architectural concern, every file, and every module — with a projected target file structure that is production-perfect.

## Context

This is `app/src/` — the main application crate of a professional futures trading platform built with Rust and Iced v0.14. Read `CLAUDE.md` for full architecture context before proceeding.

The crate contains: `app/`, `chart/`, `components/`, `modals/`, `screen/`, `style/`, `infra/`, `layout.rs`, `window.rs`, `main.rs`.

## Execution Plan

You will execute **3 stages** using an Agent Team. Create a team called `"arch-review"`. Use `TaskCreate` to track all work items. Stages 1 and 2 launch parallel agents; Stage 3 synthesizes.

### Output Files

All findings go to `review/` directory at repo root:
- `review/findings/` — per-agent raw findings (one file per agent)
- `review/REFACTOR_PLAN.md` — final consolidated refactor plan (Stage 3 output)

---

## STAGE 1: Deep Module Exploration (6 parallel agents)

Launch 6 `general-purpose` agents in parallel, one per major module area. Each agent must:
1. Read EVERY file in their assigned scope (no skipping)
2. Analyze against the review criteria below
3. Write findings to `review/findings/{agent-name}.md`

### Agent Assignments

**Agent: `review-app-core`** — Scope: `app/src/app/` + `app/src/main.rs`
- Application bootstrap, service initialization, message types, update routing
- Review: Message enum design (size, variants, nesting), update handler organization, service wiring, global state pattern (`OnceLock` usage), separation between orchestration and logic
- Flag: God-object tendencies, oversized enums, handler functions doing too much, unclear message ownership

**Agent: `review-chart-engine`** — Scope: `app/src/chart/` (all subdirectories)
- Chart trait system, rendering pipeline, interaction handling, overlays, drawing tools, scale, LOD
- Review: Trait design (`Chart` trait surface area), generic bounds complexity, render vs logic separation, cache invalidation patterns, interaction state management, drawing tool architecture
- Flag: Trait bloat, leaky abstractions, rendering code mixed with business logic, excessive generic parameters, unclear ownership between chart types

**Agent: `review-screen-panes`** — Scope: `app/src/screen/` (all subdirectories)
- Dashboard, pane grid, pane content polymorphism, panels (ladder, time&sales), layout management, loading states
- Review: Content enum design, pane lifecycle, message routing from panes to app, panel architecture, loading state management, popout window handling
- Flag: Content enum growing unbounded, pane state scattered across files, tight coupling between pane types and dashboard, panel code duplication

**Agent: `review-components`** — Scope: `app/src/components/` (all subdirectories)
- Reusable UI component library: display, input, layout, overlay, form, primitives
- Review: Component API consistency (do all similar components follow same patterns?), prop/config patterns, style integration, composability, naming conventions, directory organization granularity
- Flag: Inconsistent APIs across similar components, components that are too coupled to app state, missing abstractions, components that should be split or merged, style leaking

**Agent: `review-modals`** — Scope: `app/src/modals/` (all subdirectories)
- All application and pane-level modals: pane settings, connections, downloads, drawing tools, layout, replay, theme
- Review: Modal lifecycle pattern consistency, state management within modals, message patterns, code reuse between similar modals, settings modal organization
- Flag: Copy-paste between modal implementations, inconsistent patterns, modals doing business logic, tight coupling to specific pane types

**Agent: `review-style-infra`** — Scope: `app/src/style/` + `app/src/infra/` + `app/src/layout.rs` + `app/src/window.rs`
- Theming system, infrastructure utilities, layout serialization, multi-window management
- Review: Theme architecture (tokens, palette, style application), style consistency enforcement, layout persistence design, window management patterns, infra utility organization
- Flag: Style values hardcoded outside style module, inconsistent theme application, layout serialization fragility, window state management complexity

### Review Criteria (ALL agents must evaluate these for their scope)

Each agent writes their findings file with these sections:

```markdown
# {Module Name} — Architecture Review Findings

## 1. File Structure Assessment
- Current file/directory organization
- Files that are too large (>400 lines) or too small (<20 lines)
- Misplaced files (wrong directory for their concern)
- Missing module boundaries (files that should be split)
- Suggested reorganization

## 2. Separation of Concerns
- Functions/modules doing too many things
- Business logic mixed with rendering/UI code
- State management concerns leaking across boundaries
- Cross-cutting concerns handled inconsistently

## 3. Code Duplication & Patterns
- Duplicated logic (exact or near-duplicate)
- Patterns that should be abstracted
- Inconsistent approaches to the same problem
- Missing shared utilities

## 4. API Design & Encapsulation
- Public API surface area (too broad? too narrow?)
- Module boundaries and visibility (`pub` vs `pub(crate)` vs private)
- Type design: enums, structs, trait usage
- Function signatures: parameter count, return types, consistency

## 5. Naming & Conventions
- Inconsistent naming patterns
- Unclear or misleading names
- Module naming vs file naming alignment
- Convention deviations from Rust idioms

## 6. Reliability & Maintainability
- Error handling patterns (or lack thereof)
- Unwrap/expect usage in non-test code
- State invariant enforcement
- Testability of the code

## 7. Specific Issues (itemized)
| # | File:Line | Severity | Issue | Suggested Fix |
|---|-----------|----------|-------|---------------|
| 1 | ...       | ...      | ...   | ...           |

## 8. Architectural Recommendations
- Structural changes recommended for this module
- New abstractions or patterns to introduce
- Files/modules to create, merge, split, or relocate
- Dependency direction changes
```

---

## STAGE 2: Cross-Cutting Analysis (4 parallel agents)

After Stage 1 completes, launch 4 more agents that read ALL Stage 1 findings plus do their own targeted analysis:

**Agent: `xcut-message-flow`** — Cross-cutting: Message Architecture
- Trace the complete message flow from user input to state change to view update
- Map every Message enum and its variants across the crate
- Identify: message routing overhead, unnecessary indirection, messages that bypass the hierarchy, messages carrying too much data, missing messages
- Read all `messages.rs`, `mod.rs` files with Message enums, all `update/` handlers
- Output: `review/findings/xcut-message-flow.md`

**Agent: `xcut-state-management`** — Cross-cutting: State & Data Flow
- Map all state types: what is persisted vs ephemeral vs cached
- Trace state ownership and mutation patterns
- Identify: state scattered across too many locations, redundant state, state synchronization issues, state that should be lifted or lowered
- Read all state-related files across every module
- Output: `review/findings/xcut-state-management.md`

**Agent: `xcut-duplication`** — Cross-cutting: Code Duplication & Consistency
- Compare patterns across ALL modules found in Stage 1
- Identify: identical or near-identical code blocks across files, inconsistent approaches to the same pattern in different modules, opportunities for shared abstractions
- Use Grep extensively to find duplicated patterns
- Output: `review/findings/xcut-duplication.md`

**Agent: `xcut-dependency`** — Cross-cutting: Module Dependencies & Coupling
- Map `use` / `mod` / `pub use` across the entire crate
- Build a mental dependency graph
- Identify: circular dependencies, modules importing too broadly, tight coupling between modules that should be independent, modules that serve as "god modules" importing everything
- Assess: Could any module be extracted to its own crate? Are module boundaries at the right level?
- Output: `review/findings/xcut-dependency.md`

### Cross-Cutting Findings Format

```markdown
# {Analysis Name} — Cross-Cutting Findings

## Overview
Summary of analysis scope and methodology

## Key Findings
Numbered list of major findings with evidence (file paths, code references)

## Pattern Map
Visual or tabular representation of the pattern being analyzed (message flow diagram, state ownership map, dependency graph, duplication matrix)

## Recommendations
Prioritized list of changes, each with:
- What to change
- Why (what problem it solves)
- Impact scope (which files/modules affected)
- Risk level (low/medium/high)
```

---

## STAGE 3: Synthesis & Refactor Plan (you, the orchestrator)

After Stages 1 and 2 complete, YOU (the orchestrator) read ALL findings files and produce the final `review/REFACTOR_PLAN.md`.

This document must contain:

```markdown
# Kairos app/src/ — Complete Architectural Refactor Plan

## Executive Summary
- Overall health assessment (1-10 score with justification)
- Top 5 most critical architectural issues
- Estimated scope of refactor (files touched, new files, deleted files)

## Part 1: Architectural Principles
Define the target architectural principles:
- Module boundary rules
- Message architecture rules
- State management rules
- Component design rules
- Naming and convention rules
- File organization rules (max file size, when to split, directory depth)

## Part 2: Target File Structure
Complete projected file tree for `app/src/` after refactor.
Every file listed, with a one-line description of its purpose.
Use this format:
```
app/src/
├── main.rs                    # Entry point, window creation
├── app/
│   ├── mod.rs                 # Kairos struct, new(), subscription()
│   ├── ...
```
Mark files as: [NEW], [MOVED from X], [SPLIT from X], [MERGED from X+Y], [UNCHANGED], [MODIFIED]

## Part 3: Refactor Operations (ordered)
Numbered sequence of refactor operations, grouped into phases.
Each phase can be done as one PR. Operations within a phase are ordered.

### Phase 1: Foundation (no behavior changes)
File moves, renames, module restructuring.
Each operation:
- Operation: What to do (move/rename/split/merge/create)
- From: Source file(s)
- To: Target file(s)
- Rationale: Why
- Risk: Low/Medium/High

### Phase 2: Structural Refactors
Extract abstractions, consolidate duplicates, fix boundaries.
Each operation:
- Operation: What to refactor
- Files affected: List
- Pattern: Before → After (code sketch if helpful)
- Rationale: Why
- Risk: Low/Medium/High

### Phase 3: Architectural Changes
Message redesign, state management changes, trait refactoring.
Each operation as above.

### Phase 4: Polish & Consistency
Naming fixes, API cleanup, visibility fixes, final organization.

## Part 4: Specific Issues Registry
Complete table of every specific issue found across all agents:
| # | Source Agent | File:Line | Severity | Category | Issue | Resolution | Phase |
|---|-------------|-----------|----------|----------|-------|------------|-------|

## Part 5: Risk Assessment
- Breaking change risks
- Compilation cascade risks (changing a core type)
- Behavioral regression risks
- Recommended testing strategy per phase

## Part 6: Metrics
- Files before vs after
- Average file size before vs after
- Module depth before vs after
- Estimated lines of code moved/changed/deleted/added
```

---

## Execution Instructions

1. **Read `CLAUDE.md` first** — understand full architecture before starting
2. **Create the team**: `TeamCreate` with name `"arch-review"`
3. **Create `review/` and `review/findings/` directories**
4. **Create TaskCreate entries** for all 10 agents + synthesis task
5. **Launch Stage 1**: 6 agents in parallel using `Task` tool with `team_name: "arch-review"`. Each agent gets `subagent_type: "general-purpose"` and `mode: "bypassPermissions"`. Give each agent the full review criteria from above plus their specific scope. Tell each agent to read EVERY file in their scope — no sampling, no skipping.
6. **Wait for Stage 1 completion**: All 6 agents must finish before Stage 2
7. **Launch Stage 2**: 4 agents in parallel. Each reads the Stage 1 findings files first, then does their cross-cutting analysis.
8. **Wait for Stage 2 completion**: All 4 agents must finish before Stage 3
9. **Execute Stage 3**: Read all 10 findings files yourself. Synthesize into `review/REFACTOR_PLAN.md`.
10. **Clean up team**: Send shutdown to all agents, delete team.

## Quality Standards

- **COMPLETENESS**: Every `.rs` file in `app/src/` must be read by at least one Stage 1 agent. No file is skipped.
- **SPECIFICITY**: Findings reference exact file paths and line numbers, not vague observations.
- **ACTIONABILITY**: Every finding has a concrete recommended fix.
- **CONSISTENCY**: All agents use the same severity scale: Critical / High / Medium / Low / Info.
- **ACCURACY**: The target file structure in the refactor plan must be complete and internally consistent — every file referenced in operations must exist in the target tree.

Begin execution now. Start by reading CLAUDE.md, then create the team and launch Stage 1.
