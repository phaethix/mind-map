# GitHub sync

The desktop app can push the open map to a **secret GitHub Gist** and pull it back. `~/Documents/Mind` stays the source of truth: nothing leaves the machine unless you sign in and press Push.

## Sign-in

- GitHub OAuth **Device Flow** with the `gist` and `read:user` scopes.
- The client only carries a public **Client ID**; there is no `client_secret`.
- The token, username and avatar are stored together in `github.json` in the config folder (`0600` on Unix). Signing out clears all three.
- Without a Client ID the dialog shows the exact file it looked at instead of silently turning sync off.

## Push

Two formats, two buttons:

| Format  | File           | Keeps                                                           | Use for                          |
| ------- | -------------- | --------------------------------------------------------------- | -------------------------------- |
| JSON    | `*.mind.json`  | The whole document: hierarchy, notes, styling, layout, theme      | Backups, moving between machines |
| Mermaid | `*.mind.md`    | Text and hierarchy only                                         | Sharing, GitHub renders it inline |

A Mermaid file looks like this:

````markdown
```mermaid
mindmap
  (("Root"))
    ["Branch with parens (2026)"]
    Plain branch
```
````

Conversion rules, verified against mermaid 10.9 for both parsing and rendering:

- The root always uses the circle shape `((...))`; every other node stays bare text.
- Text containing `(` `)` `[` `]` `{` `}` or a backtick is wrapped as `["..."]` — a bare quoted label with parentheses inside fails to parse.
- Double quotes become `&quot;`: `["a \"b\""]` fails to parse, while the entity form renders as `a "b"`.
- An empty node is written as a single space.
- Depth follows the map; rendering refuses anything deeper than 64 levels. Node count is unlimited — the size cap below is what binds first.

Gists cap a file at about **1 MB**; a larger map is refused instead of truncated. Gists created here are secret: they are not listed on your profile, but anyone with the link can open them.

## Pull

The format is decided by **file content**, not by name, so old gists keep working:

- a ` ```mermaid ` fence containing `mindmap` is parsed as Mermaid;
- otherwise a `.mind.md` is parsed as the older Markdown outline;
- a `.mind.json` is parsed as JSON.

Parsing ignores node id prefixes, `::icon(...)` icons, `%%` comments and `:::` style classes. `<br>` becomes a newline and HTML entities are decoded. An existing local file is never replaced without confirmation.

## Config folder

| System  | Path                                                  |
| ------- | ----------------------------------------------------- |
| macOS   | `~/Library/Application Support/com.huanyuli.mindmap/` |
| Linux   | `~/.local/share/com.huanyuli.mindmap/`                |
| Windows | `%APPDATA%\com.huanyuli.mindmap\`                     |

`ai.json`, `github.json` (sign-in) and `github-sync.json` (map to gist links) all live there.

## Configure the OAuth App

A one-time task for the maintainer; the app does none of it.

1. Create an OAuth App at <https://github.com/settings/developers>.
2. Homepage URL can be this repository. The form requires an authorization callback URL; Device Flow does not use it, so `http://127.0.0.1` is enough.
3. On the app's settings page, enable **Device Flow**.
4. Copy the **Client ID**. Do not generate a client secret for this flow, and if GitHub shows one, keep it out of the app, the repo and CI.
5. Hand the Client ID to the app, either way:
   - `MIND_GITHUB_CLIENT_ID=YOUR_CLIENT_ID` at runtime or compile time; both `make dev` and `make desktop` pick it up. A value present while compiling the desktop crate is embedded with `option_env!`, a value set at launch overrides it.
   - Or write `clientId` into `github.json` in the config folder, leaving `accessToken` empty for the sign-in flow to fill in.

```bash
MIND_GITHUB_CLIENT_ID=YOUR_CLIENT_ID make dev
```

## GitHub Developer Program

Apply at <https://github.com/developer/register> with this integration (Gists API via OAuth Device Flow) and a support email you actually read. A development build is enough. This repository does not publish a support address.
