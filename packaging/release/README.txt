Elden Ring Native Windowed HDR 1.0.0
=====================================

WHAT THIS MOD DOES
------------------

This mod enables Elden Ring's native HDR rendering in both Borderless
Windowed and Windowed display modes. It uses the game's own HDR rendering
path; it is not a ReShade preset, post-processing filter, or Auto HDR effect.

The in-game HDR switch remains usable, and its enabled/disabled state is
saved by the game. When HDR is turned off, the mod restores normal SDR output.

REQUIREMENTS
------------

- Elden Ring. App Ver. 1.16.2 / eldenring.exe 2.6.2.0 and App Ver. 1.17 /
  eldenring.exe 2.7.0.0 have been tested in the real game.
- An HDR-capable display.
- HDR enabled in Windows for the display currently showing the game.
- ModEngine3.
- Easy Anti-Cheat disabled and offline play only.

Statically audited and real-game-tested builds:

- App Ver. 1.16.2 / eldenring.exe 2.6.2.0;
  size 86,998,096 bytes;
  SHA-256 34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB.
- App Ver. 1.17 / eldenring.exe 2.7.0.0;
  size 87,024,720 bytes;
  SHA-256 D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134.

Both listed builds passed the same runtime target resolver and windowed HDR
regression on the current NVIDIA test system. If a future update leaves the
relevant code structure unchanged, the DLL can continue after strict
validation. Any ambiguity safely preserves native game behavior and is
explained in the log instead of using stale addresses.

INSTALLATION AND USE
--------------------

1. Extract the complete archive. Keep the .me3 file and the natives folder in
   their original relative locations.
2. Load EldenRingWindowedHDR.me3 with ModEngine3 and launch the game through
   ModEngine3.
3. In Windows Settings, enable HDR for the HDR-capable display.
4. In Elden Ring, select Borderless Windowed or Windowed display mode.
5. Open the game's HDR settings page and enable High Dynamic Range Rendering.

The included INI already uses the normal release setting:

    [HDR]
    mode = windowed_hdr

If Windows HDR is disabled or the active display/swap chain does not meet the
required HDR conditions, the in-game HDR option will remain unavailable. The
mod does not and cannot turn an SDR display into an HDR display.

UNINSTALLATION
--------------

Stop using EldenRingWindowedHDR.me3 in ModEngine3, or remove the extracted mod
folder. The mod does not modify eldenring.exe, Windows HDR settings, graphics
driver settings, the registry, or Elden Ring save files.

TROUBLESHOOTING
---------------

- Confirm that Windows HDR is enabled on the display showing the game.
- Check the COMPATIBILITY lines in the log. COMPATIBILITY FAILURE means the
  current game build or another mod changed critical code, so this mod will not
  enable windowed HDR.
- Keep EldenRingWindowedHDR.dll and EldenRingWindowedHDR.ini together in the
  natives folder.
- Temporarily disable other DXGI/HDR mods if they also change swap-chain color
  spaces or fullscreen behavior.
- Include natives/EldenRingWindowedHDR.log when reporting a problem. The log is
  replaced on every launch, so save it before starting the game again.

COMPATIBILITY NOTES
-------------------

The core HDR/SDR toggle, saved-state restoration, and ordinary windowed mode
have been verified on both listed game builds on the current NVIDIA test
system. The final cross-version runs also passed Alt+Tab and coexistence with
UnlockTheFps. Windows HDR unavailable fallback and movement between two
HDR-enabled displays were additionally verified on App Ver. 1.17. AMD, Intel,
other drivers/displays, mixed HDR/SDR displays, Windows HDR hot changes,
sleep/resume, and broad overlay/mod combinations are not yet fully verified.

Future builds logged as "unknown executable accepted" remain experimental
until they receive their own static review and real-game regression test.

This mod is intended only for offline ModEngine3 use. Do not use it to enter
official matchmaking with anti-cheat bypassed.

LICENSE
-------

Open source under the MIT License. See LICENSE.txt and
THIRD_PARTY_NOTICES.txt.
