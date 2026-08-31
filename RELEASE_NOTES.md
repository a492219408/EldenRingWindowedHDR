# Elden Ring Native Windowed HDR 1.0.0

First public release.

## Highlights

- Enables the game's native HDR rendering in Borderless Windowed and Windowed modes.
- Keeps the in-game HDR switch and the game's own saved HDR state.
- Restores the previous SDR color space when HDR is disabled.
- Rejects unsupported displays, swap chains, and game executables instead of forcing PQ.
- Resolves version-specific HDR targets from executable PE sections, allowing structurally unchanged game updates to work without trusting stale RVAs.
- Loads early through ModEngine3 and keeps `start_online = false`.

## Game build compatibility

- App Ver. 1.17 / `eldenring.exe` 2.7.0.0 is statically and real-game tested.
- App Ver. 1.16.2 / `eldenring.exe` 2.6.2.0 passes the same static target audit but has not yet been tested with this mod in the game.
- Unknown hashes are accepted only when all unique signatures, caller relationships, RTTI/vtable checks, security-cookie relocation, and memory boundaries pass. Otherwise the DLL keeps only diagnostic DXGI/AGS logging and leaves native HDR behavior unchanged.

The core path has been verified on the current NVIDIA test system, including ordinary windowed mode, HDR on/off persistence, Windows HDR unavailable fallback, and movement between two HDR-enabled displays. AMD, Intel, Windows HDR hot changes, sleep/resume, and broad Overlay/MOD combinations remain incompletely tested.
