# Mermaid editor

## Goals

A Mermaid entry in the toolbar. It opens a split editor: Mermaid `mindmap` text on the left, a live preview on the right rendered the way a Mermaid mind map looks inside Markdown. Nothing reaches the canvas until you confirm.

AI turns a prompt into Mermaid text, and the result lands in the editor instead of changing the canvas directly.

## Editor

The window takes about 90% of the screen: white, 16px corners, the same look as the rest of the toolbar. The primary button uses the Mind Classic 7 red `#e25a40`.

- Opening it converts the current map to Mermaid, so both panes have content immediately.
- Typing re-renders after roughly 200ms. A parse failure keeps the last good frame and shows the error below the editor.
- The preview renders `mindmap` with Mermaid: radial, coloured per branch, matching what Markdown shows.
- "Apply" writes the nodes back to the canvas and keeps the colours, corner radius, round root and positions from the preview.
- "Copy" copies the editor text. "Download" saves a `.mmd`. "Open file" reads a `.mmd` into the editor.

Only Mermaid `mindmap` trees are supported. Node text survives; shape markers such as `((text))` are stripped on import. Associative lines, images and node styling do not travel through Mermaid.

## AI

A prompt field and a "Generate" button sit at the bottom of the editor. The model must return exactly one `mindmap` code block; the reply is written into the editor and the preview refreshes.

The endpoint is an OpenAI-compatible `POST {baseUrl}/chat/completions` with `baseUrl`, `model` and `apiKey`.

### Mac client

The key lives in `ai.json` in the app data folder, next to `session.json`, and never enters the repository. The page only knows whether a key is configured, plus the base URL and model name; Tauri sends the request.

The UI reports "key from the local config" and offers to reveal that file in Finder. A missing file gets a template.

### GitHub Pages

There is no server. The user opens settings in the editor and fills in the base URL, model and API key. Those three go into browser `localStorage` only, and requests go straight from the browser to the model service.

The UI reports "key kept in this browser only". When nothing is configured, "Generate" opens settings first.

Some endpoints are blocked by CORS. The failure message says the browser is not allowed to call that endpoint and that the Mac client has no such limit. No relay that would get to see the key.

## Not in scope

No changes to the existing Volcano Engine Ark chat. No other Mermaid diagram types such as flowchart. No keys in a repository `.env`.
