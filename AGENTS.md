# Repository Guidelines

## Project Structure & Module Organization
Matugen Studio is a Tauri 2 desktop app with three main parts:
- `src/`: React + TypeScript frontend (`App.tsx`, `pages/*`, `utils/*`).
- `src-tauri/`: Tauri Rust bridge and IPC commands (`src/commands/*.rs`).
- `matugen-core/`: vendored Rust core library used by the Tauri layer.

Static assets live in `public/` (web) and `src-tauri/icons/` (app icons). Keep frontend UI logic in `src/pages/` and IPC-facing backend logic in `src-tauri/src/commands/`.

## Build, Test, and Development Commands
- `npm run dev`: run Vite frontend only.
- `npm run tauri dev`: run full desktop app (frontend + Tauri backend).
- `npm run build`: TypeScript check + Vite production build.
- `npm run tauri build`: build desktop binaries.
- `cd src-tauri && cargo build`: compile Rust bridge.
- `cd matugen-core && cargo build`: compile core library.
- `cd matugen-core && cargo test`: run Rust tests in core library.

## Coding Style & Naming Conventions
Use TypeScript + Rust idioms already present in each module:
- TypeScript: 2-space indentation, `PascalCase` for components, `camelCase` for functions/variables.
- Rust: `snake_case` for functions/modules, `PascalCase` for structs/enums.
- Keep command files focused by domain (`color.rs`, `template.rs`, `desktop.rs`, etc.).

Run formatters/lints before opening PRs:
- Frontend: `npm run build` (includes `tsc` type checks).
- Rust: `cargo fmt` and `cargo clippy` (at least in touched Rust crates).

## Testing Guidelines
Automated tests currently exist mainly in `matugen-core` (Rust unit tests). Add tests close to changed logic:
- Rust tests inside affected module with `#[cfg(test)]`.
- Prefer deterministic color/template parsing cases.

Run `cd matugen-core && cargo test` before submitting backend/core changes.

## Commit & Pull Request Guidelines
Recent history follows short Conventional Commit style, e.g.:
- `fix: dialog exception`
- `fix: salvar presets`

Use `<type>: <summary>` (`fix`, `feat`, `refactor`, `docs`, `chore`), keep subject imperative and concise.

PRs should include:
- clear scope and rationale;
- linked issue (if available);
- screenshots/GIFs for UI changes;
- test/build commands executed and results.

<!-- prowl-agent -->
## Prowl project context

This repo has a Prowl index of its files, symbols, and how they connect. For any
semantic or structural question -- where code is, what it does, who calls it, or
what a change touches -- **run the read-only prowl CLI first**; do not grep or
read whole files just to locate things. Prowl reindexes what changed before each
query, so answers stay current and are cited to file:line, returned in one call
instead of a grep hit list you then open files to disambiguate.

| Question | First command |
|---|---|
| Map the repository | `prowl overview` |
| Locate a feature or concept | `prowl search "<question>"` |
| Locate a named symbol | `prowl find <name>` |
| Read one symbol's source | `prowl def <name-or-id>` |
| Inspect a file's structure | `prowl outline <path>` |
| Trace who uses a symbol | `prowl references <name-or-id>` |
| Size a change's blast radius | `prowl impact <path>` |
| Inspect uncommitted work | `prowl wip` / `prowl changed` |
| Read a located line range | `prowl peek <file:start-end>` |

Keep grep for exact literal or regex text and glob for filename patterns. CLI
output is token-lean TOON by default; add --format human|toon|json|markdown. If
your harness also wires Prowl as an MCP server, the same index is reachable
there; the CLI needs no server and is the first choice.
<!-- /prowl-agent -->

<!-- prowl-agent:map -->
## Prowl project map

Auto-generated from the Prowl index, refreshed on each `overview`/`init`. Prefer retrieving from Prowl (and reading the cited files) over grepping or relying on training memory; this is the current shape of the repo.

- size: 170 files, 5354 symbols, 2382 edges (resolved 538, external deps 308, unresolved 1536)
- languages: rust:55 css:33 json:17 markdown:13 toml:13 tsx:12 typescript:9 yaml:5
- subsystems: matugen-core/src(53,rust) · src/pages(26,tsx) · src-tauri/resources(5,css) · misc(2,json)
- entrypoints: src/main.tsx · src-tauri/src/lib.rs
- central files (most depended-on): matugen-core/src/lib.rs · matugen-core/src/parser/mod.rs · matugen-core/src/parser/context.rs · matugen-core/src/parser/engine.rs · matugen-core/src/parser/errors.rs
- read these guides first: README.md · AGENTS.md

Depth on demand: `prowl find|def|outline|references <name>`, `search <text>`, `context search "<question>"`, `sketch <ui>`.
<!-- /prowl-agent:map -->
