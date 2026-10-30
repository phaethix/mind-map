<p align="center">
  <img src="./assets/icon/icon.svg" width="120" height="120" alt="Mind" />
</p>

<h1 align="center">Mind</h1>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="License" /></a>
  <a href="https://github.com/phaethix/mind-map/releases"><img src="https://img.shields.io/github/v/release/phaethix/mind-map?label=release" alt="Release" /></a>
  <img src="https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/Vue-3-4FC08D?logo=vuedotjs&logoColor=white" alt="Vue" />
</p>

<p align="center">A lightweight, minimal, and customizable mind map</p>

![Mind map](./assets/screenshots/mind-map.png)

## Features

- **Lightweight** — Tauri 2 on the system WebView: 10 MB on macOS, ~70 MB RAM idle.
- **Local-first** — documents are plain `.xmind` files on disk. Nothing is uploaded unless you turn on optional GitHub sync and explicitly push a map.
- **Real folders** — organise documents in folders under `~/Documents/Mind`, so the file manager and the app always agree.
- **Two views of one document** — a classic mind map, or a Mermaid `mindmap` source with live preview.
- **Full editor** — 14 layouts, 50+ themes, dark mode, rich text, notes, tags, images, formulas and an outline view.
- **AI assisted** — generate a diagram from a prompt through your own OpenAI-compatible endpoint.

![Mermaid mode](./assets/screenshots/mermaid.png)

## Install

Prebuilt builds are on [Releases](https://github.com/phaethix/mind-map/releases).

| System                           | File                            |
| -------------------------------- | ------------------------------- |
| Windows 10 / 11 (x64)            | `Mind_<version>_x64-setup.exe`  |
| Ubuntu 22.04+ / Debian 12+ (x64) | `Mind_<version>_amd64.deb`      |
| Any distro with WebKitGTK 4.1    | `Mind_<version>_amd64.AppImage` |

The Windows build is unsigned, so the first launch needs **More info → Run anyway**. Linux builds use the system WebKitGTK 4.1 (Ubuntu 20.04 will not start), and the AppImage also needs `libfuse2`.

**macOS** — no prebuilt binary, build it locally:

```bash
make
```

Installed builds update themselves in place; builds from before 0.1.3 need one manual reinstall first. The `.deb` package is the exception — download a newer one from Releases.

## Build from source

| Item      | Detail                                                        |
| --------- | ------------------------------------------------------------- |
| Requires  | Xcode Command Line Tools — `xcode-select --install`           |
| Toolchain | `scripts/bootstrap.sh` installs Node, Bun and Rust if missing |
| Install   | `make`; `Mind.app` ends up in `~/Applications`                |
| Documents | `~/Documents/Mind`                                            |

```bash
make open      # launch the installed app
make desktop   # build only
make dev       # run against the local dev server
make bootstrap # toolchain only
```

## AI

AI generation reads `baseUrl`, `model` and `apiKey` from `ai.json` in the app config folder.

## Optional GitHub sync

The desktop app can push the open map to a **secret GitHub Gist**. This is off until you sign in and choose Push. A fresh install, and the browser build, stay local-only: nothing is uploaded in the background, and `~/Documents/Mind` remains the source of truth.

Sync uses GitHub's OAuth **Device Flow** (`gist` and `read:user`). The app embeds or reads a public **Client ID** only. There is no `client_secret` in the client. The access token is stored next to `ai.json` in `github.json` (mode `0600` on Unix). Sign out deletes that token.

Push writes the open map as JSON (the full document) or, if you choose it, Markdown (an outline). The file is text, not a `.xmind` zip. GitHub Gists are about **1 MB**; a larger map is refused with an error instead of being truncated. Pull asks before it replaces a local file. Gists created here are secret: they are not listed on your profile, but anyone with the link can open them.

| System  | Config folder                                         |
| ------- | ----------------------------------------------------- |
| macOS   | `~/Library/Application Support/com.huanyuli.mindmap/` |
| Linux   | `~/.local/share/com.huanyuli.mindmap/`                |
| Windows | `%APPDATA%\com.huanyuli.mindmap\`                     |

### Configure the OAuth App

These steps are for the maintainer. They are not done by the app.

1. Create an OAuth App at <https://github.com/settings/developers>.
2. Homepage URL can be this repository. The form requires an authorization callback URL; Device Flow does not use it. `http://127.0.0.1` is enough.
3. On the app's settings page, enable **Device Flow**.
4. Copy the **Client ID**. Do not generate a client secret for this flow. If GitHub shows one, do not put it in the app, the repo, or CI.
5. Provide the Client ID in one of these ways, with no production placeholder checked in:
   - Runtime or compile time: `MIND_GITHUB_CLIENT_ID=YOUR_CLIENT_ID`. `make dev` and `make desktop` both see it. A value set while compiling the desktop crate is embedded with `option_env!`; a value set when the app starts overrides it.
   - Or set `clientId` in `github.json` in the config folder above. Leave `accessToken` empty; the sign-in flow fills it in.

```bash
MIND_GITHUB_CLIENT_ID=YOUR_CLIENT_ID make dev
```

### Developer Program

To apply for the GitHub Developer Program, use <https://github.com/developer/register> with this integration (Gists API via OAuth Device Flow) and a support email you monitor. A development build is enough to apply. This repository does not publish a support address.

## Contributing

Issues and pull requests follow [CONTRIBUTING.md](CONTRIBUTING.md): use the issue forms, keep one concern per change, and run the checks listed there. Security problems should go through [SECURITY.md](SECURITY.md) rather than the public tracker.

## Credits

- Built on [SimpleMindMap](https://github.com/wanglin2/mind-map) by [wanglin2](https://github.com/wanglin2) — MIT, notice kept in [LICENSE](./LICENSE).
- From upstream: the editor, the `simple-mind-map/` engine, the themes and most icons.
- Added by this fork: the Tauri shell, dark mode, Mermaid authoring, AI generation, a local file library, and optional GitHub Gist sync.
