use std::{
    ffi::c_void,
    fmt::Write as _,
    mem, ptr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize, Ordering},
    },
};

use crate::{
    dxgi,
    game_compat::{GameTargets, HDR_MENU_GATE_INVOKE_INDEX},
    logger::Logger,
    windows,
};

type GraphicsConfigApply = unsafe extern "system" fn(*mut u8, *const u8);
type HdrMenuGateInvoke = unsafe extern "system" fn(*mut c_void) -> u8;
type HdrCommonAvailability = unsafe extern "system" fn() -> u8;
type HdrBackendActualQuery = unsafe extern "system" fn(*mut u8, usize) -> u8;

const MAX_INITIAL_GATE_LOGS: usize = 8;

const HDR_COMMON_AVAILABILITY_TRAMPOLINE_LENGTH: usize = 34;
const MAX_INITIAL_AVAILABILITY_LOGS: usize = 12;

const ABSOLUTE_JUMP_LENGTH: usize = 14;
const DESTINATION_SNAPSHOT_LENGTH: usize = 0x30;
const SOURCE_SNAPSHOT_LENGTH: usize = 0x62;
const MAX_UNCHANGED_APPLY_LOGS: usize = 8;
const HDR_SOURCE_OFFSET: usize = 0x15;
const HDR_DESTINATION_OFFSET: usize = 0x1B;

const HDR_BACKEND_DISABLED_OFFSET: usize = 0x30;
const HDR_BACKEND_CAPABILITY_FLAGS_OFFSET: usize = 0x32;
const HDR_BACKEND_HDR_CAPABILITY_FLAG: u8 = 0x04;
const MAX_INITIAL_BACKEND_QUERY_LOGS: usize = 12;
const LIVE_HDR_UNKNOWN: u8 = u8::MAX;

// (destination offset, source offset), recovered from FUN_14025C780.
const GRAPHICS_CONFIG_MAPPING: &[(usize, usize)] = &[
    (0x00, 0x00),
    (0x01, 0x01),
    (0x02, 0x02),
    (0x03, 0x03),
    (0x04, 0x05),
    (0x05, 0x06),
    (0x06, 0x07),
    (0x07, 0x08),
    (0x08, 0x09),
    (0x09, 0x0A),
    (0x2E, 0x0B),
    (0x0A, 0x0C),
    (0x0B, 0x0D),
    (0x0C, 0x0E),
    (0x2F, 0x0F),
    (0x0D, 0x10),
    (0x0F, 0x11),
    (0x10, 0x19),
    (0x11, 0x12),
    (0x13, 0x1B),
    (0x14, 0x13),
    (0x15, 0x1C),
    (0x16, 0x14),
    (0x17, 0x1A),
    (0x18, 0x1D),
    (0x19, 0x1E),
    (0x1A, 0x1F),
    (0x1B, 0x15),
    (0x1C, 0x16),
    (0x1D, 0x17),
    (0x1E, 0x18),
    (0x1F, 0x21),
    (0x20, 0x04),
    (0x21, 0x20),
    (0x22, 0x60),
    (0x23, 0x61),
];

static LOGGER: OnceLock<Logger> = OnceLock::new();
static HDR_MENU_GATE_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HDR_MENU_GATE_UNLOCK: AtomicBool = AtomicBool::new(false);
static HDR_MENU_GATE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static HDR_MENU_GATE_LAST_STATE: AtomicU8 = AtomicU8::new(u8::MAX);
static HDR_COMMON_AVAILABILITY_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HDR_WINDOWED_AVAILABILITY_ENABLED: AtomicBool = AtomicBool::new(false);
static HDR_AVAILABILITY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static HDR_AVAILABILITY_LAST_OBJECT: AtomicUsize = AtomicUsize::new(0);
static HDR_AVAILABILITY_LAST_REVISION: AtomicUsize = AtomicUsize::new(0);
static HDR_AVAILABILITY_LAST_STATE: AtomicU8 = AtomicU8::new(u8::MAX);
static GRAPHICS_CONFIG_APPLY_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static APPLY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static UNCHANGED_APPLY_COUNT: AtomicUsize = AtomicUsize::new(0);
static LIVE_HDR_SETTING: AtomicU8 = AtomicU8::new(LIVE_HDR_UNKNOWN);
static HDR_BACKEND_ACTUAL_QUERY_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HDR_BACKEND_EXPERIMENT_ENABLED: AtomicBool = AtomicBool::new(false);
static HDR_BACKEND_COLOR_SPACE_SYNC_ENABLED: AtomicBool = AtomicBool::new(false);
static HDR_COLOR_SPACE_REQUEST_ERROR_LOGGED: AtomicBool = AtomicBool::new(false);
static HDR_BACKEND_QUERY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static HDR_BACKEND_LAST_OBJECT: AtomicUsize = AtomicUsize::new(0);
static HDR_BACKEND_LAST_STATE: AtomicU8 = AtomicU8::new(u8::MAX);
static HDR_CANDIDATE_CACHE: OnceLock<Mutex<HdrCandidateCache>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HdrCandidateCache {
    object: usize,
    revision: usize,
    eligible: Option<bool>,
}

/// Hooks only the gray-out predicate owned by the Sound and Display HDR row.
/// In observation mode the original result is returned unchanged. In unlock
/// mode a successful native call is still made for diagnostics, after which
/// the effective gray-out result is forced to false.
///
/// # Safety
///
/// `targets` must come from the all-or-nothing executable-section resolver.
/// Every RTTI/vtable/code pointer is checked again immediately before the
/// shared read-only vtable slot is modified.
pub unsafe fn install_menu_gate(
    logger: &Logger,
    targets: &GameTargets,
    unlock: bool,
    require_verified_original: bool,
) -> Result<(), String> {
    let _ = LOGGER.set(logger.clone());
    HDR_MENU_GATE_UNLOCK.store(unlock, Ordering::Release);

    let image_base = unsafe { windows::main_module()? }.cast::<u8>();
    let vtable = unsafe { image_base.add(targets.menu_gate_vtable_rva) }.cast::<*mut c_void>();
    let expected_locator = unsafe { image_base.add(targets.menu_gate_complete_object_locator_rva) };
    let actual_locator = unsafe { *vtable.sub(1) }.cast::<u8>();
    if actual_locator != expected_locator {
        return Err(format!(
            "HDR menu-gate RTTI mismatch at vtable RVA 0x{:08X}: expected {expected_locator:p}, found {actual_locator:p}",
            targets.menu_gate_vtable_rva
        ));
    }

    for (index, &expected_rva) in targets.menu_gate_entries.iter().enumerate() {
        if index == HDR_MENU_GATE_INVOKE_INDEX {
            continue;
        }
        let expected = unsafe { image_base.add(expected_rva) }.cast::<c_void>();
        let actual = unsafe { *vtable.add(index) };
        if actual != expected {
            return Err(format!(
                "HDR menu-gate vtable neighbor {index} mismatch: expected RVA 0x{expected_rva:08X} ({expected:p}), found {actual:p}"
            ));
        }
    }

    let expected_original =
        unsafe { image_base.add(targets.menu_gate_invoke_rva) }.cast::<c_void>();
    let expected_body = unsafe {
        std::slice::from_raw_parts(
            expected_original.cast::<u8>(),
            targets.menu_gate_invoke_bytes.len(),
        )
    };
    if expected_body != targets.menu_gate_invoke_bytes {
        if require_verified_original {
            return Err(format!(
                "HDR menu-gate invoke body at RVA 0x{:08X} changed after compatibility resolution; refusing the behavior-changing override",
                targets.menu_gate_invoke_rva
            ));
        }
        logger.line(format!(
            "HDR menu-gate invoke body at RVA 0x{:08X} changed after compatibility resolution; observe mode will chain it without changing its result",
            targets.menu_gate_invoke_rva
        ));
    }

    let slot = unsafe { vtable.add(HDR_MENU_GATE_INVOKE_INDEX) };
    let replacement = hdr_menu_gate_hook as *const () as *mut c_void;
    let current = unsafe { *slot };
    if current == replacement {
        if HDR_MENU_GATE_ORIGINAL.load(Ordering::Acquire).is_null() {
            return Err("HDR menu-gate hook is present without a saved original".to_owned());
        }
        return Ok(());
    }
    if current.is_null() || !unsafe { windows::address_is_executable(current) } {
        return Err(format!(
            "HDR menu-gate vtable entry is not executable: {current:p}"
        ));
    }
    if current != expected_original && require_verified_original {
        return Err(format!(
            "HDR menu-gate vtable entry already points outside the verified game function ({current:p}); refusing to place a behavior-changing HDR mode on top of an unknown hook"
        ));
    }

    HDR_MENU_GATE_ORIGINAL
        .compare_exchange(
            ptr::null_mut(),
            current,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map_err(|_| "HDR menu-gate original was already initialized".to_owned())?;

    if let Err(error) = unsafe { windows::write_pointer(slot, replacement) } {
        let _ = HDR_MENU_GATE_ORIGINAL.compare_exchange(
            current,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(format!("cannot patch HDR menu-gate vtable slot: {error}"));
    }
    if unsafe { *slot } != replacement {
        let restore_result = unsafe { windows::write_pointer(slot, current) };
        let _ = HDR_MENU_GATE_ORIGINAL.compare_exchange(
            current,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(match restore_result {
            Ok(()) => {
                "HDR menu-gate patch verification failed; original vtable entry restored".to_owned()
            }
            Err(error) => format!(
                "HDR menu-gate patch verification failed and the original vtable entry could not be restored: {error}"
            ),
        });
    }

    logger.line(format!(
        "HDR menu-gate hook installed at vtable RVA 0x{:08X}, slot {HDR_MENU_GATE_INVOKE_INDEX}; original={current:p}; mode={}",
        targets.menu_gate_vtable_rva,
        if unlock { "unlock_hdr_menu" } else { "observe" }
    ));
    Ok(())
}

unsafe extern "system" fn hdr_menu_gate_hook(this: *mut c_void) -> u8 {
    let original_address = HDR_MENU_GATE_ORIGINAL.load(Ordering::Acquire);
    if original_address.is_null() {
        log("HDR menu-gate hook invoked without an original; returning grayed for safety");
        return 1;
    }
    // The saved pointer came directly from the verified one-argument std::function
    // vtable slot. Windows x64 uses the same representation for code pointers.
    let original = unsafe { mem::transmute::<*mut c_void, HdrMenuGateInvoke>(original_address) };
    let original_grayed = unsafe { original(this) } != 0;
    let unlock = HDR_MENU_GATE_UNLOCK.load(Ordering::Acquire);
    let windowed_mode = HDR_WINDOWED_AVAILABILITY_ENABLED.load(Ordering::Acquire);
    let effective_grayed = original_grayed && !unlock;

    let sequence = HDR_MENU_GATE_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let state =
        u8::from(original_grayed) | (u8::from(effective_grayed) << 1) | (u8::from(unlock) << 2);
    let previous = HDR_MENU_GATE_LAST_STATE.swap(state, Ordering::Relaxed);
    if sequence <= MAX_INITIAL_GATE_LOGS || state != previous {
        log(format!(
            "HDR menu gate #{sequence}: object={this:p}, upstream_eligible={}, original_grayed={}, effective_grayed={}, mode={}",
            !original_grayed,
            original_grayed,
            effective_grayed,
            if windowed_mode {
                "windowed_hdr"
            } else if unlock {
                "unlock_hdr_menu"
            } else {
                "observe"
            }
        ));
    } else if sequence == MAX_INITIAL_GATE_LOGS + 1 {
        log(
            "HDR menu gate: repeated unchanged calls are suppressed; state transitions will still be logged",
        );
    }

    u8::from(effective_grayed)
}

/// Installs a pass-through observer for the game's common HDR availability
/// query. The result remains native until all windowed-HDR prerequisites have
/// installed and `enable_windowed_availability_override` is called.
///
/// # Safety
///
/// `targets` must come from the strict resolver. The function has a RIP-relative
/// security-cookie load in its first 14 bytes, so installation uses a dedicated
/// relocated trampoline instead of the generic copier.
pub unsafe fn install_availability_observer(
    logger: &Logger,
    targets: &GameTargets,
) -> Result<(), String> {
    let _ = LOGGER.set(logger.clone());
    HDR_WINDOWED_AVAILABILITY_ENABLED.store(false, Ordering::Release);

    let image_base = unsafe { windows::main_module()? }.cast::<u8>();
    let trampoline = unsafe { install_verified_availability_hook(image_base, targets) }?;
    logger.line(format!(
        "HDR common-availability observer installed at RVA 0x{:08X}; trampoline={trampoline:p}; passthrough=true",
        targets.common_availability_rva
    ));
    Ok(())
}

pub fn enable_windowed_availability_override() {
    HDR_WINDOWED_AVAILABILITY_ENABLED.store(true, Ordering::Release);
    log(
        "EXPERIMENT: strict windowed HDR availability override enabled for both borderless and ordinary windowed swap chains; game save files remain owned by the game",
    );
}

unsafe extern "system" fn hdr_common_availability_hook() -> u8 {
    let original_address = HDR_COMMON_AVAILABILITY_ORIGINAL.load(Ordering::Acquire);
    if original_address.is_null() {
        log("HDR common-availability observer invoked without an original trampoline");
        return 0;
    }
    // The dedicated trampoline reproduces the verified security-cookie
    // prologue and then resumes FUN_140953A10 with its original no-argument ABI.
    let original =
        unsafe { mem::transmute::<*mut c_void, HdrCommonAvailability>(original_address) };
    let native_eligible = unsafe { original() } != 0;
    let override_enabled = HDR_WINDOWED_AVAILABILITY_ENABLED.load(Ordering::Acquire);
    let active = dxgi::active_swap_chain_snapshot();
    let mut windowed_candidate = false;
    let mut candidate_checked = false;
    if override_enabled
        && !native_eligible
        && let Some(snapshot) = active
    {
        candidate_checked = true;
        windowed_candidate = unsafe {
            candidate_is_eligible(
                snapshot.swap_chain,
                snapshot.revision,
                "HDR common availability",
            )
        };
    }
    let effective_eligible = native_eligible || (override_enabled && windowed_candidate);

    let sequence = HDR_AVAILABILITY_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let object = active.map_or(0, |snapshot| snapshot.swap_chain as usize);
    let revision = active.map_or(0, |snapshot| snapshot.revision);
    let previous_object = HDR_AVAILABILITY_LAST_OBJECT.swap(object, Ordering::Relaxed);
    let previous_revision = HDR_AVAILABILITY_LAST_REVISION.swap(revision, Ordering::Relaxed);
    let state = u8::from(native_eligible)
        | (u8::from(windowed_candidate) << 1)
        | (u8::from(effective_eligible) << 2)
        | (u8::from(override_enabled) << 3)
        | (u8::from(candidate_checked) << 4)
        | (u8::from(active.is_some()) << 5);
    let previous_state = HDR_AVAILABILITY_LAST_STATE.swap(state, Ordering::Relaxed);
    if sequence <= MAX_INITIAL_AVAILABILITY_LOGS
        || object != previous_object
        || revision != previous_revision
        || state != previous_state
    {
        log(format!(
            "HDR common availability #{sequence}: native_eligible={native_eligible}, active_swap_chain={}, revision={revision}, candidate_checked={candidate_checked}, windowed_candidate={windowed_candidate}, effective_eligible={effective_eligible}, mode={}",
            active
                .map(|snapshot| format!("{:p}", snapshot.swap_chain))
                .unwrap_or_else(|| "none".to_owned()),
            if override_enabled {
                "windowed_hdr"
            } else {
                "passthrough"
            }
        ));
    } else if sequence == MAX_INITIAL_AVAILABILITY_LOGS + 1 {
        log(
            "HDR common availability: repeated unchanged calls are suppressed; state, swap-chain, and revision transitions will still be logged",
        );
    }

    u8::from(effective_eligible)
}

/// Installs a read-only diagnostic inline hook around the routine that copies
/// the menu's graphics settings into the game's live graphics configuration.
///
/// # Safety
///
/// `targets` must come from the strict resolver. Its captured prologue is
/// checked byte-for-byte before it is replaced.
/// Installation occurs during the early ModEngine3 initializer, before the
/// target routine is expected to execute.
pub unsafe fn install_config_observer(
    logger: &Logger,
    targets: &GameTargets,
) -> Result<(), String> {
    let _ = LOGGER.set(logger.clone());

    let image_base = unsafe { windows::main_module()? }.cast::<u8>();
    let trampoline = unsafe {
        install_verified_inline_hook(
            image_base,
            targets.graphics_config_apply_rva,
            &targets.graphics_config_apply_prologue,
            graphics_config_apply_hook as *const () as usize,
            &GRAPHICS_CONFIG_APPLY_ORIGINAL,
            "graphics-config apply",
        )
    }?;

    logger.line(format!(
        "graphics-config apply observer installed at RVA 0x{:08X}; trampoline={trampoline:p}; verified HDR mapping=source+0x{HDR_SOURCE_OFFSET:02X} -> destination+0x{HDR_DESTINATION_OFFSET:02X}",
        targets.graphics_config_apply_rva
    ));
    Ok(())
}

/// Installs a transparent observer for the function that converts DXGI's
/// exclusive-fullscreen state into the renderer's actual HDR state.
///
/// # Safety
///
/// `targets` must come from the strict resolver. The hook remains a pass-through
/// until `enable_backend_experiment` is explicitly called after all prerequisite
/// hooks have installed successfully.
pub unsafe fn install_backend_observer(
    logger: &Logger,
    targets: &GameTargets,
) -> Result<(), String> {
    let _ = LOGGER.set(logger.clone());
    HDR_BACKEND_EXPERIMENT_ENABLED.store(false, Ordering::Release);
    HDR_BACKEND_COLOR_SPACE_SYNC_ENABLED.store(false, Ordering::Release);

    let image_base = unsafe { windows::main_module()? }.cast::<u8>();
    let trampoline = unsafe {
        install_verified_inline_hook(
            image_base,
            targets.backend_actual_query_rva,
            &targets.backend_actual_query_prologue,
            hdr_backend_actual_query_hook as *const () as usize,
            &HDR_BACKEND_ACTUAL_QUERY_ORIGINAL,
            "HDR backend actual-state query",
        )
    }?;
    logger.line(format!(
        "HDR backend actual-state observer installed at RVA 0x{:08X}; trampoline={trampoline:p}; passthrough=true",
        targets.backend_actual_query_rva
    ));
    Ok(())
}

pub fn enable_backend_experiment(synchronize_color_space: bool) {
    HDR_BACKEND_COLOR_SPACE_SYNC_ENABLED.store(synchronize_color_space, Ordering::Release);
    HDR_BACKEND_EXPERIMENT_ENABLED.store(true, Ordering::Release);
    if synchronize_color_space {
        log(
            "EXPERIMENT: internal HDR fullscreen-state emulation plus Present-synchronized PQ color-space submission enabled; the previous color space will be restored when HDR stops",
        );
    } else {
        log(
            "EXPERIMENT: internal HDR fullscreen-state emulation enabled; DXGI fullscreen state and color space remain unchanged",
        );
    }
}

unsafe extern "system" fn graphics_config_apply_hook(destination: *mut u8, source: *const u8) {
    let original_address = GRAPHICS_CONFIG_APPLY_ORIGINAL.load(Ordering::Acquire);
    if original_address.is_null() {
        log("graphics-config apply observer invoked without an original trampoline");
        return;
    }
    // Windows x64 uses the same pointer representation for this executable
    // address and the ABI-compatible function pointer declared above.
    let original = unsafe { mem::transmute::<*mut c_void, GraphicsConfigApply>(original_address) };

    if destination.is_null() || source.is_null() {
        log(format!(
            "graphics-config apply observer received invalid pointers: destination={destination:p}, source={source:p}; forwarding unchanged"
        ));
        unsafe { original(destination, source) };
        return;
    }

    // The original routine reads at least source+0x61 and writes through
    // destination+0x2F. A call reaching this hook therefore already carries
    // ranges that must be valid for these snapshots.
    let mut before = [0u8; DESTINATION_SNAPSHOT_LENGTH];
    let mut source_snapshot = [0u8; SOURCE_SNAPSHOT_LENGTH];
    unsafe {
        ptr::copy_nonoverlapping(destination, before.as_mut_ptr(), before.len());
        ptr::copy_nonoverlapping(source, source_snapshot.as_mut_ptr(), source_snapshot.len());
        original(destination, source);
    }
    let mut after = [0u8; DESTINATION_SNAPSHOT_LENGTH];
    unsafe { ptr::copy_nonoverlapping(destination, after.as_mut_ptr(), after.len()) };
    LIVE_HDR_SETTING.store(after[HDR_DESTINATION_OFFSET], Ordering::Release);

    let sequence = APPLY_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let message = format_apply(sequence, &before, &after, &source_snapshot);
    if before != after {
        log(message);
        return;
    }

    let unchanged_index = UNCHANGED_APPLY_COUNT.fetch_add(1, Ordering::Relaxed);
    if unchanged_index < MAX_UNCHANGED_APPLY_LOGS {
        log(message);
    } else if unchanged_index == MAX_UNCHANGED_APPLY_LOGS {
        log(
            "graphics-config apply observer: further no-change calls are suppressed; changed calls will still be logged",
        );
    }
}

unsafe extern "system" fn hdr_backend_actual_query_hook(
    backend: *mut u8,
    output_identity: usize,
) -> u8 {
    let original_address = HDR_BACKEND_ACTUAL_QUERY_ORIGINAL.load(Ordering::Acquire);
    if original_address.is_null() {
        log("HDR backend actual-state observer invoked without an original trampoline");
        return 0;
    }
    if backend.is_null() {
        log("HDR backend actual-state observer received a null backend pointer");
        return 0;
    }
    // The saved pointer is a trampoline built from the verified 14-byte
    // prologue of FUN_141E9F4D0 and uses this exact Windows x64 ABI.
    let original =
        unsafe { mem::transmute::<*mut c_void, HdrBackendActualQuery>(original_address) };
    let native_actual = unsafe { original(backend, output_identity) } != 0;

    // The original routine has already safely dereferenced this verified
    // backend object. Static analysis establishes +0x30 as the inverse of the
    // requested HDR state, +0x32 bit 2 as the HDR-capable swap-chain flag, and
    // the first pointer-sized field as IDXGISwapChain.
    let disabled = unsafe { *backend.add(HDR_BACKEND_DISABLED_OFFSET) };
    let capability_flags = unsafe { *backend.add(HDR_BACKEND_CAPABILITY_FLAGS_OFFSET) };
    let swap_chain = unsafe { *backend.cast::<*mut c_void>() };
    dxgi::note_game_backend_swap_chain(swap_chain);
    let backend_requested_hdr = disabled == 0;
    let live_hdr = LIVE_HDR_SETTING.load(Ordering::Acquire);
    let experiment = HDR_BACKEND_EXPERIMENT_ENABLED.load(Ordering::Acquire);
    let synchronize_color_space = HDR_BACKEND_COLOR_SPACE_SYNC_ENABLED.load(Ordering::Acquire);
    let windowed_mode = HDR_WINDOWED_AVAILABILITY_ENABLED.load(Ordering::Acquire);
    let capable = capability_flags & HDR_BACKEND_HDR_CAPABILITY_FLAG != 0;
    // Before the settings-copy observer runs, the backend's verified inverse
    // request bit is the only runtime source for a persisted HDR=true value.
    // Unknown is therefore allowed, while any observed value other than 1 is
    // rejected. This lets the game restore its own saved setting without a MOD
    // mirror file and retains the stricter two-source check after a menu apply.
    let live_hdr_allows_request = matches!(live_hdr, LIVE_HDR_UNKNOWN | 1);

    let mut effective_actual = native_actual;
    let mut overridden = false;
    if experiment
        && backend_requested_hdr
        && live_hdr_allows_request
        && capable
        && !native_actual
        && unsafe {
            candidate_is_eligible(
                swap_chain,
                dxgi::swap_chain_revision(swap_chain),
                "HDR backend",
            )
        }
    {
        let color_space_ready = if synchronize_color_space {
            match unsafe { dxgi::request_managed_hdr_color_space(swap_chain, true) } {
                Ok(accepted) => accepted,
                Err(error) => {
                    log_color_space_request_error_once(error);
                    false
                }
            }
        } else {
            true
        };
        if color_space_ready {
            effective_actual = true;
            overridden = true;
        }
    }
    if synchronize_color_space && !overridden {
        match unsafe { dxgi::request_managed_hdr_color_space(swap_chain, false) } {
            Ok(_) => {}
            Err(error) => log_color_space_request_error_once(error),
        }
    }

    let sequence = HDR_BACKEND_QUERY_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let object = backend as usize;
    let previous_object = HDR_BACKEND_LAST_OBJECT.swap(object, Ordering::Relaxed);
    let state = u8::from(backend_requested_hdr)
        | (u8::from(native_actual) << 1)
        | (u8::from(effective_actual) << 2)
        | (u8::from(overridden) << 3)
        | (u8::from(experiment) << 4)
        | (u8::from(capable) << 5)
        | (u8::from(live_hdr_allows_request) << 6)
        | (u8::from(synchronize_color_space) << 7);
    let previous_state = HDR_BACKEND_LAST_STATE.swap(state, Ordering::Relaxed);
    if sequence <= MAX_INITIAL_BACKEND_QUERY_LOGS
        || object != previous_object
        || state != previous_state
    {
        log(format!(
            "HDR backend actual query #{sequence}: backend={backend:p}, swap_chain={swap_chain:p}, live_config_hdr={}, backend_requested_hdr={backend_requested_hdr} (backend+0x30={disabled}), capability_flags=0x{capability_flags:02X}, native_actual={native_actual}, effective_actual={effective_actual}, override={overridden}, mode={}",
            live_hdr_text(live_hdr),
            if windowed_mode {
                "windowed_hdr"
            } else if synchronize_color_space {
                "emulate_hdr_and_set_pq"
            } else if experiment {
                "emulate_hdr_fullscreen_state"
            } else {
                "passthrough"
            }
        ));
    } else if sequence == MAX_INITIAL_BACKEND_QUERY_LOGS + 1 {
        log(
            "HDR backend actual query: repeated unchanged calls are suppressed; state transitions will still be logged",
        );
    }

    u8::from(effective_actual)
}

unsafe fn candidate_is_eligible(swap_chain: *mut c_void, revision: usize, context: &str) -> bool {
    let object = swap_chain as usize;
    let mut cache = match HDR_CANDIDATE_CACHE
        .get_or_init(|| Mutex::new(HdrCandidateCache::default()))
        .lock()
    {
        Ok(cache) => cache,
        Err(_) => {
            log(format!(
                "{context} windowed candidate check: eligible=false; candidate cache lock is poisoned"
            ));
            return false;
        }
    };
    if cache.object == object
        && cache.revision == revision
        && let Some(eligible) = cache.eligible
    {
        return eligible;
    }
    cache.object = object;
    cache.revision = revision;
    cache.eligible = None;

    // Keep the cache lock across this one read-only COM inspection. That makes
    // a one-shot settings initialization wait for an in-flight backend check
    // instead of transiently observing false and normalizing saved HDR off.
    let result = unsafe { dxgi::inspect_windowed_hdr_candidate(swap_chain) };
    match result {
        Ok(candidate) => {
            log(format!(
                "{context} windowed candidate check: eligible={}; revision={revision}; {}",
                candidate.eligible, candidate.details
            ));
            cache.eligible = Some(candidate.eligible);
            candidate.eligible
        }
        Err(error) => {
            log(format!(
                "{context} windowed candidate check: eligible=false; revision={revision}; {error}"
            ));
            cache.eligible = Some(false);
            false
        }
    }
}

fn log_color_space_request_error_once(error: String) {
    if !HDR_COLOR_SPACE_REQUEST_ERROR_LOGGED.swap(true, Ordering::AcqRel) {
        log(format!(
            "HDR color-space synchronization request failed; internal HDR emulation will remain disabled: {error}"
        ));
    }
}

fn live_hdr_text(value: u8) -> &'static str {
    match value {
        LIVE_HDR_UNKNOWN => "unknown",
        0 => "0",
        1 => "1",
        _ => "non_boolean",
    }
}

unsafe fn install_verified_availability_hook(
    image_base: *mut u8,
    targets: &GameTargets,
) -> Result<*mut u8, String> {
    let target = unsafe { image_base.add(targets.common_availability_rva) };
    let patch = absolute_jump(hdr_common_availability_hook as *const () as usize);
    let actual_prologue = unsafe {
        std::slice::from_raw_parts(
            target.cast_const(),
            targets.common_availability_prologue.len(),
        )
    };
    if actual_prologue == patch {
        let original = HDR_COMMON_AVAILABILITY_ORIGINAL.load(Ordering::Acquire);
        if original.is_null() {
            return Err(
                "HDR common-availability hook is present without a saved trampoline".to_owned(),
            );
        }
        return Ok(original.cast());
    }
    if actual_prologue != targets.common_availability_prologue {
        return Err(format!(
            "HDR common-availability prologue mismatch at RVA 0x{:08X}; refusing to overwrite a post-resolution patch",
            targets.common_availability_rva
        ));
    }

    let displacement = i32::from_le_bytes(
        targets.common_availability_prologue[7..11]
            .try_into()
            .expect("fixed displacement range"),
    );
    let resolved_cookie = (unsafe { target.add(11) } as usize)
        .checked_add_signed(displacement as isize)
        .ok_or_else(|| {
            "HDR common-availability security-cookie relocation overflowed after resolution"
                .to_owned()
        })?;
    let expected_cookie = unsafe { image_base.add(targets.security_cookie_rva) } as usize;
    if resolved_cookie != expected_cookie {
        return Err(format!(
            "HDR common-availability security-cookie relocation mismatch: expected {expected_cookie:#018X}, resolved {resolved_cookie:#018X}"
        ));
    }

    let return_address = unsafe { target.add(targets.common_availability_prologue.len()) } as usize;
    let trampoline_bytes = build_availability_trampoline(expected_cookie, return_address);
    let trampoline = unsafe { windows::allocate_read_write(trampoline_bytes.len()) }?;
    unsafe {
        ptr::copy_nonoverlapping(
            trampoline_bytes.as_ptr(),
            trampoline,
            trampoline_bytes.len(),
        );
        windows::protect_execute_read(trampoline, trampoline_bytes.len())?;
    }

    HDR_COMMON_AVAILABILITY_ORIGINAL
        .compare_exchange(
            ptr::null_mut(),
            trampoline.cast(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map_err(|_| "HDR common-availability trampoline was already initialized".to_owned())?;

    if let Err(error) = unsafe { windows::write_memory(target, &patch) } {
        let _ = HDR_COMMON_AVAILABILITY_ORIGINAL.compare_exchange(
            trampoline.cast(),
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(format!(
            "cannot patch HDR common-availability routine: {error}"
        ));
    }
    let installed_bytes = unsafe { std::slice::from_raw_parts(target, patch.len()) };
    if installed_bytes != patch {
        let restore_result =
            unsafe { windows::write_memory(target, &targets.common_availability_prologue) };
        let _ = HDR_COMMON_AVAILABILITY_ORIGINAL.compare_exchange(
            trampoline.cast(),
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(match restore_result {
            Ok(()) => {
                "HDR common-availability patch verification failed; original prologue restored"
                    .to_owned()
            }
            Err(error) => format!(
                "HDR common-availability patch verification failed and the original prologue could not be restored: {error}"
            ),
        });
    }

    Ok(trampoline)
}

fn build_availability_trampoline(
    security_cookie_address: usize,
    return_address: usize,
) -> [u8; HDR_COMMON_AVAILABILITY_TRAMPOLINE_LENGTH] {
    let mut trampoline = [0u8; HDR_COMMON_AVAILABILITY_TRAMPOLINE_LENGTH];
    trampoline[0..4].copy_from_slice(&[0x48, 0x83, 0xEC, 0x48]);
    trampoline[4..6].copy_from_slice(&[0x48, 0xB8]);
    trampoline[6..14].copy_from_slice(&(security_cookie_address as u64).to_le_bytes());
    trampoline[14..17].copy_from_slice(&[0x48, 0x8B, 0x00]);
    trampoline[17..20].copy_from_slice(&[0x48, 0x33, 0xC4]);
    trampoline[20..34].copy_from_slice(&absolute_jump(return_address));
    trampoline
}

unsafe fn install_verified_inline_hook(
    image_base: *mut u8,
    target_rva: usize,
    expected_prologue: &[u8; ABSOLUTE_JUMP_LENGTH],
    replacement: usize,
    original_storage: &AtomicPtr<c_void>,
    label: &str,
) -> Result<*mut u8, String> {
    let target = unsafe { image_base.add(target_rva) };
    let patch = absolute_jump(replacement);
    let actual_prologue =
        unsafe { std::slice::from_raw_parts(target.cast_const(), expected_prologue.len()) };
    if actual_prologue == patch {
        let original = original_storage.load(Ordering::Acquire);
        if original.is_null() {
            return Err(format!(
                "{label} hook is present without a saved trampoline"
            ));
        }
        return Ok(original.cast());
    }
    if actual_prologue != expected_prologue {
        return Err(format!(
            "{label} prologue mismatch at RVA 0x{target_rva:08X}; refusing to overwrite another patch or unsupported build"
        ));
    }

    let trampoline_size = expected_prologue.len() + ABSOLUTE_JUMP_LENGTH;
    let trampoline = unsafe { windows::allocate_read_write(trampoline_size) }?;
    let return_address = unsafe { target.add(expected_prologue.len()) } as usize;
    let return_jump = absolute_jump(return_address);
    // The private allocation remains unreachable until the trampoline pointer
    // is atomically published after all bytes and page protection are ready.
    unsafe {
        ptr::copy_nonoverlapping(
            expected_prologue.as_ptr(),
            trampoline,
            expected_prologue.len(),
        );
        ptr::copy_nonoverlapping(
            return_jump.as_ptr(),
            trampoline.add(expected_prologue.len()),
            return_jump.len(),
        );
        windows::protect_execute_read(trampoline, trampoline_size)?;
    }

    original_storage
        .compare_exchange(
            ptr::null_mut(),
            trampoline.cast(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map_err(|_| format!("{label} trampoline was already initialized"))?;

    if let Err(error) = unsafe { windows::write_memory(target, &patch) } {
        let _ = original_storage.compare_exchange(
            trampoline.cast(),
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(format!("cannot patch {label} routine: {error}"));
    }
    let installed_bytes = unsafe { std::slice::from_raw_parts(target, patch.len()) };
    if installed_bytes != patch {
        let restore_result = unsafe { windows::write_memory(target, expected_prologue) };
        let _ = original_storage.compare_exchange(
            trampoline.cast(),
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        return Err(match restore_result {
            Ok(()) => format!("{label} patch verification failed; original prologue restored"),
            Err(error) => format!(
                "{label} patch verification failed and the original prologue could not be restored: {error}"
            ),
        });
    }

    Ok(trampoline)
}

fn absolute_jump(destination: usize) -> [u8; ABSOLUTE_JUMP_LENGTH] {
    let mut jump = [0u8; ABSOLUTE_JUMP_LENGTH];
    // jmp qword ptr [rip+0], followed by the absolute 64-bit destination.
    jump[..6].copy_from_slice(&[0xFF, 0x25, 0, 0, 0, 0]);
    jump[6..].copy_from_slice(&(destination as u64).to_le_bytes());
    jump
}

fn format_apply(
    sequence: usize,
    before: &[u8; DESTINATION_SNAPSHOT_LENGTH],
    after: &[u8; DESTINATION_SNAPSHOT_LENGTH],
    source: &[u8; SOURCE_SNAPSHOT_LENGTH],
) -> String {
    let mut message = format!(
        "graphics-config apply #{sequence}: HDR source+0x{HDR_SOURCE_OFFSET:02X}={}, destination+0x{HDR_DESTINATION_OFFSET:02X}={}->{}; changed=",
        source[HDR_SOURCE_OFFSET], before[HDR_DESTINATION_OFFSET], after[HDR_DESTINATION_OFFSET]
    );
    let mut changed = 0usize;
    for &(destination_offset, source_offset) in GRAPHICS_CONFIG_MAPPING {
        if before[destination_offset] == after[destination_offset] {
            continue;
        }
        if changed != 0 {
            message.push_str(", ");
        }
        let _ = write!(
            message,
            "dst+0x{destination_offset:02X} {}->{} (src+0x{source_offset:02X}={})",
            before[destination_offset], after[destination_offset], source[source_offset]
        );
        changed += 1;
    }
    if changed == 0 {
        message.push_str("none");
    }
    message
}

fn log(message: impl AsRef<str>) {
    if let Some(logger) = LOGGER.get() {
        logger.line(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_jump_uses_rip_indirection_and_embeds_destination() {
        let jump = absolute_jump(0x1122_3344_5566_7788);
        assert_eq!(&jump[..6], &[0xFF, 0x25, 0, 0, 0, 0]);
        assert_eq!(
            u64::from_le_bytes(jump[6..].try_into().unwrap()),
            0x1122_3344_5566_7788
        );
    }

    #[test]
    fn availability_trampoline_rewrites_the_rip_relative_cookie_load() {
        let trampoline =
            build_availability_trampoline(0x1122_3344_5566_7788, 0x8877_6655_4433_2211);
        assert_eq!(&trampoline[0..4], &[0x48, 0x83, 0xEC, 0x48]);
        assert_eq!(&trampoline[4..6], &[0x48, 0xB8]);
        assert_eq!(
            u64::from_le_bytes(trampoline[6..14].try_into().unwrap()),
            0x1122_3344_5566_7788
        );
        assert_eq!(&trampoline[14..20], &[0x48, 0x8B, 0x00, 0x48, 0x33, 0xC4]);
        assert_eq!(&trampoline[20..26], &[0xFF, 0x25, 0, 0, 0, 0]);
        assert_eq!(
            u64::from_le_bytes(trampoline[26..34].try_into().unwrap()),
            0x8877_6655_4433_2211
        );
    }

    #[test]
    fn verified_hdr_mapping_is_present_and_unique() {
        let matches = GRAPHICS_CONFIG_MAPPING
            .iter()
            .filter(|&&(destination, source)| {
                destination == HDR_DESTINATION_OFFSET && source == HDR_SOURCE_OFFSET
            })
            .count();
        assert_eq!(matches, 1);
    }

    #[test]
    fn formats_hdr_and_other_changed_fields() {
        let mut before = [0u8; DESTINATION_SNAPSHOT_LENGTH];
        let mut after = before;
        let mut source = [0u8; SOURCE_SNAPSHOT_LENGTH];
        before[HDR_DESTINATION_OFFSET] = 0;
        after[HDR_DESTINATION_OFFSET] = 1;
        source[HDR_SOURCE_OFFSET] = 1;
        after[0x20] = 2;
        source[0x04] = 2;

        let message = format_apply(7, &before, &after, &source);
        assert!(message.contains("apply #7"));
        assert!(message.contains("destination+0x1B=0->1"));
        assert!(message.contains("dst+0x1B 0->1 (src+0x15=1)"));
        assert!(message.contains("dst+0x20 0->2 (src+0x04=2)"));
    }

    #[test]
    fn formats_live_hdr_state_without_guessing_unknown_values() {
        assert_eq!(live_hdr_text(LIVE_HDR_UNKNOWN), "unknown");
        assert_eq!(live_hdr_text(0), "0");
        assert_eq!(live_hdr_text(1), "1");
        assert_eq!(live_hdr_text(2), "non_boolean");
    }
}
