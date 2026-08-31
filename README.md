# Elden Ring Native Windowed HDR

[简体中文](README.zh-CN.md)

This is a Rust 2024 in-process DLL for ModEngine3 that enables Elden Ring's native HDR rendering in Borderless Windowed and Windowed modes. It preserves the game's own HDR toggle and saved state, synchronizes the HDR10/PQ color-space transition with the corresponding `Present`, and restores the previous color space when HDR is disabled.

The release targets exactly Elden Ring App Ver. 1.17 / `eldenring.exe` 2.7.0.0:

- Size: `87,024,720` bytes
- SHA-256: `D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134`

The DLL validates this fingerprint before installing version-specific hooks. It does not modify the game executable, Windows HDR settings, graphics driver settings, the registry, or save files.

## Status

The core HDR/SDR path, game-owned persistence, ordinary windowed mode, Windows HDR unavailable fallback, and movement between two HDR-enabled displays have been verified on the current NVIDIA test system. This does not yet establish universal compatibility across AMD, Intel, other displays/drivers, Windows HDR hot changes, sleep/resume, and every DXGI Overlay or mod.

Player-facing installation instructions are maintained in [`packaging/release/README.txt`](packaging/release/README.txt) and [`packaging/release/README.zh-CN.txt`](packaging/release/README.zh-CN.txt). Detailed reverse-engineering evidence and test boundaries are in `docs/`.

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

## License

MIT License. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
