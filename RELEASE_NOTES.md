# Elden Ring Native Windowed HDR 1.0.0

First public release.

## Highlights

- Enables the game's native HDR rendering in Borderless Windowed and Windowed modes.
- Keeps the in-game HDR switch and the game's own saved HDR state.
- Restores the previous SDR color space when HDR is disabled.
- Rejects unsupported displays, swap chains, and game executables instead of forcing PQ.
- Loads early through ModEngine3 and keeps `start_online = false`.

## Supported game build

- App Ver. 1.17 / `eldenring.exe` 2.7.0.0
- Size: `87,024,720` bytes
- SHA-256: `D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134`

The core path has been verified on the current NVIDIA test system, including ordinary windowed mode, HDR on/off persistence, Windows HDR unavailable fallback, and movement between two HDR-enabled displays. AMD, Intel, Windows HDR hot changes, sleep/resume, and broad Overlay/MOD combinations remain incompletely tested.
