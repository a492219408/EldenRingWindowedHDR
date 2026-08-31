# Elden Ring Native Windowed HDR

[简体中文](README.zh-CN.md)

This is a Rust 2024 in-process DLL for ModEngine3 that enables Elden Ring's native HDR rendering in Borderless Windowed and Windowed modes. It preserves the game's own HDR toggle and saved state, synchronizes the HDR10/PQ color-space transition with the corresponding `Present`, and restores the previous color space when HDR is disabled.

Two executable fingerprints have been statically audited and real-game tested on the current NVIDIA test system:

- App Ver. 1.16.2 / file version 2.6.2.0: `86,998,096` bytes, SHA-256
  `34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB`.
- App Ver. 1.17 / file version 2.7.0.0: `87,024,720` bytes, SHA-256
  `D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134`.

App Ver. 1.16.2 and 1.17 both passed the same runtime target resolver and windowed HDR regression in the real game. For unknown future hashes, the DLL scans only executable PE sections and requires unique signatures plus caller, RTTI/vtable, field-copy, security-cookie, and memory-boundary checks. A structurally unchanged update may therefore continue to work; any ambiguity falls back to diagnostic DXGI/AGS logging without taking ownership of HDR or PQ. It does not modify the game executable, Windows HDR settings, graphics driver settings, the registry, or save files.

## Status

The runtime resolver, core HDR/SDR path, game-owned persistence, and ordinary windowed mode have been verified on App Ver. 1.16.2 and 1.17 on the current NVIDIA test system. The final cross-version runs also passed Alt+Tab and coexistence with UnlockTheFps. Windows HDR unavailable fallback and movement between two HDR-enabled displays were additionally verified on 1.17. This does not establish universal compatibility across AMD, Intel, other displays/drivers, mixed HDR/SDR displays, Windows HDR hot changes, sleep/resume, and every DXGI Overlay or mod.

Player-facing installation instructions are maintained in [`packaging/release/README.txt`](packaging/release/README.txt) and [`packaging/release/README.zh-CN.txt`](packaging/release/README.zh-CN.txt). Detailed reverse-engineering evidence and test boundaries are in `docs/`; the cross-version resolver is documented in [`docs/version-compatibility.zh-CN.md`](docs/version-compatibility.zh-CN.md).

## Build

```powershell
mise exec -- cargo fmt --all -- --check
mise exec -- cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
mise exec -- cargo test --locked --target x86_64-pc-windows-msvc
mise exec -- cargo build --locked --release --target x86_64-pc-windows-msvc
```

If Cargo is not managed by mise, use the same Cargo commands without `mise exec --`. The formal release package is generated with:

```powershell
.\scripts\package.ps1
```

GitHub Actions pins Rust 1.98.0 so CI and release lint results remain reproducible instead of changing whenever the floating `stable` channel advances.

## License

MIT License. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
