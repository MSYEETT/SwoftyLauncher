# Launching the EA App build

How to run the EA App build of F1 25 through the offline chain on the Linux
bench, for development and debugging. Players use the launcher's Play button;
this page is the manual path.

## The short version

```bash
# once per boot: the bench service that the game runs under (provides pv.sock)
/mnt/gaming/EAApp/ea-app.sh app &

# stage the offline chain, launch, collect logs, restore the game folder
/mnt/gaming/EAApp/ai-fix/stage-and-run.sh
```

Logs land in `/mnt/gaming/EAApp/ai-fix/run-<timestamp>/`:

- `offline.log` / `loader.log` — every check the game makes and how it was
  answered, plus the loader's binding resolution
- `game_stdout.log` — the Proton-side output

## What the chain is

The session launcher (`f1op_stub.exe`) replaces
`EAAntiCheat.GameServiceLauncher.exe` in the game folder and starts the game as
its own child. The genuine EA-signed launcher is parked beside it as
`EAAntiCheat.GameServiceLauncher.exe.bak` and is what the parent-signature
check is answered with. The hook (`f1op_hook.dll`) loads into the game before
entry and answers the game's checks; the game folder is restored afterwards.

The launcher refuses to run an extension build whose digest is not pinned; the
session launcher itself does not check digests (the manager does), so the chain
can be run directly with a development hook.

## The environment contract

Set these before invoking Proton (the service sets them for the EA app; a
direct run needs them explicitly):

```bash
export STEAM_COMPAT_DATA_PATH=/mnt/gaming/EAApp/pfx   # resolves the bench prefix
export STEAM_COMPAT_CLIENT_INSTALL_PATH=$HOME/.local/share/Steam
export STEAM_COMPAT_TOOL_PATHS=$HOME/.local/share/Steam/compatibilitytools.d/proton-cachyos-10.0-20260425-slr-x86_64:/mnt/gaming/SteamLibrary/steamapps/common/SteamLinuxRuntime_sniper
export SteamAppId=0 SteamGameId=0 STEAM_COMPAT_APP_ID=0
export WINEDLLOVERRIDES=version=n,b                    # loads the loader proxy
```

`SteamAppId=0` matters: without any of `UMU_ID` / `SteamAppId` / `SteamGameId`,
protonfixes falls back to extracting a game id from `STEAM_COMPAT_DATA_PATH`,
and this bench's digit-less path crashes it before the game starts.

## Launching

```bash
/mnt/gaming/EAApp/shim/proton run '/mnt/gaming/EAApp/Games/F1 25/EAAntiCheat.GameServiceLauncher.exe'
```

Use the shim's `run` mode for the session launcher: it waits for the game the
launcher spawns and returns when it exits. `waitforexitandrun` blocks until the
whole wine prefix is quiet, which never happens while the EA app runs in it.

The shim forwards only its `--pass-env` list into the container; anything else
needed inside (for example `PROTONFIXES_DISABLE`) requires adding it to that
list.

## Pitfalls

- The pv-socket service (`ea-app.sh app`) is long-lived and dies with the
  machine. If runs fail with `Can't connect to peer socket`, restart it.
- First boots are slow (shader compilation); a 300-second window for the
  loader's window trigger can expire before the menu appears.
- The EA account sign-in gate needs the EA app running and signed in; without
  it the game stalls in early init and the code sections stay encrypted.
