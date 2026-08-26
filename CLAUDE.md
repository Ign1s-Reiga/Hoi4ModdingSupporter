# CLAUDE.md

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

Three rules that fall out of that, each learned from a bug:

- **Batch additions.** Every new line inserted into a block targets the same
  span just before its closing brace, so two separate insertions collide and
  `apply_edits` keeps only the first. Go through `BlockEditor`, which collects
  them into one insertion.
- **Skip no-op edits.** If a field already says what it is being set to, emit
  no edit at all. Rewriting an unchanged `prerequisite` block would throw away
  the comments inside it.
- **Round-trip the encoding.** A file read as Windows-1252 is written back as
  Windows-1252; only text that no longer fits falls back to UTF-8, and the
  caller is told. Never silently transcode a file the user only partly edited.

Localisation is the one deliberate exception: `.yml` files are always written
as UTF-8 with a BOM, because the game ignores them otherwise. Entry lines that
did not change are still copied through untouched.

## Code Style

- Follow `.editorconfig` (2 spaces, CRLF; 4 spaces in Rust).
- Rust: no `unwrap()` on anything that can fail at runtime — return `AppError`.
- TypeScript: `strict` mode, no `any`. Derive state rather than mirroring it in
  an effect; `react-hooks/set-state-in-effect` is enforced.
- Comments explain why, not what. Skip comments that restate the code.

## Testing

- `pnpm test:rust` — the parser, editor, focus, localisation, settings and
  asset tests. Any change to parsing or writing needs a test that proves the
  round trip, including what the save must _not_ disturb.
- `pnpm typecheck`, `pnpm lint` and `pnpm fmt` before finishing frontend work.
- `pnpm desktop` runs the app; `pnpm desktop:build` produces a bundle.

Linting is [oxlint](https://oxc.rs) and formatting is oxfmt, configured in
`oxlint.config.ts` and `oxfmt.config.ts`. Two things to know:

- Suppressions use `oxlint-disable-next-line <plugin>/<rule>`, with the rule
  name exactly as the diagnostic prints it (`next/no-img-element`,
  `react-hooks/exhaustive-deps`, `react/set-state-in-effect`). oxlint also
  honours `eslint-disable` comments, so a stale one keeps working silently —
  which makes it easy to delete one and not notice what it was holding back.
- Passing glob patterns to oxfmt alongside a directory filters the directory
  walk, so `oxfmt src "*.ts"` silently skips every `.tsx` file. The scripts
  pass paths only, and `.oxfmtignore` carves out what other tools own.

## Git Strategy

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
- Branch names are `<type>/<hyphenated-abstract>`, e.g. `feat/focus-canvas`.
- One commit per logical change.
- Commits are GPG-signed; never bypass signing to get a commit through.

## Want to do

- Event, history and ideology editors with the same form-plus-source approach
  as the focus editor.
- Flag and focus icon importer that writes the `.dds`/`.tga` and the interface
  entries together.
- Cross-file validation: focuses referencing ids that do not exist, localisation
  keys with no definition.

<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
