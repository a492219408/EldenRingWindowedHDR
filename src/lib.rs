#![deny(unsafe_op_in_unsafe_fn)]
// Windows requires the mixed-case DllMain entry-point spelling.
#![allow(non_snake_case)]

mod config;
mod dxgi;
mod game_compat;
mod game_hdr;
mod logger;
mod sha256;
mod windows;

use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::atomic::{AtomicPtr, AtomicU8, Ordering},
    time::Instant,
};

use config::{Config, Mode};
use logger::Logger;
use windows::Module;

const DLL_PROCESS_ATTACH: u32 = 1;
const INITIALIZER_WAIT_TIMEOUT_MS: u32 = 15_000;

// 0 = not started, 1 = running, 2 = succeeded, 3 = failed.
static INIT_STATE: AtomicU8 = AtomicU8::new(0);
static DLL_MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[unsafe(no_mangle)]
/// Windows loader entry point.
///
/// # Safety
///
/// Called only by the Windows loader with standard `DllMain` arguments. Work performed while
/// holding the loader lock is limited to storing the module, disabling thread notifications and
/// creating a worker thread.
pub unsafe extern "system" fn DllMain(module: Module, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        DLL_MODULE.store(module, Ordering::Release);
        unsafe { windows::disable_thread_notifications(module) };
        start_initialization();
    }
    1
}

/// ModEngine3 initializer. Waiting here ensures the import hooks are installed before game main.
#[unsafe(no_mangle)]
pub extern "C" fn elden_ring_windowed_hdr_init() -> bool {
    start_initialization();
    wait_for_initialization()
}

/// Compatibility alias for pre-1.0 development profiles.
#[unsafe(no_mangle)]
pub extern "C" fn elden_ring_borderless_hdr_init() -> bool {
    elden_ring_windowed_hdr_init()
}

fn wait_for_initialization() -> bool {
    let started = Instant::now();
    loop {
        match INIT_STATE.load(Ordering::Acquire) {
            2 => return true,
            3 => return false,
            _ if started.elapsed().as_millis() >= u128::from(INITIALIZER_WAIT_TIMEOUT_MS) => {
                return false;
            }
            _ => windows::sleep(1),
        }
    }
}

fn start_initialization() {
    if INIT_STATE
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    if !unsafe { windows::spawn_thread(initialization_thread) } {
        INIT_STATE.store(3, Ordering::Release);
    }
}

unsafe extern "system" fn initialization_thread(_parameter: *mut c_void) -> u32 {
    let succeeded = catch_unwind(AssertUnwindSafe(initialize))
        .ok()
        .and_then(Result::ok)
        .is_some();
    INIT_STATE.store(if succeeded { 2 } else { 3 }, Ordering::Release);
    0
}

fn initialize() -> Result<(), String> {
    let module = DLL_MODULE.load(Ordering::Acquire);
    if module.is_null() {
        return Err("DLL module handle is unavailable".to_owned());
    }

    let dll_path = unsafe { windows::module_path(module) }?;
    let ini_path = dll_path.with_extension("ini");
    let log_path = dll_path.with_extension("log");
    let logger = Logger::new(&log_path)?;
    logger.line(format!(
        "EldenRingWindowedHDR {} starting from {}",
        env!("CARGO_PKG_VERSION"),
        dll_path.display()
    ));

    let exe_path = unsafe { windows::module_path(std::ptr::null_mut()) }?;
    if !is_elden_ring(&exe_path) {
        let message = format!(
            "refusing to hook unexpected executable: {}",
            exe_path.display()
        );
        logger.line(&message);
        return Err(message);
    }
    logger.line(format!("target executable: {}", exe_path.display()));
    let metadata = std::fs::metadata(&exe_path)
        .map_err(|error| format!("cannot read target executable metadata: {error}"))?;
    logger.line(format!("target executable size: {} bytes", metadata.len()));
    let executable_hash = sha256::file_hex(&exe_path)
        .map_err(|error| format!("cannot hash target executable: {error}"))?;
    logger.line(format!("target executable SHA-256: {executable_hash}"));

    let (config, created) = match Config::load_or_create(&ini_path) {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!("configuration error: {error}"));
            return Err(error);
        }
    };
    if created {
        logger.line(format!(
            "created default configuration at {}",
            ini_path.display()
        ));
    }
    logger.line(format!("configuration: mode={}", config.mode.as_str()));
    if config.retired_force_pq_requested {
        logger.line(
            "SAFETY: force_pq_if_hdr10 was retired after real-game testing showed SDR/PQ encoding mismatch; continuing in observe mode",
        );
    }

    let windowed_hdr = config.mode == Mode::WindowedHdr;
    let emulate_hdr_fullscreen_state = matches!(
        config.mode,
        Mode::EmulateHdrFullscreenState | Mode::EmulateHdrAndSetPq | Mode::WindowedHdr
    );
    let synchronize_hdr_color_space =
        matches!(config.mode, Mode::EmulateHdrAndSetPq | Mode::WindowedHdr);
    let force_unlock_hdr_menu = matches!(
        config.mode,
        Mode::UnlockHdrMenu | Mode::EmulateHdrFullscreenState | Mode::EmulateHdrAndSetPq
    );
    let behavior_changing_hdr_mode = config.mode != Mode::Observe;
    let game_targets = match unsafe {
        game_compat::resolve(&logger, metadata.len(), &executable_hash)
    } {
        Ok(targets) => Some(targets),
        Err(error) => {
            logger.line(format!(
                "COMPATIBILITY FAILURE: {error}; refusing all internal HDR hooks and behavior changes"
            ));
            logger.line(
                "SAFETY: DXGI/AGS diagnostic observation may continue, but native game HDR behavior and color-space ownership will remain unchanged",
            );
            None
        }
    };

    let installed = unsafe { dxgi::install(&logger) }?;
    let hdr_menu_gate_hooked = if let Some(targets) = game_targets.as_ref() {
        match unsafe {
            game_hdr::install_menu_gate(
                &logger,
                targets,
                force_unlock_hdr_menu,
                behavior_changing_hdr_mode,
            )
        } {
            Ok(()) => true,
            Err(error) => {
                logger.line(format!(
                    "HDR menu-gate hook was not installed; no menu override is active: {error}"
                ));
                false
            }
        }
    } else {
        false
    };
    let hdr_availability_hooked = if windowed_hdr && let Some(targets) = game_targets.as_ref() {
        match unsafe { game_hdr::install_availability_observer(&logger, targets) } {
            Ok(()) => true,
            Err(error) => {
                logger.line(format!(
                    "HDR common-availability observer was not installed; no persistence-preserving windowed override is active: {error}"
                ));
                false
            }
        }
    } else {
        false
    };
    let graphics_config_hooked = if let Some(targets) = game_targets.as_ref() {
        match unsafe { game_hdr::install_config_observer(&logger, targets) } {
            Ok(()) => true,
            Err(error) => {
                logger.line(format!(
                    "graphics-config apply observer was not installed; DXGI/AGS observation remains active: {error}"
                ));
                false
            }
        }
    } else {
        false
    };
    let hdr_backend_hooked = if let Some(targets) = game_targets.as_ref() {
        match unsafe { game_hdr::install_backend_observer(&logger, targets) } {
            Ok(()) => true,
            Err(error) => {
                logger.line(format!(
                    "HDR backend actual-state observer was not installed; no internal fullscreen-state emulation is possible: {error}"
                ));
                false
            }
        }
    } else {
        false
    };
    let hdr_backend_experiment_enabled = emulate_hdr_fullscreen_state
        && hdr_menu_gate_hooked
        && (!windowed_hdr || hdr_availability_hooked)
        && graphics_config_hooked
        && hdr_backend_hooked
        && installed.dxgi_factory_imports != 0;
    if hdr_backend_experiment_enabled {
        if windowed_hdr {
            game_hdr::enable_windowed_availability_override();
        }
        game_hdr::enable_backend_experiment(synchronize_hdr_color_space);
    }
    logger.line(format!(
        "installed {} import hook(s); waiting for DXGI factory and AMD AGS activity",
        installed.total()
    ));
    logger.line(format!(
        "hook summary: dxgi_factory_imports={}, amd_ags_imports={}, hdr_menu_gate={hdr_menu_gate_hooked}, hdr_common_availability={hdr_availability_hooked}, graphics_config_apply={graphics_config_hooked}, hdr_backend_actual_query={hdr_backend_hooked}, hdr_backend_experiment={hdr_backend_experiment_enabled}, hdr_color_space_sync={}, windowed_hdr={windowed_hdr}",
        installed.dxgi_factory_imports, installed.amd_ags_imports,
        hdr_backend_experiment_enabled && synchronize_hdr_color_space,
    ));
    if force_unlock_hdr_menu && !hdr_menu_gate_hooked {
        logger.line(
            "SAFETY: unlock_hdr_menu was requested but its structurally verified hook could not be installed; continuing without changing the menu",
        );
    }
    if emulate_hdr_fullscreen_state && !hdr_backend_experiment_enabled {
        logger.line(
            "SAFETY: an HDR backend experiment was requested, but one or more prerequisite hooks failed; continuing without internal HDR-state emulation or color-space changes",
        );
    }
    if windowed_hdr && !hdr_availability_hooked {
        logger.line(
            "SAFETY: windowed_hdr requires the structurally verified common-availability hook; persistence and HDR behavior remain native because that hook failed",
        );
    }
    if installed.dxgi_factory_imports == 0 {
        let message = "eldenring.exe imports neither CreateDXGIFactory nor CreateDXGIFactory1";
        logger.line(message);
        return Err(message.to_owned());
    }
    if installed.amd_ags_imports == 0 {
        logger.line("AMD AGS display-mode import was not hooked; this is expected only when that import is absent or renamed");
    }
    if game_targets.is_some() {
        logger.line("initialization completed successfully");
    } else {
        logger.line(
            "initialization completed in safe compatibility fallback; diagnostic DXGI/AGS hooks are active, but all internal HDR behavior remains native",
        );
    }
    Ok(())
}

fn is_elden_ring(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("eldenring.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_target_executable_name_case_insensitively() {
        assert!(is_elden_ring(Path::new(r"C:\Game\ELDENRING.EXE")));
        assert!(!is_elden_ring(Path::new(
            r"C:\Game\start_protected_game.exe"
        )));
    }
}
