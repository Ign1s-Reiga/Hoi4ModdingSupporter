# AGENTS.md

## Project Description

A support tool for Hearts of Iron IV modding. It loads a mod through its `.mod`
descriptor and provides editors for scripts, national focus trees, localisation
and assets, writing changes straight back into the mod folder.

## Architecture

- **Tauri 2** desktop shell, Rust backend (`src-tauri/`).
- **Next.js 16** App Router frontend, exported statically (`output: "export"`)
  and served from the Tauri window. Every route must be resolvable at build
  time — no server-side rendering, no dynamic route params.
- **Tailwind CSS 4** with shadcn-style copy-in components in
  `src/components/ui/`.
- **Zustand** for the small amount of shared client state (settings, open
  project, file scan).

### Where the work happens

All file system access, script parsing and writing lives in Rust. The frontend
never touches the disk directly; it calls typed wrappers in `src/lib/ipc.ts`,
which are the only place `invoke()` is used.

`src-tauri/src/paradox/` is the core of the tool: a lexer and parser for the
Clausewitz script format that records the byte range of every node, plus an
editor that rewrites those ranges. Editing is always surgical — a save must
leave comments, spacing and unrelated keys exactly as the author wrote them.
Never re-serialise a whole file to write one field.

Two format rules the game enforces and the code must keep:

- Localisation `.yml` files only load with a UTF-8 BOM, so one is always written.
- Some older script files are Windows-1252; they are decoded on read and flagged
  rather than corrupted.

## Code Style

- Follow `.editorconfig` (2 spaces, CRLF; 4 spaces in Rust).
- Rust: no `unwrap()` on anything that can fail at runtime — return `AppError`.
- TypeScript: `strict` mode, no `any`. Prefer deriving state over duplicating it.
- Comments explain why, not what. Skip comments that restate the code.

## Testing

- `pnpm test:rust` — the parser, editor, focus, localisation and settings tests.
  Any change to parsing or writing needs a test that proves the round trip.
- `pnpm typecheck` and `pnpm lint` before finishing frontend work.

## Git Strategy

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
- Branch names are `<type>/<hyphenated-abstract>`, e.g. `feat/focus-canvas`.
- One commit per logical change.

## Want to do

- Event, history and ideology editors with the same form-plus-source approach
  as the focus editor.
- Flag and focus icon importer that writes the `.dds`/`.tga` and the interface
  entries together.
- Cross-file validation: focuses referencing ids that do not exist, localisation
  keys with no definition.
