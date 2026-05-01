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
