# AGENTS.md

Instructions for coding agents working in this repository.

## Project

Mind is a local-first mind map app. A Vue 3 editor (`web/`) runs inside a Tauri 2 shell (`desktop/`) and renders through the engine in `simple-mind-map/`. Documents are `.xmind` files on disk; nothing leaves the machine.

## Layout

| Path                   | Contents                                      |
| ---------------------- | --------------------------------------------- |
| `web/`                 | Editor UI — Vue 3, Element Plus, Vite         |
| `desktop/src-tauri/`   | Rust shell, menus, file handling, updater     |
| `simple-mind-map/`     | Rendering engine, imported as source          |
| `scripts/`             | Bootstrap and version helpers                 |
| `assets/icon/icon.svg` | Source for every app icon and the web favicon |

## Commands

```bash
make            # build and install the desktop app (macOS)
make dev        # desktop app against the local dev server
make web        # web dev server only
cd web && bun install --frozen-lockfile
cd web && npm run build

bun x prettier --check .
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check
cargo check --manifest-path desktop/src-tauri/Cargo.toml

bun web/src/utils/fileLibrary.test.cjs
bun web/src/utils/mermaidMindmap.test.cjs
bun web/src/utils/githubStatus.test.cjs
```

Use Node 20.11 or newer: the build toolchain relies on `import.meta.dirname`.

Dependencies are installed with Bun 1.4+; `bun.lock` in the root, `web/` and `desktop/` is the only lockfile, and CI runs `bun install --frozen-lockfile`, so it fails whenever `package.json` and the lockfile disagree. After changing a dependency run `bun install` in that folder and commit the updated `bun.lock`. The scripts still run on Node — `vite` and `tauri` are resolved through their node shebangs, and the Tauri hooks call `node` / `npm run` directly.

## Conventions

- Run `bun x prettier --check .` after any frontend change, and `cargo fmt` after any Rust change.
- `web` also has eslint (`npm run lint`). It is not wired into CI and currently reports 68 pre-existing problems, so fix what your change touches instead of attempting a repo-wide cleanup.
- Write code comments in Chinese, as the existing files do. Keep user-facing strings in `web/src/lang/` and add every new key to `zh_cn`, `zh_tw`, `en_us` and `vi_vn`.
- Return Chinese, human-readable error strings from Tauri commands.
- Register every new Tauri command in `desktop/src-tauri/build.rs` and grant it in `desktop/src-tauri/capabilities/default.json`.
- Regenerate icons from the SVG instead of editing PNGs: `cd desktop && bunx tauri icon ../assets/icon/icon.svg && cp src-tauri/icons/icon.ico ../web/public/logo.ico`.
- Commit with Conventional Commits (`feat:`, `fix:`, `perf:`, `docs:`, `style:`, `refactor:`, `test:`, `build:`, `ci:`, `chore:`, `revert:`), lowercase and imperative, no trailing period.

## Releases

1. `node scripts/sync-version.mjs vX.Y.Z`, then commit as `chore: release X.Y.Z`.
2. `git tag -a vX.Y.Z -m "Mind X.Y.Z"` and push the tag. `.github/workflows/release.yml` builds the bundles, signs the updater artifacts and publishes `latest.json`.
3. **Mandatory:** once the tag is pushed, update the GitHub Release notes with concise bullet points describing what changed. No `feat:`-style prefixes and no raw commit list.

Updater artifacts are signed with the `TAURI_SIGNING_PRIVATE_KEY` secret; without it the release job fails while composing `latest.json`.

## Pull requests

- One concern per pull request, linked to an issue. CI runs `title`, `format`, `cargo` and `build`; all four must pass before merge. `format` runs on Bun and owns Prettier, and `build` runs the test scripts before bundling.
- PR titles follow Conventional Commits — they become the squash commit subject.
