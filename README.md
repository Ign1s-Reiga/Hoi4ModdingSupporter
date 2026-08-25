# Hoi4 Modding Supporter

A desktop companion for Hearts of Iron IV modding: open a mod by its `.mod`
descriptor, then browse, edit and organise everything inside it.

Built with [Tauri 2](https://tauri.app) and [Next.js](https://nextjs.org).
The Rust backend owns all file access and script parsing; the frontend is a
static export served inside the Tauri window.

## Features

- **Project browser** — reads a `.mod` descriptor, resolves the mod folder it
  points at and remembers recent projects.
- **Script editor** — syntax-highlighted editing of any text file in the mod,
  grouped by area (focus trees, events, history, decisions, interface …).
- **National focus editor** — focuses laid out on the game grid; drag a node to
  set its `x`/`y`, edit prerequisites and rewards in a form, add or delete
  focuses. Saving rewrites only the value that changed, so comments and
  formatting survive.
- **Localisation editor** — edit keys, versions and text in a table. Comments
  and untouched lines are preserved and every save writes the UTF-8 BOM the
  game requires.
- **Asset browser** — mod and game art side by side, with `.dds` and `.tga`
  decoded for preview.

## Requirements

- [Node.js](https://nodejs.org) 20 or later and [pnpm](https://pnpm.io) 10 or later
- [Rust](https://rustup.rs) 1.77 or later
- Windows: [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)
  (preinstalled on Windows 11 and up-to-date Windows 10) and the MSVC build tools

## Development

```bash
pnpm install
pnpm desktop
```

`pnpm desktop` starts the Next.js dev server and opens the Tauri window against
it. Opening `http://localhost:3000` in a normal browser shows the UI but no
backend commands are available there.

| Command | What it does |
| --- | --- |
| `pnpm desktop` | Run the app in development |
| `pnpm desktop:build` | Build a release bundle (NSIS installer on Windows) |
| `pnpm test:rust` | Run the backend test suite |
| `pnpm typecheck` | Type-check the frontend |
| `pnpm lint` | Lint the frontend |
| `node scripts/generate-icon.mjs` | Redraw `assets/app-icon.png` |

## Layout

```
src/                 Next.js frontend (App Router, static export)
  app/               Routes: home, settings, workspace sections
  components/        Shell, editors and UI primitives
  lib/               Typed IPC wrappers, store, shared helpers
src-tauri/src/       Rust backend
  paradox/           Clausewitz script lexer, parser and span-based editor
  focus.rs           National focus reading and writing
  localisation.rs    Localisation reading and writing
  project.rs         Descriptor parsing and file scanning
```

The WinUI 3 version of this tool lives in the history of this repository, at
the `winui3-final` tag.

## License

MIT — see [LICENSE](LICENSE).
