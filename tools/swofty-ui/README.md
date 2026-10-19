# Swofty UI

A local web app for viewing and editing F1 25 UI scenes (`.idf`, the `eScene` layout graph that `tools/idf.py`
decodes). It reads the game's UI ERPs in place and never writes to the game folder. User documentation:
<https://f1.swofty.net/reference/ui/swofty-ui>.

Release archives ship it as `bin/swofty-ui` (Linux) and `bin\swofty-ui.exe` (Windows), a one-file PyInstaller build
that needs no Python. From source it needs Python 3.14 or newer, or Python 3.10 to 3.13 with the `zstandard` module or the
`zstd` command.

```sh
python3 tools/swofty-ui/serve.py                     # finds the game, opens http://127.0.0.1:7341/
python3 tools/swofty-ui/serve.py --game "/path/to/F1 25" --open art/screens/career/rnd_overview.idf
python3 tools/swofty-ui/serve.py --no-game --open work/rnd_overview.idf
```

| Flag | Default |
|---|---|
| `--game DIR` | `$F1_25_DIR`, then the launcher's **Locate game** folder, then the Steam libraries. A pristine copy or a `ui_package` folder also works. |
| `--export-dir DIR` | `swofty-ui/exports` in the launcher's data folder (`%LOCALAPPDATA%\Swofty`, `~/.local/share/swofty`, or `$SWOFTY_DATA_DIR`) |
| `--port N` | `7341`, or the next free port |
| `--host ADDR` | `127.0.0.1` |
| `--open SCENE` | An eaid path or a local `.idf` to open first |
| `--no-browser` | Opens the browser unless given |
| `--no-game` | No ERP catalog: local files and uploads only |

`SWOFTY_UI_LOG=1` logs every request. `SWOFTY_STEAM_ROOT` adds a Steam folder to search, as it does for the launcher.

## Build and test

```sh
python3 tools/swofty-ui/serve.py --self-test         # synthetic scene and ERP, HTTP round trip, discovery
python3 -m pip install pyinstaller==6.22.3
python3 tools/swofty-ui/build.py                      # dist/swofty-ui/swofty-ui[.exe]
dist/swofty-ui/swofty-ui --self-test
```

CI runs both self-tests on Ubuntu 22.04 and Windows. The release workflow builds the executable on the same runners
as the launcher and copies it into `bin/`; the Linux archive also gets `share/applications/net.swofty.SwoftyUI.desktop`
and its icons.

## Files

| File | |
|---|---|
| `serve.py` | HTTP server and CLI |
| `gamedir.py` | Game folder discovery (launcher config, Steam libraries) and the launcher data folder |
| `erpsource.py` | ERP catalog: `UILayout` scenes, materials, bindings and textures by name; fragment reads |
| `scene.py` | Model building, op replay, xref expansion, export verification |
| `assets.py` | Geom resolution: mesh, buffers, `.emb` bindings, material cbuffer, surface headers |
| `selftest.py` | `--self-test` |
| `build.py` | PyInstaller build |
| `static/` | Single-page client: `app.js`, `bc7.js` (MIT, see `THIRD_PARTY.md`), `app.css`, `index.html`, icons |
| `res/` | Source logo, Windows `.ico`, Linux icons and desktop entry |

It imports `tools/idf.py`, `tools/pkg.py` and `tools/erp.py`.

## HTTP API

| Endpoint | |
|---|---|
| `GET /api/catalog` | Scene list with ERP indexes, and the export folder |
| `GET /api/open?eaid=<path>[&erp=]` | Open a scene from the ERPs. Returns `doc` and the model. |
| `GET /api/open-file?path=` / `POST /api/upload?name=` | Open a local or uploaded `.idf` |
| `POST /api/apply {doc, ops}` | Replay ops and return the model. Ops: `set` (position, scale, rotation), `param`, `clone`, `rename`, `delete`. Keys are child-index paths such as `0.1.3`. |
| `POST /api/export {doc, ops, name?}` | Write and verify an export |
| `POST /api/reveal` | Open the export folder in the file manager |
| `GET /api/xref?eaid=[&reveal=1]` | Flattened boxes and geoms of a referenced scene |
| `POST /api/visuals {items:[{res, mat, erp}]}` | Resolved mesh, material params and texture names per geom |
| `GET /api/texture?name=<.tif.image>` | Top-mip BC7 bytes, size in `X-Tex-Width`/`X-Tex-Height` |
| `GET /api/resources?erp=&prefix=` / `GET /api/resource?erp=&name=&frag=` | Raw ERP resource listing and bytes |

Requests must carry a `Host` of the loopback address the server listens on, and a `POST` from a browser must come
from the same origin.
