use std::{
    collections::HashMap,
    ffi::c_void,
    mem, ptr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
    },
};

use crate::{logger::Logger, windows};

type HResult = i32;
type CreateFactory = unsafe extern "system" fn(*const Guid, *mut *mut c_void) -> HResult;
type QueryInterface =
    unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HResult;
type CreateSwapChain =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_void, *mut *mut c_void) -> HResult;
type CreateSwapChainForHwnd = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *const c_void,
    *const c_void,
    *mut c_void,
    *mut *mut c_void,
) -> HResult;
type CreateSwapChainForCoreWindow = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *const c_void,
    *mut c_void,
    *mut *mut c_void,
) -> HResult;
type CreateSwapChainForComposition = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *const c_void,
    *mut c_void,
    *mut *mut c_void,
) -> HResult;
type Present = unsafe extern "system" fn(*mut c_void, u32, u32) -> HResult;
type Present1 = unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void) -> HResult;
type GetFullscreenState =
    unsafe extern "system" fn(*mut c_void, *mut i32, *mut *mut c_void) -> HResult;
type SetFullscreenState = unsafe extern "system" fn(*mut c_void, i32, *mut c_void) -> HResult;
type GetSwapChainDesc = unsafe extern "system" fn(*mut c_void, *mut SwapChainDesc) -> HResult;
type GetSwapChainDesc1 = unsafe extern "system" fn(*mut c_void, *mut SwapChainDesc1) -> HResult;
type ResizeBuffers = unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32, u32) -> HResult;
type ResizeTarget = unsafe extern "system" fn(*mut c_void, *const ModeDesc) -> HResult;
type GetContainingOutput = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HResult;
type CheckColorSpaceSupport = unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> HResult;
type SetColorSpace1 = unsafe extern "system" fn(*mut c_void, u32) -> HResult;
type ResizeBuffers1 = unsafe extern "system" fn(
    *mut c_void,
    u32,
    u32,
    u32,
    u32,
    u32,
    *const u32,
    *const *mut c_void,
) -> HResult;
type SetHdrMetadata = unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void) -> HResult;
type GetOutputDesc1 = unsafe extern "system" fn(*mut c_void, *mut OutputDesc1) -> HResult;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type AgsSetDisplayMode =
    unsafe extern "system" fn(*mut c_void, i32, i32, *const AgsDisplaySettings) -> i32;

const S_OK_MINIMUM: HResult = 0;
const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
const IMAGE_ORDINAL_FLAG64: u64 = 1 << 63;

const FACTORY_QUERY_INTERFACE_INDEX: usize = 0;
const FACTORY_CREATE_SWAP_CHAIN_INDEX: usize = 10;
const FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX: usize = 15;
const FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX: usize = 16;
const FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX: usize = 24;

const UNKNOWN_RELEASE_INDEX: usize = 2;
const SWAP_CHAIN_QUERY_INTERFACE_INDEX: usize = 0;
const SWAP_CHAIN_PRESENT_INDEX: usize = 8;
const SWAP_CHAIN_SET_FULLSCREEN_STATE_INDEX: usize = 10;
const SWAP_CHAIN_GET_FULLSCREEN_STATE_INDEX: usize = 11;
const SWAP_CHAIN_GET_DESC_INDEX: usize = 12;
const SWAP_CHAIN_RESIZE_BUFFERS_INDEX: usize = 13;
const SWAP_CHAIN_RESIZE_TARGET_INDEX: usize = 14;
const SWAP_CHAIN_GET_CONTAINING_OUTPUT_INDEX: usize = 15;
const SWAP_CHAIN_GET_DESC1_INDEX: usize = 18;
const SWAP_CHAIN_PRESENT1_INDEX: usize = 22;
const SWAP_CHAIN_CHECK_COLOR_SPACE_SUPPORT_INDEX: usize = 37;
const SWAP_CHAIN_SET_COLOR_SPACE1_INDEX: usize = 38;
const SWAP_CHAIN_RESIZE_BUFFERS1_INDEX: usize = 39;
const SWAP_CHAIN_SET_HDR_METADATA_INDEX: usize = 40;

const FACTORY_VTABLE_LEN: usize = 12;
const FACTORY1_VTABLE_LEN: usize = 14;
const FACTORY2_VTABLE_LEN: usize = 25;
const FACTORY3_VTABLE_LEN: usize = 26;
const FACTORY4_VTABLE_LEN: usize = 28;
const FACTORY5_VTABLE_LEN: usize = 29;
const FACTORY6_VTABLE_LEN: usize = 30;
const FACTORY7_VTABLE_LEN: usize = 32;

const SWAP_CHAIN_VTABLE_LEN: usize = 18;
const SWAP_CHAIN1_VTABLE_LEN: usize = 29;
const SWAP_CHAIN2_VTABLE_LEN: usize = 36;
const SWAP_CHAIN3_VTABLE_LEN: usize = 40;
const SWAP_CHAIN4_VTABLE_LEN: usize = 41;

const OUTPUT_GET_DESC1_INDEX: usize = 27;
const MAX_VTABLE_CLONE_REGION_SIZE: usize = 1024 * 1024;

const DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709: u32 = 0;
const DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709: u32 = 1;
const DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020: u32 = 12;
const DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT: u32 = 1;

static LOGGER: OnceLock<Logger> = OnceLock::new();
static FACTORY_SEEN: AtomicBool = AtomicBool::new(false);
static SWAP_CHAIN_SEEN: AtomicBool = AtomicBool::new(false);
static CREATE_FACTORY_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static CREATE_FACTORY1_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static AGS_SET_DISPLAY_MODE_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static SHADOW_INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static SHADOW_RECORDS: AtomicPtr<ShadowRecord> = AtomicPtr::new(ptr::null_mut());
static SWAP_CHAIN_STATES: OnceLock<Mutex<HashMap<usize, SwapChainState>>> = OnceLock::new();
static ACTIVE_SWAP_CHAIN: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ACTIVE_SWAP_CHAIN_REVISION: AtomicUsize = AtomicUsize::new(1);

#[cfg(test)]
mod hdr_tests {
    use super::*;

    #[test]
    fn hdr10_metadata_layout_matches_dxgi() {
        assert_eq!(mem::size_of::<HdrMetadataHdr10>(), 28);
    }

    #[test]
    fn ags_display_settings_layout_matches_ags_505() {
        assert_eq!(mem::size_of::<AgsDisplaySettings>(), 0x68);
    }

    #[test]
    fn names_critical_hdr_values() {
        assert_eq!(format_name(24), "R10G10B10A2_UNORM");
        assert!(color_space_name(DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020).contains("PQ"));
        assert_eq!(ags_mode_name(2), "PQ");
    }

    #[test]
    fn guarded_windowed_candidate_requires_every_hdr_prerequisite() {
        let mut description = SwapChainDesc {
            buffer_desc: ModeDesc {
                format: 24,
                ..ModeDesc::default()
            },
            buffer_count: 3,
            windowed: 1,
            swap_effect: 4,
            ..SwapChainDesc::default()
        };
        let output = OutputDesc1 {
            attached_to_desktop: 1,
            bits_per_color: 10,
            color_space: DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
            ..OutputDesc1::default()
        };

        assert!(windowed_hdr_candidate_is_eligible(
            &description,
            false,
            &output,
            DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT,
        ));
        assert!(!windowed_hdr_candidate_is_eligible(
            &description,
            true,
            &output,
            DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT,
        ));
        assert!(!windowed_hdr_candidate_is_eligible(
            &description,
            false,
            &output,
            0,
        ));
        description.buffer_desc.format = 28;
        assert!(!windowed_hdr_candidate_is_eligible(
            &description,
            false,
            &output,
            DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT,
        ));
    }

    #[test]
    fn managed_color_space_transitions_from_baseline_to_pq_and_back() {
        let mut state = ManagedColorSpaceState::default();

        assert!(state.request(true, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709));
        let enable = state.begin_transition().unwrap();
        assert!(enable.enabling_hdr);
        assert_eq!(
            enable.target_color_space,
            DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
        );
        state.finish_transition(enable, true);
        assert_eq!(
            state.restore_color_space,
            Some(DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709)
        );

        assert!(state.request(false, DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020));
        let restore = state.begin_transition().unwrap();
        assert!(!restore.enabling_hdr);
        assert_eq!(
            restore.target_color_space,
            DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709
        );
        state.finish_transition(restore, true);
        assert_eq!(state.restore_color_space, None);
        assert!(!state.failure_latched);
    }

    #[test]
    fn managed_color_space_failure_is_latched_and_restores_the_baseline() {
        let mut state = ManagedColorSpaceState::default();

        assert!(state.request(true, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709));
        let enable = state.begin_transition().unwrap();
        state.finish_transition(enable, false);
        assert!(state.failure_latched);
        assert!(!state.request(true, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709));

        let restore = state.begin_transition().unwrap();
        assert!(!restore.enabling_hdr);
        state.finish_transition(restore, true);
        assert_eq!(state.restore_color_space, None);
        assert!(state.failure_latched);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstallationReport {
    pub dxgi_factory_imports: usize,
    pub amd_ags_imports: usize,
}

impl InstallationReport {
    pub const fn total(self) -> usize {
        self.dxgi_factory_imports + self.amd_ags_imports
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SwapChainState {
    first_present_logged: bool,
    generation: u64,
    last_signature: Option<SwapChainSignature>,
    observed_color_space: u32,
    managed_color_space: ManagedColorSpaceState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ManagedColorSpaceState {
    restore_color_space: Option<u32>,
    desired_hdr: bool,
    transition_pending: bool,
    transition_in_progress: bool,
    failure_latched: bool,
    request_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ManagedColorSpaceTransition {
    target_color_space: u32,
    enabling_hdr: bool,
    request_epoch: u64,
}

impl ManagedColorSpaceState {
    fn request(&mut self, hdr: bool, current_color_space: u32) -> bool {
        if hdr && self.failure_latched {
            return false;
        }

        if hdr {
            if self.restore_color_space.is_none() {
                self.restore_color_space = Some(current_color_space);
                self.desired_hdr = true;
                self.transition_pending = true;
                self.request_epoch = self.request_epoch.wrapping_add(1);
            } else if !self.desired_hdr {
                self.desired_hdr = true;
                self.transition_pending = true;
                self.request_epoch = self.request_epoch.wrapping_add(1);
            }
        } else if self.restore_color_space.is_some() && self.desired_hdr {
            self.desired_hdr = false;
            self.transition_pending = true;
            self.request_epoch = self.request_epoch.wrapping_add(1);
        }

        true
    }

    fn begin_transition(&mut self) -> Option<ManagedColorSpaceTransition> {
        if !self.transition_pending || self.transition_in_progress {
            return None;
        }
        let restore_color_space = self.restore_color_space?;
        self.transition_pending = false;
        self.transition_in_progress = true;
        Some(ManagedColorSpaceTransition {
            target_color_space: if self.desired_hdr {
                DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
            } else {
                restore_color_space
            },
            enabling_hdr: self.desired_hdr,
            request_epoch: self.request_epoch,
        })
    }

    fn finish_transition(&mut self, transition: ManagedColorSpaceTransition, succeeded: bool) {
        self.transition_in_progress = false;
        if succeeded {
            if transition.request_epoch == self.request_epoch && !transition.enabling_hdr {
                self.restore_color_space = None;
            }
            return;
        }

        self.failure_latched = true;
        if transition.request_epoch == self.request_epoch && transition.enabling_hdr {
            // A failed PQ submission must not leave the renderer in the
            // emulated HDR path. Schedule restoration of the captured baseline
            // before the next Present and reject further enable attempts for
            // the lifetime of this swap chain.
            self.desired_hdr = false;
            self.transition_pending = true;
            self.request_epoch = self.request_epoch.wrapping_add(1);
        }
    }

    fn reapply_after_reconfiguration(&mut self) {
        if self.restore_color_space.is_some() && !self.failure_latched {
            self.transition_pending = true;
        }
    }

    fn observe_external_set(&mut self, requested_color_space: u32) -> bool {
        let Some(restore_color_space) = self.restore_color_space else {
            return false;
        };
        let expected = if self.desired_hdr {
            DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
        } else {
            restore_color_space
        };
        if requested_color_space == expected {
            self.transition_pending = false;
            if !self.desired_hdr {
                self.restore_color_space = None;
            }
            return false;
        }

        // Another component has taken ownership of the color-space state.
        // Relinquish this experiment rather than fighting an Overlay or MOD on
        // every frame.
        self.restore_color_space = None;
        self.desired_hdr = false;
        self.transition_pending = false;
        self.transition_in_progress = false;
        self.failure_latched = true;
        self.request_epoch = self.request_epoch.wrapping_add(1);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SwapChainSignature {
    width: u32,
    height: u32,
    format: u32,
    buffer_count: u32,
    windowed: bool,
    swap_effect: u32,
    flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShadowKind {
    Factory,
    SwapChain,
}

struct ShadowRecord {
    object: usize,
    kind: ShadowKind,
    // Records and vtables intentionally live until process exit. Release only
    // retires the address so allocator reuse cannot inherit an old hook chain.
    active: AtomicBool,
    entry_count: AtomicUsize,
    root_original_vtable: usize,
    live_root_slots: AtomicBool,
    originals: AtomicPtr<c_void>,
    generations: AtomicPtr<ShadowGeneration>,
    next: *mut ShadowRecord,
}

struct ShadowGeneration {
    vtable: usize,
    entry_count: usize,
    entry_capacity: usize,
    region_base: usize,
    region_size: usize,
    next: *mut ShadowGeneration,
}

struct VtableHook {
    index: usize,
    replacement: *mut c_void,
    name: &'static str,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Rational {
    numerator: u32,
    denominator: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ModeDesc {
    width: u32,
    height: u32,
    refresh_rate: Rational,
    format: u32,
    scanline_ordering: u32,
    scaling: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SampleDesc {
    count: u32,
    quality: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SwapChainDesc {
    buffer_desc: ModeDesc,
    sample_desc: SampleDesc,
    buffer_usage: u32,
    buffer_count: u32,
    output_window: *mut c_void,
    windowed: i32,
    swap_effect: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SwapChainDesc1 {
    width: u32,
    height: u32,
    format: u32,
    stereo: i32,
    sample_desc: SampleDesc,
    buffer_usage: u32,
    buffer_count: u32,
    scaling: u32,
    swap_effect: u32,
    alpha_mode: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FullscreenDesc {
    refresh_rate: Rational,
    scanline_ordering: u32,
    scaling: u32,
    windowed: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct OutputDesc1 {
    device_name: [u16; 32],
    desktop_coordinates: Rect,
    attached_to_desktop: i32,
    rotation: u32,
    monitor: *mut c_void,
    bits_per_color: u32,
    color_space: u32,
    red_primary: [f32; 2],
    green_primary: [f32; 2],
    blue_primary: [f32; 2],
    white_point: [f32; 2],
    min_luminance: f32,
    max_luminance: f32,
    max_full_frame_luminance: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct HdrMetadataHdr10 {
    red_primary: [u16; 2],
    green_primary: [u16; 2],
    blue_primary: [u16; 2],
    white_point: [u16; 2],
    max_mastering_luminance: u32,
    min_mastering_luminance: u32,
    max_content_light_level: u16,
    max_frame_average_light_level: u16,
}

impl Default for OutputDesc1 {
    fn default() -> Self {
        Self {
            device_name: [0; 32],
            desktop_coordinates: Rect::default(),
            attached_to_desktop: 0,
            rotation: 0,
            monitor: ptr::null_mut(),
            bits_per_color: 0,
            color_space: 0,
            red_primary: [0.0; 2],
            green_primary: [0.0; 2],
            blue_primary: [0.0; 2],
            white_point: [0.0; 2],
            min_luminance: 0.0,
            max_luminance: 0.0,
            max_full_frame_luminance: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct AgsDisplaySettings {
    mode: i32,
    alignment_padding: i32,
    chromaticity_red_x: f64,
    chromaticity_red_y: f64,
    chromaticity_green_x: f64,
    chromaticity_green_y: f64,
    chromaticity_blue_x: f64,
    chromaticity_blue_y: f64,
    chromaticity_white_point_x: f64,
    chromaticity_white_point_y: f64,
    min_luminance: f64,
    max_luminance: f64,
    max_content_light_level: f64,
    max_frame_average_light_level: f64,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const IID_IDXGI_FACTORY: Guid = guid(
    0x7b7166ec,
    0x21c7,
    0x44ae,
    [0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69],
);
const IID_IDXGI_FACTORY1: Guid = guid(
    0x770aae78,
    0xf26f,
    0x4dba,
    [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87],
);
const IID_IDXGI_FACTORY2: Guid = guid(
    0x50c83a1c,
    0xe072,
    0x4c48,
    [0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0],
);
const IID_IDXGI_FACTORY3: Guid = guid(
    0x25483823,
    0xcd46,
    0x4c7d,
    [0x86, 0xca, 0x47, 0xaa, 0x95, 0xb8, 0x37, 0xbd],
);
const IID_IDXGI_FACTORY4: Guid = guid(
    0x1bc6ea02,
    0xef36,
    0x464f,
    [0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a],
);
const IID_IDXGI_FACTORY5: Guid = guid(
    0x7632e1f5,
    0xee65,
    0x4dca,
    [0x87, 0xfd, 0x84, 0xcd, 0x75, 0xf8, 0x83, 0x8d],
);
const IID_IDXGI_FACTORY6: Guid = guid(
    0xc1b6694f,
    0xff09,
    0x44a9,
    [0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17],
);
const IID_IDXGI_FACTORY7: Guid = guid(
    0xa4966eed,
    0x76db,
    0x44da,
    [0x84, 0xc1, 0xee, 0x9a, 0x7a, 0xfb, 0x20, 0xa8],
);

const IID_IDXGI_SWAP_CHAIN: Guid = guid(
    0x310d36a0,
    0xd2e7,
    0x4c0a,
    [0xaa, 0x04, 0x6a, 0x9d, 0x23, 0xb8, 0x88, 0x6a],
);
const IID_IDXGI_SWAP_CHAIN1: Guid = guid(
    0x790a45f7,
    0x0d42,
    0x4876,
    [0x98, 0x3a, 0x0a, 0x55, 0xcf, 0xe6, 0xf4, 0xaa],
);
const IID_IDXGI_SWAP_CHAIN2: Guid = guid(
    0xa8be2ac4,
    0x199f,
    0x4946,
    [0xb3, 0x31, 0x79, 0x59, 0x9f, 0xb9, 0x8d, 0xe7],
);
const IID_IDXGI_SWAP_CHAIN3: Guid = guid(
    0x94d99bdb,
    0xf1f8,
    0x4ab0,
    [0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1],
);
const IID_IDXGI_SWAP_CHAIN4: Guid = guid(
    0x3d585d5a,
    0xbd4a,
    0x489e,
    [0xb1, 0xf4, 0x3d, 0xbc, 0xb6, 0x45, 0x2f, 0xfb],
);
const IID_IDXGI_OUTPUT6: Guid = guid(
    0x068346e8,
    0xaaec,
    0x4b84,
    [0xad, 0xd7, 0x13, 0x7f, 0x51, 0x3f, 0x77, 0xa1],
);

const fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Guid {
    Guid {
        data1,
        data2,
        data3,
        data4,
    }
}

pub unsafe fn install(logger: &Logger) -> Result<InstallationReport, String> {
    let _ = LOGGER.set(logger.clone());
    let image = unsafe { Image::main_module()? };
    let mut report = InstallationReport::default();

    for import in unsafe { image.imports("dxgi.dll")? } {
        let (replacement, original) = match import.name.as_str() {
            "CreateDXGIFactory" => (
                create_dxgi_factory_hook as *const () as *mut c_void,
                &CREATE_FACTORY_ORIGINAL,
            ),
            "CreateDXGIFactory1" => (
                create_dxgi_factory1_hook as *const () as *mut c_void,
                &CREATE_FACTORY1_ORIGINAL,
            ),
            _ => continue,
        };

        original.store(import.current, Ordering::Release);
        unsafe { windows::write_pointer(import.slot, replacement) }?;
        report.dxgi_factory_imports += 1;
    }

    for import in unsafe { image.imports("amd_ags_x64.dll")? } {
        if import.name != "agsSetDisplayMode" {
            continue;
        }
        AGS_SET_DISPLAY_MODE_ORIGINAL.store(import.current, Ordering::Release);
        unsafe {
            windows::write_pointer(
                import.slot,
                ags_set_display_mode_hook as *const () as *mut c_void,
            )
        }?;
        report.amd_ags_imports += 1;
    }

    Ok(report)
}

unsafe extern "system" fn ags_set_display_mode_hook(
    context: *mut c_void,
    device_index: i32,
    display_index: i32,
    settings: *const AgsDisplaySettings,
) -> i32 {
    let Some(function) =
        (unsafe { load_function::<AgsSetDisplayMode>(&AGS_SET_DISPLAY_MODE_ORIGINAL) })
    else {
        return -1;
    };

    if settings.is_null() {
        log(format!(
            "AGS agsSetDisplayMode(device={device_index}, display={display_index}, settings=NULL)"
        ));
    } else {
        let value = unsafe { &*settings };
        log(format!(
            "AGS agsSetDisplayMode(device={device_index}, display={display_index}, mode={} [{}], min={:.3}, max={:.3}, MaxCLL={:.3}, MaxFALL={:.3})",
            value.mode,
            ags_mode_name(value.mode),
            value.min_luminance,
            value.max_luminance,
            value.max_content_light_level,
            value.max_frame_average_light_level
        ));
    }

    let result = unsafe { function(context, device_index, display_index, settings) };
    log(format!("AGS agsSetDisplayMode returned {result}"));
    result
}

unsafe extern "system" fn create_dxgi_factory_hook(
    riid: *const Guid,
    factory: *mut *mut c_void,
) -> HResult {
    unsafe { create_factory_common(&CREATE_FACTORY_ORIGINAL, riid, factory) }
}

unsafe extern "system" fn create_dxgi_factory1_hook(
    riid: *const Guid,
    factory: *mut *mut c_void,
) -> HResult {
    unsafe { create_factory_common(&CREATE_FACTORY1_ORIGINAL, riid, factory) }
}

unsafe fn create_factory_common(
    original: &AtomicPtr<c_void>,
    riid: *const Guid,
    factory: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe { load_function::<CreateFactory>(original) }) else {
        return -1;
    };
    let result = unsafe { function(riid, factory) };
    if result >= S_OK_MINIMUM && !riid.is_null() && !factory.is_null() {
        let object = unsafe { *factory };
        if !object.is_null() {
            let interface = unsafe { &*riid };
            if let Some(entry_count) = factory_vtable_len(interface) {
                match unsafe { patch_factory(object, entry_count) } {
                    Ok(()) => log_once(
                        &FACTORY_SEEN,
                        "captured the game's DXGI factory with an object-local shadow vtable",
                    ),
                    Err(error) => log(format!("cannot hook the DXGI factory: {error}")),
                }
            } else {
                log(
                    "DXGI factory creation returned an unrecognized interface; leaving it unmodified",
                );
            }
        }
    }
    result
}

unsafe extern "system" fn factory_query_interface_hook(
    this: *mut c_void,
    riid: *const Guid,
    object: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<QueryInterface>(
            this,
            ShadowKind::Factory,
            FACTORY_QUERY_INTERFACE_INDEX,
        )
    }) else {
        return -1;
    };
    let result = unsafe { function(this, riid, object) };
    if result >= S_OK_MINIMUM && !riid.is_null() && !object.is_null() {
        let object = unsafe { *object };
        if !object.is_null() {
            let interface = unsafe { &*riid };
            if let Some(entry_count) = factory_vtable_len(interface)
                && let Err(error) = unsafe { patch_factory(object, entry_count) }
            {
                log(format!(
                    "cannot hook the queried DXGI factory interface: {error}"
                ));
            }
        }
    }
    result
}

unsafe fn patch_factory(factory: *mut c_void, entry_count: usize) -> Result<(), String> {
    let mut hooks = vec![
        VtableHook {
            index: FACTORY_QUERY_INTERFACE_INDEX,
            replacement: factory_query_interface_hook as *const () as *mut c_void,
            name: "IDXGIFactory::QueryInterface",
        },
        VtableHook {
            index: UNKNOWN_RELEASE_INDEX,
            replacement: factory_release_hook as *const () as *mut c_void,
            name: "IDXGIFactory::Release",
        },
        VtableHook {
            index: FACTORY_CREATE_SWAP_CHAIN_INDEX,
            replacement: create_swap_chain_hook as *const () as *mut c_void,
            name: "IDXGIFactory::CreateSwapChain",
        },
    ];
    if entry_count > FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX {
        hooks.extend([
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX,
                replacement: create_swap_chain_for_hwnd_hook as *const () as *mut c_void,
                name: "IDXGIFactory2::CreateSwapChainForHwnd",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX,
                replacement: create_swap_chain_for_core_window_hook as *const () as *mut c_void,
                name: "IDXGIFactory2::CreateSwapChainForCoreWindow",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX,
                replacement: create_swap_chain_for_composition_hook as *const () as *mut c_void,
                name: "IDXGIFactory2::CreateSwapChainForComposition",
            },
        ]);
    }
    unsafe { install_shadow_vtable(factory, ShadowKind::Factory, entry_count, &hooks) }
}

unsafe extern "system" fn factory_release_hook(this: *mut c_void) -> u32 {
    unsafe { release_shadow_object(this, ShadowKind::Factory) }
}

unsafe extern "system" fn create_swap_chain_hook(
    this: *mut c_void,
    device: *mut c_void,
    description: *const c_void,
    swap_chain: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<CreateSwapChain>(
            this,
            ShadowKind::Factory,
            FACTORY_CREATE_SWAP_CHAIN_INDEX,
        )
    }) else {
        return -1;
    };
    unsafe { log_legacy_creation_description("CreateSwapChain request", description) };
    let result = unsafe { function(this, device, description, swap_chain) };
    unsafe { capture_swap_chain(result, swap_chain, SWAP_CHAIN_VTABLE_LEN, "CreateSwapChain") };
    result
}

unsafe extern "system" fn create_swap_chain_for_hwnd_hook(
    this: *mut c_void,
    device: *mut c_void,
    window: *mut c_void,
    description: *const c_void,
    fullscreen_description: *const c_void,
    restrict_to_output: *mut c_void,
    swap_chain: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<CreateSwapChainForHwnd>(
            this,
            ShadowKind::Factory,
            FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX,
        )
    }) else {
        return -1;
    };
    unsafe {
        log_desc1_creation_description(
            "CreateSwapChainForHwnd request",
            description,
            fullscreen_description,
        )
    };
    let result = unsafe {
        function(
            this,
            device,
            window,
            description,
            fullscreen_description,
            restrict_to_output,
            swap_chain,
        )
    };
    unsafe {
        capture_swap_chain(
            result,
            swap_chain,
            SWAP_CHAIN1_VTABLE_LEN,
            "CreateSwapChainForHwnd",
        )
    };
    result
}

unsafe extern "system" fn create_swap_chain_for_core_window_hook(
    this: *mut c_void,
    device: *mut c_void,
    window: *mut c_void,
    description: *const c_void,
    restrict_to_output: *mut c_void,
    swap_chain: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<CreateSwapChainForCoreWindow>(
            this,
            ShadowKind::Factory,
            FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX,
        )
    }) else {
        return -1;
    };
    unsafe {
        log_desc1_creation_description(
            "CreateSwapChainForCoreWindow request",
            description,
            ptr::null(),
        )
    };
    let result = unsafe {
        function(
            this,
            device,
            window,
            description,
            restrict_to_output,
            swap_chain,
        )
    };
    unsafe {
        capture_swap_chain(
            result,
            swap_chain,
            SWAP_CHAIN1_VTABLE_LEN,
            "CreateSwapChainForCoreWindow",
        )
    };
    result
}

unsafe extern "system" fn create_swap_chain_for_composition_hook(
    this: *mut c_void,
    device: *mut c_void,
    description: *const c_void,
    restrict_to_output: *mut c_void,
    swap_chain: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<CreateSwapChainForComposition>(
            this,
            ShadowKind::Factory,
            FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX,
        )
    }) else {
        return -1;
    };
    unsafe {
        log_desc1_creation_description(
            "CreateSwapChainForComposition request",
            description,
            ptr::null(),
        )
    };
    let result = unsafe { function(this, device, description, restrict_to_output, swap_chain) };
    unsafe {
        capture_swap_chain(
            result,
            swap_chain,
            SWAP_CHAIN1_VTABLE_LEN,
            "CreateSwapChainForComposition",
        )
    };
    result
}

unsafe fn capture_swap_chain(
    result: HResult,
    output: *mut *mut c_void,
    entry_count: usize,
    source: &str,
) {
    if result < S_OK_MINIMUM || output.is_null() {
        log(format!(
            "{source} did not create a swap chain (HRESULT 0x{:08X})",
            result as u32
        ));
        return;
    }
    let swap_chain = unsafe { *output };
    if swap_chain.is_null() {
        return;
    }
    match unsafe { patch_swap_chain(swap_chain, entry_count) } {
        Ok(()) => {
            ensure_swap_chain_state(swap_chain);
            log_once(
                &SWAP_CHAIN_SEEN,
                "captured the game swap chain with an object-local shadow vtable; HDR observation hooks are active",
            );
            unsafe { ensure_modern_swap_chain_hooks(swap_chain) };
            note_active_swap_chain(swap_chain);
            unsafe { log_swap_chain_state(swap_chain, source, true) };
        }
        Err(error) => log(format!("cannot patch the game swap chain: {error}")),
    }
}

unsafe fn patch_swap_chain(swap_chain: *mut c_void, entry_count: usize) -> Result<(), String> {
    let mut hooks = vec![
        VtableHook {
            index: SWAP_CHAIN_QUERY_INTERFACE_INDEX,
            replacement: swap_chain_query_interface_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::QueryInterface",
        },
        VtableHook {
            index: UNKNOWN_RELEASE_INDEX,
            replacement: swap_chain_release_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::Release",
        },
        VtableHook {
            index: SWAP_CHAIN_PRESENT_INDEX,
            replacement: present_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::Present",
        },
        VtableHook {
            index: SWAP_CHAIN_SET_FULLSCREEN_STATE_INDEX,
            replacement: set_fullscreen_state_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::SetFullscreenState",
        },
        VtableHook {
            index: SWAP_CHAIN_RESIZE_BUFFERS_INDEX,
            replacement: resize_buffers_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::ResizeBuffers",
        },
        VtableHook {
            index: SWAP_CHAIN_RESIZE_TARGET_INDEX,
            replacement: resize_target_hook as *const () as *mut c_void,
            name: "IDXGISwapChain::ResizeTarget",
        },
    ];
    if entry_count > SWAP_CHAIN_PRESENT1_INDEX {
        hooks.push(VtableHook {
            index: SWAP_CHAIN_PRESENT1_INDEX,
            replacement: present1_hook as *const () as *mut c_void,
            name: "IDXGISwapChain1::Present1",
        });
    }
    if entry_count > SWAP_CHAIN_SET_COLOR_SPACE1_INDEX {
        hooks.extend([
            VtableHook {
                index: SWAP_CHAIN_CHECK_COLOR_SPACE_SUPPORT_INDEX,
                replacement: check_color_space_support_hook as *const () as *mut c_void,
                name: "IDXGISwapChain3::CheckColorSpaceSupport",
            },
            VtableHook {
                index: SWAP_CHAIN_SET_COLOR_SPACE1_INDEX,
                replacement: set_color_space1_hook as *const () as *mut c_void,
                name: "IDXGISwapChain3::SetColorSpace1",
            },
        ]);
    }
    if entry_count > SWAP_CHAIN_RESIZE_BUFFERS1_INDEX {
        hooks.push(VtableHook {
            index: SWAP_CHAIN_RESIZE_BUFFERS1_INDEX,
            replacement: resize_buffers1_hook as *const () as *mut c_void,
            name: "IDXGISwapChain3::ResizeBuffers1",
        });
    }
    if entry_count > SWAP_CHAIN_SET_HDR_METADATA_INDEX {
        hooks.push(VtableHook {
            index: SWAP_CHAIN_SET_HDR_METADATA_INDEX,
            replacement: set_hdr_metadata_hook as *const () as *mut c_void,
            name: "IDXGISwapChain4::SetHDRMetaData",
        });
    }
    unsafe { install_shadow_vtable(swap_chain, ShadowKind::SwapChain, entry_count, &hooks) }
}

unsafe extern "system" fn swap_chain_release_hook(this: *mut c_void) -> u32 {
    unsafe { release_shadow_object(this, ShadowKind::SwapChain) }
}

unsafe extern "system" fn swap_chain_query_interface_hook(
    this: *mut c_void,
    riid: *const Guid,
    object: *mut *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<QueryInterface>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_QUERY_INTERFACE_INDEX,
        )
    }) else {
        return -1;
    };
    let result = unsafe { function(this, riid, object) };
    if result >= S_OK_MINIMUM && !riid.is_null() && !object.is_null() {
        let queried = unsafe { *object };
        if !queried.is_null() {
            let interface = unsafe { &*riid };
            if let Some(entry_count) = swap_chain_vtable_len(interface) {
                match unsafe { patch_swap_chain(queried, entry_count) } {
                    Ok(()) => ensure_swap_chain_state(queried),
                    Err(error) => log(format!(
                        "cannot hook the queried DXGI swap-chain interface: {error}"
                    )),
                }
            }
        }
    }
    result
}

unsafe extern "system" fn present_hook(
    this: *mut c_void,
    sync_interval: u32,
    flags: u32,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<Present>(this, ShadowKind::SwapChain, SWAP_CHAIN_PRESENT_INDEX)
    }) else {
        return -1;
    };
    unsafe { apply_managed_color_space_before_present(this) };
    unsafe { note_first_present(this, "Present", sync_interval, flags) };
    unsafe { function(this, sync_interval, flags) }
}

unsafe extern "system" fn present1_hook(
    this: *mut c_void,
    sync_interval: u32,
    flags: u32,
    parameters: *const c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<Present1>(this, ShadowKind::SwapChain, SWAP_CHAIN_PRESENT1_INDEX)
    }) else {
        return -1;
    };
    unsafe { apply_managed_color_space_before_present(this) };
    unsafe { note_first_present(this, "Present1", sync_interval, flags) };
    unsafe { function(this, sync_interval, flags, parameters) }
}

unsafe extern "system" fn set_fullscreen_state_hook(
    this: *mut c_void,
    fullscreen: i32,
    target: *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<SetFullscreenState>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_SET_FULLSCREEN_STATE_INDEX,
        )
    }) else {
        return -1;
    };
    log(format!(
        "SetFullscreenState request: object={this:p}, fullscreen={}, target={target:p}",
        fullscreen != 0
    ));
    let result = unsafe { function(this, fullscreen, target) };
    log(format!(
        "SetFullscreenState returned HRESULT 0x{:08X}",
        result as u32
    ));
    if result >= S_OK_MINIMUM {
        advance_swap_chain_generation(this);
        unsafe { log_swap_chain_state(this, "SetFullscreenState", true) };
    }
    result
}

unsafe extern "system" fn resize_buffers_hook(
    this: *mut c_void,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: u32,
    flags: u32,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<ResizeBuffers>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_RESIZE_BUFFERS_INDEX,
        )
    }) else {
        return -1;
    };
    log(format!(
        "ResizeBuffers request: object={this:p}, buffers={buffer_count}, size={width}x{height}, format={new_format} [{}], flags=0x{flags:08X}",
        format_name(new_format)
    ));
    let result = unsafe { function(this, buffer_count, width, height, new_format, flags) };
    log(format!(
        "ResizeBuffers returned HRESULT 0x{:08X}",
        result as u32
    ));
    if result >= S_OK_MINIMUM {
        advance_swap_chain_generation(this);
        unsafe { log_swap_chain_state(this, "ResizeBuffers", true) };
    }
    result
}

unsafe extern "system" fn resize_target_hook(
    this: *mut c_void,
    requested: *const ModeDesc,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<ResizeTarget>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_RESIZE_TARGET_INDEX,
        )
    }) else {
        return -1;
    };
    if requested.is_null() {
        log(format!("ResizeTarget request: object={this:p}, mode=NULL"));
    } else {
        let mode = unsafe { &*requested };
        log(format!(
            "ResizeTarget request: object={this:p}, size={}x{}, refresh={}, format={} [{}]",
            mode.width,
            mode.height,
            rational_text(mode.refresh_rate),
            mode.format,
            format_name(mode.format)
        ));
    }
    let result = unsafe { function(this, requested) };
    log(format!(
        "ResizeTarget returned HRESULT 0x{:08X}",
        result as u32
    ));
    if result >= S_OK_MINIMUM {
        unsafe { log_swap_chain_state(this, "ResizeTarget", true) };
        note_active_swap_chain_reconfigured(this);
    }
    result
}

unsafe extern "system" fn resize_buffers1_hook(
    this: *mut c_void,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: u32,
    flags: u32,
    creation_node_mask: *const u32,
    present_queue: *const *mut c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<ResizeBuffers1>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_RESIZE_BUFFERS1_INDEX,
        )
    }) else {
        return -1;
    };
    log(format!(
        "ResizeBuffers1 request: object={this:p}, buffers={buffer_count}, size={width}x{height}, format={new_format} [{}], flags=0x{flags:08X}",
        format_name(new_format)
    ));
    let result = unsafe {
        function(
            this,
            buffer_count,
            width,
            height,
            new_format,
            flags,
            creation_node_mask,
            present_queue,
        )
    };
    log(format!(
        "ResizeBuffers1 returned HRESULT 0x{:08X}",
        result as u32
    ));
    if result >= S_OK_MINIMUM {
        advance_swap_chain_generation(this);
        unsafe { log_swap_chain_state(this, "ResizeBuffers1", true) };
    }
    result
}

unsafe extern "system" fn check_color_space_support_hook(
    this: *mut c_void,
    color_space: u32,
    support: *mut u32,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<CheckColorSpaceSupport>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_CHECK_COLOR_SPACE_SUPPORT_INDEX,
        )
    }) else {
        return -1;
    };
    let result = unsafe { function(this, color_space, support) };
    // DXGI does not promise an initialized output value after a failed call.
    // Read the caller-owned pointer only on success to avoid observing
    // indeterminate memory in this transparent diagnostic hook.
    if result >= S_OK_MINIMUM && !support.is_null() {
        let support_value = unsafe { *support };
        log(format!(
            "CheckColorSpaceSupport: object={this:p}, color_space={color_space} [{}], HRESULT=0x{:08X}, support=0x{support_value:08X} (PRESENT={})",
            color_space_name(color_space),
            result as u32,
            support_value & DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT != 0
        ));
    } else {
        log(format!(
            "CheckColorSpaceSupport: object={this:p}, color_space={color_space} [{}], HRESULT=0x{:08X}, support=unavailable",
            color_space_name(color_space),
            result as u32,
        ));
    }
    result
}

unsafe extern "system" fn set_color_space1_hook(
    this: *mut c_void,
    requested_color_space: u32,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<SetColorSpace1>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_SET_COLOR_SPACE1_INDEX,
        )
    }) else {
        return -1;
    };

    let result = unsafe { function(this, requested_color_space) };
    let mut conflict = false;
    if result >= S_OK_MINIMUM
        && let Ok(mut states) = swap_chain_states().lock()
    {
        let state = states.entry(this as usize).or_default();
        state.observed_color_space = requested_color_space;
        conflict = state
            .managed_color_space
            .observe_external_set(requested_color_space);
    }
    log(format!(
        "SetColorSpace1: object={this:p}, requested={requested_color_space} [{}], forwarded unchanged, HRESULT=0x{:08X}",
        color_space_name(requested_color_space),
        result as u32
    ));
    if conflict {
        log(format!(
            "SAFETY: external SetColorSpace1 changed object={this:p} while managed HDR color-space synchronization was active; relinquishing the experiment for this swap chain"
        ));
    }
    result
}

unsafe extern "system" fn set_hdr_metadata_hook(
    this: *mut c_void,
    metadata_type: u32,
    size: u32,
    metadata: *const c_void,
) -> HResult {
    let Some(function) = (unsafe {
        load_object_function::<SetHdrMetadata>(
            this,
            ShadowKind::SwapChain,
            SWAP_CHAIN_SET_HDR_METADATA_INDEX,
        )
    }) else {
        return -1;
    };

    if metadata_type == 1
        && !metadata.is_null()
        && (size as usize) >= mem::size_of::<HdrMetadataHdr10>()
    {
        let value = unsafe { ptr::read_unaligned(metadata.cast::<HdrMetadataHdr10>()) };
        log(format!(
            "SetHDRMetaData HDR10: object={this:p}, size={size}, mastering={}..{}, MaxCLL={}, MaxFALL={}, primaries R={:?} G={:?} B={:?} W={:?}",
            value.min_mastering_luminance,
            value.max_mastering_luminance,
            value.max_content_light_level,
            value.max_frame_average_light_level,
            value.red_primary,
            value.green_primary,
            value.blue_primary,
            value.white_point
        ));
    } else {
        log(format!(
            "SetHDRMetaData: object={this:p}, type={metadata_type}, size={size}, metadata={metadata:p}"
        ));
    }
    let result = unsafe { function(this, metadata_type, size, metadata) };
    log(format!(
        "SetHDRMetaData returned HRESULT 0x{:08X}",
        result as u32
    ));
    result
}

struct OutputSnapshot {
    description: OutputDesc1,
    device_name: String,
}

pub(crate) struct WindowedHdrCandidate {
    pub eligible: bool,
    pub details: String,
}

/// Rechecks the concrete DXGI object used by the game's internal HDR-state
/// query. This is intentionally read-only: it never changes fullscreen state,
/// color space, buffers, metadata, or output configuration.
///
/// # Safety
///
/// `swap_chain` must be either the live `IDXGISwapChain` pointer obtained from
/// the verified game backend object or the active pointer captured from the
/// game's factory creation path. The function rejects objects that were not
/// previously captured by this module's object-local shadow-vtable hooks.
pub(crate) unsafe fn inspect_windowed_hdr_candidate(
    swap_chain: *mut c_void,
) -> Result<WindowedHdrCandidate, String> {
    if swap_chain.is_null() {
        return Err("the game backend contains a null swap-chain pointer".to_owned());
    }
    if (unsafe { find_shadow_record(swap_chain, ShadowKind::SwapChain) }).is_none() {
        return Err(format!(
            "swap chain {swap_chain:p} was not captured by the DXGI observer"
        ));
    }

    let description = unsafe { swap_chain_description(swap_chain) }?;
    let fullscreen = unsafe { query_fullscreen_state(swap_chain) }?;
    let output = unsafe { query_output_desc1(swap_chain) }?;
    let pq_support = unsafe { query_pq_present_support(swap_chain) }?;
    let eligible = windowed_hdr_candidate_is_eligible(
        &description,
        fullscreen,
        &output.description,
        pq_support,
    );
    let details = format!(
        "swap_chain={swap_chain:p}, format={} [{}], buffers={}, windowed={}, exclusive_fullscreen={}, swap_effect={} [{}], output={}, attached={}, bpc={}, output_color_space={} [{}], PQ_PRESENT_support=0x{pq_support:08X}",
        description.buffer_desc.format,
        format_name(description.buffer_desc.format),
        description.buffer_count,
        description.windowed != 0,
        fullscreen,
        description.swap_effect,
        swap_effect_name(description.swap_effect),
        output.device_name,
        output.description.attached_to_desktop != 0,
        output.description.bits_per_color,
        output.description.color_space,
        color_space_name(output.description.color_space),
    );
    Ok(WindowedHdrCandidate { eligible, details })
}

fn windowed_hdr_candidate_is_eligible(
    description: &SwapChainDesc,
    fullscreen: bool,
    output: &OutputDesc1,
    pq_support: u32,
) -> bool {
    description.buffer_desc.format == 24
        && description.buffer_count >= 2
        && description.windowed != 0
        && !fullscreen
        && matches!(description.swap_effect, 3 | 4)
        && output.attached_to_desktop != 0
        && output.bits_per_color >= 10
        && output.color_space == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
        && pq_support & DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT != 0
}

/// Arms a color-space transition for the next Present on a captured swap
/// chain. Enabling records the most recently observed color space as the exact
/// restoration target; disabling never assumes that target is SDR.
///
/// The return value is false only after a previous managed transition failed
/// or an external SetColorSpace1 call conflicted with this experiment. Such a
/// failure is latched for the lifetime of the swap chain so a render-thread
/// failure cannot turn into an unbounded retry loop.
///
/// # Safety
///
/// `swap_chain` must be the live pointer from the verified game backend. The
/// object must have been captured by this module's shadow-vtable machinery.
pub(crate) unsafe fn request_managed_hdr_color_space(
    swap_chain: *mut c_void,
    hdr: bool,
) -> Result<bool, String> {
    if swap_chain.is_null() {
        return Err("cannot manage the color space of a null swap chain".to_owned());
    }
    if (unsafe { find_shadow_record(swap_chain, ShadowKind::SwapChain) }).is_none() {
        return Err(format!(
            "swap chain {swap_chain:p} was not captured by the DXGI observer"
        ));
    }

    let (accepted, changed, restore_color_space) = {
        let mut states = swap_chain_states()
            .lock()
            .map_err(|_| "swap-chain state lock is poisoned".to_owned())?;
        let state = states.entry(swap_chain as usize).or_default();
        let before = state.managed_color_space;
        let accepted = state
            .managed_color_space
            .request(hdr, state.observed_color_space);
        (
            accepted,
            before != state.managed_color_space,
            state.managed_color_space.restore_color_space,
        )
    };

    if changed {
        let target = if hdr {
            DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
        } else {
            restore_color_space.unwrap_or(DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709)
        };
        log(format!(
            "HDR color-space synchronization request: object={swap_chain:p}, desired={}, target={target} [{}], captured_restore={} [{}]; transition will run immediately before the next Present",
            if hdr { "HDR/PQ" } else { "restore" },
            color_space_name(target),
            restore_color_space
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            restore_color_space.map(color_space_name).unwrap_or("none"),
        ));
    }
    Ok(accepted)
}

unsafe fn apply_managed_color_space_before_present(swap_chain: *mut c_void) {
    let transition = if let Ok(mut states) = swap_chain_states().lock() {
        states
            .entry(swap_chain as usize)
            .or_default()
            .managed_color_space
            .begin_transition()
    } else {
        None
    };
    let Some(transition) = transition else {
        return;
    };

    let call_result = if transition.enabling_hdr {
        match unsafe { inspect_windowed_hdr_candidate(swap_chain) } {
            Ok(candidate) if candidate.eligible => {
                log(format!(
                    "managed HDR color-space Present validation: eligible=true; {}",
                    candidate.details
                ));
                unsafe {
                    set_color_space_without_observer(swap_chain, transition.target_color_space)
                }
            }
            Ok(candidate) => Err(format!(
                "Present-time HDR candidate validation rejected the transition: {}",
                candidate.details
            )),
            Err(error) => Err(format!(
                "Present-time HDR candidate validation failed: {error}"
            )),
        }
    } else {
        unsafe { set_color_space_without_observer(swap_chain, transition.target_color_space) }
    };
    let succeeded = matches!(call_result, Ok(result) if result >= S_OK_MINIMUM);
    if let Ok(mut states) = swap_chain_states().lock() {
        let state = states.entry(swap_chain as usize).or_default();
        if succeeded {
            state.observed_color_space = transition.target_color_space;
        }
        state
            .managed_color_space
            .finish_transition(transition, succeeded);
    }

    match call_result {
        Ok(result) => log(format!(
            "managed SetColorSpace1 before Present: object={swap_chain:p}, transition={}, requested={} [{}], HRESULT=0x{:08X}, success={succeeded}",
            if transition.enabling_hdr {
                "enable_hdr_pq"
            } else {
                "restore_previous"
            },
            transition.target_color_space,
            color_space_name(transition.target_color_space),
            result as u32,
        )),
        Err(error) => log(format!(
            "managed SetColorSpace1 before Present failed before the call: object={swap_chain:p}, transition={}, requested={} [{}], error={error}; the failure is latched and HDR emulation will stop",
            if transition.enabling_hdr {
                "enable_hdr_pq"
            } else {
                "restore_previous"
            },
            transition.target_color_space,
            color_space_name(transition.target_color_space),
        )),
    }
}

unsafe fn set_color_space_without_observer(
    swap_chain: *mut c_void,
    color_space: u32,
) -> Result<HResult, String> {
    let swap_chain3 = unsafe { query_swap_chain_interface(swap_chain, &IID_IDXGI_SWAP_CHAIN3) }?;
    let function = unsafe {
        load_object_function::<SetColorSpace1>(
            swap_chain3,
            ShadowKind::SwapChain,
            SWAP_CHAIN_SET_COLOR_SPACE1_INDEX,
        )
    };
    let result = match function {
        Some(function) => Ok(unsafe { function(swap_chain3, color_space) }),
        None => Err("captured IDXGISwapChain3 has no saved SetColorSpace1 chain".to_owned()),
    };
    unsafe { release_com_object(swap_chain3) };
    result
}

fn swap_chain_states() -> &'static Mutex<HashMap<usize, SwapChainState>> {
    SWAP_CHAIN_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_swap_chain_state(swap_chain: *mut c_void) {
    if let Ok(mut states) = swap_chain_states().lock() {
        states.entry(swap_chain as usize).or_default();
    }
}

fn advance_swap_chain_generation(swap_chain: *mut c_void) {
    if let Ok(mut states) = swap_chain_states().lock() {
        let state = states.entry(swap_chain as usize).or_default();
        state.generation = state.generation.wrapping_add(1);
        state.first_present_logged = false;
        state.last_signature = None;
        state.managed_color_space.reapply_after_reconfiguration();
    }
    note_active_swap_chain_reconfigured(swap_chain);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveSwapChainSnapshot {
    pub swap_chain: *mut c_void,
    pub revision: usize,
}

pub(crate) fn active_swap_chain_snapshot() -> Option<ActiveSwapChainSnapshot> {
    loop {
        let before = ACTIVE_SWAP_CHAIN_REVISION.load(Ordering::Acquire);
        let swap_chain = ACTIVE_SWAP_CHAIN.load(Ordering::Acquire);
        let after = ACTIVE_SWAP_CHAIN_REVISION.load(Ordering::Acquire);
        if before != after {
            continue;
        }
        return (!swap_chain.is_null()).then_some(ActiveSwapChainSnapshot {
            swap_chain,
            revision: after,
        });
    }
}

pub(crate) fn swap_chain_revision(swap_chain: *mut c_void) -> usize {
    if ACTIVE_SWAP_CHAIN.load(Ordering::Acquire) == swap_chain {
        ACTIVE_SWAP_CHAIN_REVISION.load(Ordering::Acquire)
    } else {
        0
    }
}

pub(crate) fn note_game_backend_swap_chain(swap_chain: *mut c_void) {
    note_active_swap_chain(swap_chain);
}

fn note_active_swap_chain(swap_chain: *mut c_void) {
    if swap_chain.is_null() {
        return;
    }
    let previous = ACTIVE_SWAP_CHAIN.swap(swap_chain, Ordering::AcqRel);
    if previous != swap_chain {
        ACTIVE_SWAP_CHAIN_REVISION.fetch_add(1, Ordering::AcqRel);
    }
}

fn note_active_swap_chain_reconfigured(swap_chain: *mut c_void) {
    if ACTIVE_SWAP_CHAIN.load(Ordering::Acquire) == swap_chain {
        ACTIVE_SWAP_CHAIN_REVISION.fetch_add(1, Ordering::AcqRel);
    }
}

unsafe fn note_first_present(
    swap_chain: *mut c_void,
    method: &str,
    sync_interval: u32,
    flags: u32,
) {
    let should_log = if let Ok(mut states) = swap_chain_states().lock() {
        let state = states.entry(swap_chain as usize).or_default();
        if state.first_present_logged {
            false
        } else {
            state.first_present_logged = true;
            true
        }
    } else {
        false
    };
    if !should_log {
        return;
    }

    let fullscreen = unsafe { query_fullscreen_state(swap_chain) }.unwrap_or(false);
    log(format!(
        "first {method} for generation: object={swap_chain:p}, fullscreen={fullscreen}, SyncInterval={sync_interval}, flags=0x{flags:08X}"
    ));
    unsafe { log_swap_chain_state(swap_chain, method, false) };
}

unsafe fn ensure_modern_swap_chain_hooks(swap_chain: *mut c_void) {
    for (interface, label) in [
        (&IID_IDXGI_SWAP_CHAIN4, "IDXGISwapChain4"),
        (&IID_IDXGI_SWAP_CHAIN3, "IDXGISwapChain3"),
    ] {
        match unsafe { query_swap_chain_interface(swap_chain, interface) } {
            Ok(queried) => {
                log(format!(
                    "queried {label} successfully for object {swap_chain:p}; HDR methods are observable"
                ));
                unsafe { release_com_object(queried) };
                return;
            }
            Err(error) if *interface == IID_IDXGI_SWAP_CHAIN3 => {
                log(format!(
                    "swap chain exposes neither IDXGISwapChain4 nor IDXGISwapChain3: {error}"
                ));
            }
            Err(_) => {}
        }
    }
}

unsafe fn query_swap_chain_interface(
    swap_chain: *mut c_void,
    interface: &Guid,
) -> Result<*mut c_void, String> {
    let function = unsafe {
        load_object_function::<QueryInterface>(
            swap_chain,
            ShadowKind::SwapChain,
            SWAP_CHAIN_QUERY_INTERFACE_INDEX,
        )
    }
    .ok_or_else(|| "swap-chain QueryInterface original function is unavailable".to_owned())?;
    let mut queried = ptr::null_mut();
    let result = unsafe { function(swap_chain, interface, &mut queried) };
    if result < S_OK_MINIMUM || queried.is_null() {
        Err(format!(
            "QueryInterface returned HRESULT 0x{:08X}",
            result as u32
        ))
    } else {
        if let Some(entry_count) = swap_chain_vtable_len(interface)
            && let Err(error) = unsafe { patch_swap_chain(queried, entry_count) }
        {
            unsafe { release_com_object(queried) };
            return Err(format!("cannot hook queried swap-chain interface: {error}"));
        }
        ensure_swap_chain_state(queried);
        Ok(queried)
    }
}

unsafe fn query_pq_present_support(swap_chain: *mut c_void) -> Result<u32, String> {
    let query: QueryInterface =
        unsafe { vtable_function(swap_chain, SWAP_CHAIN_QUERY_INTERFACE_INDEX)? };
    let mut swap_chain3 = ptr::null_mut();
    let query_result = unsafe { query(swap_chain, &IID_IDXGI_SWAP_CHAIN3, &mut swap_chain3) };
    if query_result < S_OK_MINIMUM || swap_chain3.is_null() {
        return Err(format!(
            "IDXGISwapChain::QueryInterface(IDXGISwapChain3) returned HRESULT 0x{:08X}",
            query_result as u32
        ));
    }

    let result = match unsafe {
        vtable_function::<CheckColorSpaceSupport>(
            swap_chain3,
            SWAP_CHAIN_CHECK_COLOR_SPACE_SUPPORT_INDEX,
        )
    } {
        Ok(check) => {
            let mut support = 0u32;
            let check_result = unsafe {
                check(
                    swap_chain3,
                    DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
                    &mut support,
                )
            };
            if check_result < S_OK_MINIMUM {
                Err(format!(
                    "IDXGISwapChain3::CheckColorSpaceSupport(PQ) returned HRESULT 0x{:08X}",
                    check_result as u32
                ))
            } else {
                Ok(support)
            }
        }
        Err(error) => Err(error),
    };
    unsafe { release_com_object(swap_chain3) };
    result
}

unsafe fn log_swap_chain_state(swap_chain: *mut c_void, source: &str, force: bool) {
    let description = match unsafe { swap_chain_description(swap_chain) } {
        Ok(description) => description,
        Err(error) => {
            log(format!(
                "cannot read swap-chain state after {source}: {error}"
            ));
            return;
        }
    };
    let fullscreen = unsafe { query_fullscreen_state(swap_chain) }.unwrap_or(false);
    let signature = SwapChainSignature {
        width: description.buffer_desc.width,
        height: description.buffer_desc.height,
        format: description.buffer_desc.format,
        buffer_count: description.buffer_count,
        windowed: description.windowed != 0,
        swap_effect: description.swap_effect,
        flags: description.flags,
    };
    let changed = if let Ok(mut states) = swap_chain_states().lock() {
        let state = states.entry(swap_chain as usize).or_default();
        let changed = state.last_signature != Some(signature);
        state.last_signature = Some(signature);
        changed
    } else {
        true
    };
    if !force && !changed {
        return;
    }

    log(format!(
        "swap-chain state ({source}): object={swap_chain:p}, size={}x{}, refresh={}, format={} [{}], buffers={}, windowed={}, exclusive_fullscreen={}, swap_effect={} [{}], flags=0x{:08X}",
        description.buffer_desc.width,
        description.buffer_desc.height,
        rational_text(description.buffer_desc.refresh_rate),
        description.buffer_desc.format,
        format_name(description.buffer_desc.format),
        description.buffer_count,
        description.windowed != 0,
        fullscreen,
        description.swap_effect,
        swap_effect_name(description.swap_effect),
        description.flags
    ));
    match unsafe { swap_chain_description1(swap_chain) } {
        Ok(Some(description1)) => log(format!(
            "swap-chain DESC1 ({source}): size={}x{}, format={} [{}], buffers={}, sample={}x/q{}, scaling={}, swap_effect={} [{}], alpha={}, flags=0x{:08X}",
            description1.width,
            description1.height,
            description1.format,
            format_name(description1.format),
            description1.buffer_count,
            description1.sample_desc.count,
            description1.sample_desc.quality,
            description1.scaling,
            description1.swap_effect,
            swap_effect_name(description1.swap_effect),
            description1.alpha_mode,
            description1.flags
        )),
        Ok(None) => {}
        Err(error) => log(format!(
            "cannot read IDXGISwapChain1::GetDesc1 after {source}: {error}"
        )),
    }
    match unsafe { query_output_desc1(swap_chain) } {
        Ok(output) => log(format!(
            "output state ({source}): device={}, attached={}, {} bpc, color_space={} [{}], luminance={:.3}..{:.3} nits, max_full_frame={:.3} nits, desktop=({}, {})-({}, {})",
            output.device_name,
            output.description.attached_to_desktop != 0,
            output.description.bits_per_color,
            output.description.color_space,
            color_space_name(output.description.color_space),
            output.description.min_luminance,
            output.description.max_luminance,
            output.description.max_full_frame_luminance,
            output.description.desktop_coordinates.left,
            output.description.desktop_coordinates.top,
            output.description.desktop_coordinates.right,
            output.description.desktop_coordinates.bottom
        )),
        Err(error) => log(format!(
            "cannot read IDXGIOutput6::GetDesc1 after {source}: {error}"
        )),
    }
}

unsafe fn log_legacy_creation_description(source: &str, pointer: *const c_void) {
    if pointer.is_null() {
        log(format!("{source}: description=NULL"));
        return;
    }
    let description = unsafe { &*pointer.cast::<SwapChainDesc>() };
    log(format!(
        "{source}: size={}x{}, refresh={}, format={} [{}], buffers={}, windowed={}, swap_effect={} [{}], flags=0x{:08X}",
        description.buffer_desc.width,
        description.buffer_desc.height,
        rational_text(description.buffer_desc.refresh_rate),
        description.buffer_desc.format,
        format_name(description.buffer_desc.format),
        description.buffer_count,
        description.windowed != 0,
        description.swap_effect,
        swap_effect_name(description.swap_effect),
        description.flags
    ));
}

unsafe fn log_desc1_creation_description(
    source: &str,
    pointer: *const c_void,
    fullscreen_pointer: *const c_void,
) {
    if pointer.is_null() {
        log(format!("{source}: description=NULL"));
        return;
    }
    let description = unsafe { &*pointer.cast::<SwapChainDesc1>() };
    let windowed = if fullscreen_pointer.is_null() {
        "default".to_owned()
    } else {
        let fullscreen = unsafe { &*fullscreen_pointer.cast::<FullscreenDesc>() };
        format!(
            "{} (refresh={})",
            fullscreen.windowed != 0,
            rational_text(fullscreen.refresh_rate)
        )
    };
    log(format!(
        "{source}: size={}x{}, format={} [{}], buffers={}, sample={}x/q{}, scaling={}, swap_effect={} [{}], alpha={}, flags=0x{:08X}, windowed={windowed}",
        description.width,
        description.height,
        description.format,
        format_name(description.format),
        description.buffer_count,
        description.sample_desc.count,
        description.sample_desc.quality,
        description.scaling,
        description.swap_effect,
        swap_effect_name(description.swap_effect),
        description.alpha_mode,
        description.flags
    ));
}

unsafe fn swap_chain_description(swap_chain: *mut c_void) -> Result<SwapChainDesc, String> {
    let function: GetSwapChainDesc =
        unsafe { vtable_function(swap_chain, SWAP_CHAIN_GET_DESC_INDEX)? };
    let mut description = SwapChainDesc::default();
    let result = unsafe { function(swap_chain, &mut description) };
    if result < S_OK_MINIMUM {
        Err(format!(
            "IDXGISwapChain::GetDesc returned HRESULT 0x{:08X}",
            result as u32
        ))
    } else {
        Ok(description)
    }
}

unsafe fn swap_chain_description1(
    swap_chain: *mut c_void,
) -> Result<Option<SwapChainDesc1>, String> {
    let Some(record) = (unsafe { find_shadow_record(swap_chain, ShadowKind::SwapChain) }) else {
        return Ok(None);
    };
    if unsafe { &*record }.entry_count.load(Ordering::Acquire) <= SWAP_CHAIN_GET_DESC1_INDEX {
        return Ok(None);
    }
    let function = unsafe {
        load_object_function::<GetSwapChainDesc1>(
            swap_chain,
            ShadowKind::SwapChain,
            SWAP_CHAIN_GET_DESC1_INDEX,
        )
    }
    .ok_or_else(|| "IDXGISwapChain1::GetDesc1 original function is unavailable".to_owned())?;
    let mut description = SwapChainDesc1::default();
    let result = unsafe { function(swap_chain, &mut description) };
    if result < S_OK_MINIMUM {
        Err(format!(
            "IDXGISwapChain1::GetDesc1 returned HRESULT 0x{:08X}",
            result as u32
        ))
    } else {
        Ok(Some(description))
    }
}

unsafe fn containing_output(swap_chain: *mut c_void) -> Result<*mut c_void, String> {
    let function: GetContainingOutput =
        unsafe { vtable_function(swap_chain, SWAP_CHAIN_GET_CONTAINING_OUTPUT_INDEX)? };
    let mut output = ptr::null_mut();
    let result = unsafe { function(swap_chain, &mut output) };
    if result < S_OK_MINIMUM || output.is_null() {
        Err(format!(
            "IDXGISwapChain::GetContainingOutput returned HRESULT 0x{:08X}",
            result as u32
        ))
    } else {
        Ok(output)
    }
}

unsafe fn query_output_desc1(swap_chain: *mut c_void) -> Result<OutputSnapshot, String> {
    let output = unsafe { containing_output(swap_chain)? };
    let query: QueryInterface = match unsafe { vtable_function(output, 0) } {
        Ok(function) => function,
        Err(error) => {
            unsafe { release_com_object(output) };
            return Err(error);
        }
    };
    let mut output6 = ptr::null_mut();
    let query_result = unsafe { query(output, &IID_IDXGI_OUTPUT6, &mut output6) };
    if query_result < S_OK_MINIMUM || output6.is_null() {
        unsafe { release_com_object(output) };
        return Err(format!(
            "IDXGIOutput::QueryInterface(IDXGIOutput6) returned HRESULT 0x{:08X}",
            query_result as u32
        ));
    }

    let get_description: GetOutputDesc1 =
        match unsafe { vtable_function(output6, OUTPUT_GET_DESC1_INDEX) } {
            Ok(function) => function,
            Err(error) => {
                unsafe {
                    release_com_object(output6);
                    release_com_object(output);
                }
                return Err(error);
            }
        };
    let mut description = OutputDesc1::default();
    let result = unsafe { get_description(output6, &mut description) };
    unsafe {
        release_com_object(output6);
        release_com_object(output);
    }
    if result < S_OK_MINIMUM {
        return Err(format!(
            "IDXGIOutput6::GetDesc1 returned HRESULT 0x{:08X}",
            result as u32
        ));
    }
    let length = description
        .device_name
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(description.device_name.len());
    Ok(OutputSnapshot {
        device_name: String::from_utf16_lossy(&description.device_name[..length]),
        description,
    })
}

unsafe fn release_com_object(object: *mut c_void) {
    if let Ok(function) = unsafe { vtable_function::<Release>(object, UNKNOWN_RELEASE_INDEX) } {
        let _ = unsafe { function(object) };
    }
}

unsafe fn vtable_function<T: Copy>(object: *mut c_void, index: usize) -> Result<T, String> {
    let vtable = unsafe { object_vtable(object)? };
    let pointer = unsafe { *vtable.add(index) };
    if pointer.is_null() {
        Err(format!("COM vtable entry {index} is null"))
    } else {
        Ok(unsafe { mem::transmute_copy::<*mut c_void, T>(&pointer) })
    }
}

unsafe fn query_fullscreen_state(swap_chain: *mut c_void) -> Result<bool, String> {
    let function: GetFullscreenState =
        unsafe { vtable_function(swap_chain, SWAP_CHAIN_GET_FULLSCREEN_STATE_INDEX)? };
    let mut fullscreen = 0i32;
    let result = unsafe { function(swap_chain, &mut fullscreen, ptr::null_mut()) };
    if result < S_OK_MINIMUM {
        Err(format!(
            "IDXGISwapChain::GetFullscreenState returned HRESULT 0x{:08X}",
            result as u32
        ))
    } else {
        Ok(fullscreen != 0)
    }
}

fn rational_text(value: Rational) -> String {
    if value.numerator == 0 || value.denominator == 0 {
        "unspecified".to_owned()
    } else {
        format!(
            "{}/{} ({:.3} Hz)",
            value.numerator,
            value.denominator,
            f64::from(value.numerator) / f64::from(value.denominator)
        )
    }
}

fn format_name(format: u32) -> &'static str {
    match format {
        0 => "UNKNOWN",
        10 => "R16G16B16A16_FLOAT",
        24 => "R10G10B10A2_UNORM",
        28 => "R8G8B8A8_UNORM",
        29 => "R8G8B8A8_UNORM_SRGB",
        87 => "B8G8R8A8_UNORM",
        91 => "B8G8R8A8_UNORM_SRGB",
        _ => "unrecognized",
    }
}

fn color_space_name(color_space: u32) -> &'static str {
    match color_space {
        DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709 => "RGB_FULL_G22_NONE_P709 (SDR)",
        DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709 => "RGB_FULL_G10_NONE_P709 (scRGB)",
        DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020 => "RGB_FULL_G2084_NONE_P2020 (HDR10/PQ)",
        _ => "unrecognized",
    }
}

fn swap_effect_name(swap_effect: u32) -> &'static str {
    match swap_effect {
        0 => "DISCARD",
        1 => "SEQUENTIAL",
        3 => "FLIP_SEQUENTIAL",
        4 => "FLIP_DISCARD",
        _ => "unrecognized",
    }
}

fn ags_mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "SDR",
        1 => "scRGB",
        2 => "PQ",
        3 => "Dolby Vision",
        _ => "unrecognized",
    }
}

unsafe fn release_shadow_object(object: *mut c_void, kind: ShadowKind) -> u32 {
    let Some(record) = (unsafe { find_shadow_record(object, kind) }) else {
        return 0;
    };
    let Some(function) =
        (unsafe { load_saved_record_function::<Release>(record, UNKNOWN_RELEASE_INDEX) })
    else {
        return 0;
    };

    let remaining = unsafe { function(object) };
    if remaining == 0 {
        unsafe { &*record }.active.store(false, Ordering::Release);
        if kind == ShadowKind::SwapChain {
            if ACTIVE_SWAP_CHAIN
                .compare_exchange(object, ptr::null_mut(), Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                ACTIVE_SWAP_CHAIN_REVISION.fetch_add(1, Ordering::AcqRel);
            }
            if let Ok(mut states) = swap_chain_states().lock() {
                states.remove(&(object as usize));
            }
        }
    }
    remaining
}

unsafe fn install_shadow_vtable(
    object: *mut c_void,
    kind: ShadowKind,
    entry_count: usize,
    hooks: &[VtableHook],
) -> Result<(), String> {
    let lock = SHADOW_INSTALL_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "shadow-vtable installation lock is poisoned".to_owned())?;
    let current_vtable = unsafe { object_vtable(object)? };
    let existing = unsafe { find_shadow_record(object, kind) };

    if entry_count == 0 {
        return Err(format!("{kind:?} has no usable original vtable"));
    }
    for hook in hooks {
        if hook.index >= entry_count {
            return Err(format!(
                "{} index {} is outside the {entry_count}-entry {kind:?} vtable",
                hook.name, hook.index
            ));
        }
    }

    if let Some(record) = existing {
        let record_ref = unsafe { &*record };
        if let Some(generation) = unsafe { find_shadow_generation(record_ref, current_vtable) } {
            if generation.entry_count >= entry_count {
                return Ok(());
            }
            return unsafe { expand_owned_shadow(record_ref, generation, entry_count, hooks) };
        }
        log(format!(
            "{kind:?} object now uses a foreign vtable; leaving the outer hook chain in control"
        ));
        return Ok(());
    }

    let root_vtable = current_vtable;
    let required_bytes = entry_count
        .checked_mul(mem::size_of::<*mut c_void>())
        .ok_or_else(|| "COM vtable size overflow".to_owned())?;
    let (shadow_region, shadow_address, shadow_region_size) = unsafe {
        windows::clone_memory_region_containing(
            root_vtable.cast(),
            mem::size_of::<*mut c_void>(),
            required_bytes,
            MAX_VTABLE_CLONE_REGION_SIZE,
        )
    }?;
    let shadow = shadow_address.cast::<*mut c_void>();
    let shadow_offset = (shadow as usize)
        .checked_sub(shadow_region as usize)
        .ok_or_else(|| "cloned COM vtable is outside its allocation".to_owned())?;
    let entry_capacity = shadow_region_size
        .checked_sub(shadow_offset)
        .ok_or_else(|| "cloned COM vtable has an invalid offset".to_owned())?
        / mem::size_of::<*mut c_void>();
    let original_entries = unsafe { std::slice::from_raw_parts(shadow, entry_count) }.to_vec();
    for hook in hooks {
        if original_entries[hook.index].is_null() {
            return Err(format!("{} original vtable entry is null", hook.name));
        }
        if original_entries[hook.index] == hook.replacement {
            return Err(format!(
                "{} original vtable entry already points at our replacement",
                hook.name
            ));
        }
    }
    for hook in hooks {
        unsafe { *shadow.add(hook.index) = hook.replacement };
    }
    unsafe { windows::protect_read_only(shadow_region, shadow_region_size) }?;

    let originals = Box::leak(original_entries.into_boxed_slice())
        .as_mut_ptr()
        .cast::<c_void>();
    let record = Box::into_raw(Box::new(ShadowRecord {
        object: object as usize,
        kind,
        active: AtomicBool::new(true),
        entry_count: AtomicUsize::new(entry_count),
        root_original_vtable: root_vtable as usize,
        live_root_slots: AtomicBool::new(unsafe {
            windows::address_is_in_module(root_vtable.cast(), "dxgi.dll")
        }),
        originals: AtomicPtr::new(originals),
        generations: AtomicPtr::new(ptr::null_mut()),
        next: ptr::null_mut(),
    }));
    unsafe { publish_shadow_record(record) };
    let record_ref = unsafe { &*record };

    let installed = match unsafe {
        windows::compare_exchange_pointer(
            object.cast::<*mut c_void>(),
            current_vtable.cast(),
            shadow.cast(),
        )
    } {
        Ok(installed) => installed,
        Err(error) => {
            record_ref.active.store(false, Ordering::Release);
            return Err(error);
        }
    };
    if !installed {
        record_ref.active.store(false, Ordering::Release);
        return Err(format!(
            "{kind:?} object vtable changed while installing its shadow; leaving the winner in control"
        ));
    }

    unsafe {
        publish_shadow_generation(
            record_ref,
            shadow,
            entry_count,
            entry_capacity,
            shadow_region,
            shadow_region_size,
        )
    };
    Ok(())
}

unsafe fn expand_owned_shadow(
    record: &ShadowRecord,
    generation: &ShadowGeneration,
    entry_count: usize,
    hooks: &[VtableHook],
) -> Result<(), String> {
    if entry_count > generation.entry_capacity {
        return Err(format!(
            "expanded COM vtable requires {entry_count} entries, but its cloned memory region only has room for {}",
            generation.entry_capacity
        ));
    }

    let shadow = generation.vtable as *mut *mut c_void;
    let old_entry_count = generation.entry_count;
    let old_originals = record
        .originals
        .load(Ordering::Acquire)
        .cast::<*mut c_void>();
    if old_originals.is_null() {
        return Err("shadow record has no original function table".to_owned());
    }

    let mut original_entries = unsafe { std::slice::from_raw_parts(shadow, entry_count) }.to_vec();
    let preserved = unsafe { std::slice::from_raw_parts(old_originals, old_entry_count) };
    original_entries[..old_entry_count].copy_from_slice(preserved);

    for hook in hooks {
        if original_entries[hook.index].is_null() {
            return Err(format!("{} original vtable entry is null", hook.name));
        }
        if original_entries[hook.index] == hook.replacement {
            return Err(format!(
                "{} original vtable entry already points at our replacement",
                hook.name
            ));
        }
    }

    let originals = Box::leak(original_entries.into_boxed_slice())
        .as_mut_ptr()
        .cast::<c_void>();
    unsafe {
        windows::protect_read_write(generation.region_base as *mut u8, generation.region_size)
    }?;
    record.originals.store(originals, Ordering::Release);
    record.entry_count.store(entry_count, Ordering::Release);

    for hook in hooks {
        let slot = unsafe { shadow.add(hook.index) };
        let current = unsafe { *slot };
        let wrapped_in_place = hook.index < old_entry_count && current != hook.replacement;
        if wrapped_in_place {
            log(format!(
                "{} is wrapped in place by another module; preserving that outer hook during interface expansion",
                hook.name
            ));
        } else if current != hook.replacement {
            unsafe { ptr::write(slot, hook.replacement) };
        }
    }

    let protection_result = unsafe {
        windows::protect_read_only(generation.region_base as *mut u8, generation.region_size)
    };
    unsafe {
        publish_shadow_generation(
            record,
            shadow,
            entry_count,
            generation.entry_capacity,
            generation.region_base as *mut u8,
            generation.region_size,
        )
    };
    protection_result
}

unsafe fn publish_shadow_record(record: *mut ShadowRecord) {
    loop {
        let head = SHADOW_RECORDS.load(Ordering::Acquire);
        unsafe { (*record).next = head };
        if SHADOW_RECORDS
            .compare_exchange_weak(head, record, Ordering::Release, Ordering::Acquire)
            .is_ok()
        {
            break;
        }
    }
}

unsafe fn publish_shadow_generation(
    record: &ShadowRecord,
    vtable: *mut *mut c_void,
    entry_count: usize,
    entry_capacity: usize,
    region_base: *mut u8,
    region_size: usize,
) {
    let head = record.generations.load(Ordering::Acquire);
    let generation = Box::into_raw(Box::new(ShadowGeneration {
        vtable: vtable as usize,
        entry_count,
        entry_capacity,
        region_base: region_base as usize,
        region_size,
        next: head,
    }));
    record.generations.store(generation, Ordering::Release);
}

unsafe fn find_shadow_record(object: *mut c_void, kind: ShadowKind) -> Option<*mut ShadowRecord> {
    let mut current = SHADOW_RECORDS.load(Ordering::Acquire);
    while !current.is_null() {
        let record = unsafe { &*current };
        if record.object == object as usize
            && record.kind == kind
            && record.active.load(Ordering::Acquire)
        {
            return Some(current);
        }
        current = record.next;
    }
    None
}

unsafe fn find_shadow_generation(
    record: &ShadowRecord,
    vtable: *mut *mut c_void,
) -> Option<&ShadowGeneration> {
    let mut current = record.generations.load(Ordering::Acquire);
    while !current.is_null() {
        let generation = unsafe { &*current };
        if generation.vtable == vtable as usize {
            return Some(generation);
        }
        current = generation.next;
    }
    None
}

fn shadow_replacement(kind: ShadowKind, index: usize) -> Option<*mut c_void> {
    match (kind, index) {
        (ShadowKind::Factory, FACTORY_QUERY_INTERFACE_INDEX) => {
            Some(factory_query_interface_hook as *const () as *mut c_void)
        }
        (ShadowKind::Factory, UNKNOWN_RELEASE_INDEX) => {
            Some(factory_release_hook as *const () as *mut c_void)
        }
        (ShadowKind::Factory, FACTORY_CREATE_SWAP_CHAIN_INDEX) => {
            Some(create_swap_chain_hook as *const () as *mut c_void)
        }
        (ShadowKind::Factory, FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX) => {
            Some(create_swap_chain_for_hwnd_hook as *const () as *mut c_void)
        }
        (ShadowKind::Factory, FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX) => {
            Some(create_swap_chain_for_core_window_hook as *const () as *mut c_void)
        }
        (ShadowKind::Factory, FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX) => {
            Some(create_swap_chain_for_composition_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_QUERY_INTERFACE_INDEX) => {
            Some(swap_chain_query_interface_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, UNKNOWN_RELEASE_INDEX) => {
            Some(swap_chain_release_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_PRESENT_INDEX) => {
            Some(present_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_SET_FULLSCREEN_STATE_INDEX) => {
            Some(set_fullscreen_state_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_RESIZE_BUFFERS_INDEX) => {
            Some(resize_buffers_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_RESIZE_TARGET_INDEX) => {
            Some(resize_target_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_PRESENT1_INDEX) => {
            Some(present1_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_CHECK_COLOR_SPACE_SUPPORT_INDEX) => {
            Some(check_color_space_support_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_SET_COLOR_SPACE1_INDEX) => {
            Some(set_color_space1_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_RESIZE_BUFFERS1_INDEX) => {
            Some(resize_buffers1_hook as *const () as *mut c_void)
        }
        (ShadowKind::SwapChain, SWAP_CHAIN_SET_HDR_METADATA_INDEX) => {
            Some(set_hdr_metadata_hook as *const () as *mut c_void)
        }
        _ => None,
    }
}

unsafe fn load_saved_record_function<T: Copy>(
    record: *mut ShadowRecord,
    index: usize,
) -> Option<T> {
    let record = unsafe { &*record };
    if index >= record.entry_count.load(Ordering::Acquire) {
        return None;
    }
    let originals = record
        .originals
        .load(Ordering::Acquire)
        .cast::<*mut c_void>();
    if originals.is_null() {
        return None;
    }
    let saved = unsafe { *originals.add(index) };
    (!saved.is_null()).then(|| unsafe { mem::transmute_copy::<*mut c_void, T>(&saved) })
}

unsafe fn load_record_function<T: Copy>(record: *mut ShadowRecord, index: usize) -> Option<T> {
    let record_ref = unsafe { &*record };
    let saved_pointer = unsafe { load_saved_record_function::<*mut c_void>(record, index)? };
    let pointer = if record_ref.live_root_slots.load(Ordering::Acquire) {
        let root = record_ref.root_original_vtable as *mut *mut c_void;
        // A vtable-patching overlay such as GG may update the shared DXGI table
        // after this object's shadow was installed. Follow that live external
        // chain, while refusing to call back into our own replacement.
        let current_root = unsafe { ptr::read_volatile(root.add(index)) };
        if !current_root.is_null()
            && shadow_replacement(record_ref.kind, index) != Some(current_root)
        {
            current_root
        } else {
            saved_pointer
        }
    } else {
        saved_pointer
    };
    (!pointer.is_null()).then(|| unsafe { mem::transmute_copy::<*mut c_void, T>(&pointer) })
}

unsafe fn load_object_function<T: Copy>(
    object: *mut c_void,
    kind: ShadowKind,
    index: usize,
) -> Option<T> {
    let record = unsafe { find_shadow_record(object, kind)? };
    unsafe { load_record_function(record, index) }
}

unsafe fn object_vtable(object: *mut c_void) -> Result<*mut *mut c_void, String> {
    if object.is_null() {
        return Err("COM object is null".to_owned());
    }
    let vtable = unsafe { *object.cast::<*mut *mut c_void>() };
    if vtable.is_null() {
        Err("COM vtable is null".to_owned())
    } else {
        Ok(vtable)
    }
}

unsafe fn load_function<T: Copy>(slot: &AtomicPtr<c_void>) -> Option<T> {
    let pointer = slot.load(Ordering::Acquire);
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { mem::transmute_copy::<*mut c_void, T>(&pointer) })
    }
}

fn factory_vtable_len(riid: &Guid) -> Option<usize> {
    match *riid {
        IID_IDXGI_FACTORY => Some(FACTORY_VTABLE_LEN),
        IID_IDXGI_FACTORY1 => Some(FACTORY1_VTABLE_LEN),
        IID_IDXGI_FACTORY2 => Some(FACTORY2_VTABLE_LEN),
        IID_IDXGI_FACTORY3 => Some(FACTORY3_VTABLE_LEN),
        IID_IDXGI_FACTORY4 => Some(FACTORY4_VTABLE_LEN),
        IID_IDXGI_FACTORY5 => Some(FACTORY5_VTABLE_LEN),
        IID_IDXGI_FACTORY6 => Some(FACTORY6_VTABLE_LEN),
        IID_IDXGI_FACTORY7 => Some(FACTORY7_VTABLE_LEN),
        _ => None,
    }
}

fn swap_chain_vtable_len(riid: &Guid) -> Option<usize> {
    match *riid {
        IID_IDXGI_SWAP_CHAIN => Some(SWAP_CHAIN_VTABLE_LEN),
        IID_IDXGI_SWAP_CHAIN1 => Some(SWAP_CHAIN1_VTABLE_LEN),
        IID_IDXGI_SWAP_CHAIN2 => Some(SWAP_CHAIN2_VTABLE_LEN),
        IID_IDXGI_SWAP_CHAIN3 => Some(SWAP_CHAIN3_VTABLE_LEN),
        IID_IDXGI_SWAP_CHAIN4 => Some(SWAP_CHAIN4_VTABLE_LEN),
        _ => None,
    }
}

fn log_once(flag: &AtomicBool, message: &str) {
    if flag
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log(message);
    }
}

fn log(message: impl AsRef<str>) {
    if let Some(logger) = LOGGER.get() {
        logger.line(message);
    }
}

struct Import {
    name: String,
    slot: *mut *mut c_void,
    current: *mut c_void,
}

struct Image {
    base: *mut u8,
    size: usize,
    import_rva: usize,
    import_size: usize,
}

impl Image {
    unsafe fn main_module() -> Result<Self, String> {
        let base = unsafe { windows::main_module()? }.cast::<u8>();
        if unsafe { read_u16(base, 0) } != 0x5a4d {
            return Err("main module has no MZ signature".to_owned());
        }
        let nt = unsafe { read_u32(base, 0x3c) } as usize;
        if unsafe { read_u32(base, nt) } != 0x0000_4550 {
            return Err("main module has no PE signature".to_owned());
        }
        let optional = nt + 24;
        if unsafe { read_u16(base, optional) } != 0x20b {
            return Err("main module is not PE32+".to_owned());
        }
        let size = unsafe { read_u32(base, optional + 56) } as usize;
        let directory = optional + 112 + IMAGE_DIRECTORY_ENTRY_IMPORT * 8;
        let import_rva = unsafe { read_u32(base, directory) } as usize;
        let import_size = unsafe { read_u32(base, directory + 4) } as usize;
        if size == 0 || import_rva == 0 || import_size < 20 {
            return Err("main module has no usable import directory".to_owned());
        }
        if import_rva
            .checked_add(import_size)
            .is_none_or(|end| end > size)
        {
            return Err("main module import directory is outside the image".to_owned());
        }
        Ok(Self {
            base,
            size,
            import_rva,
            import_size,
        })
    }

    unsafe fn imports(&self, requested_module: &str) -> Result<Vec<Import>, String> {
        let mut result = Vec::new();
        let descriptor_count = self.import_size / 20;
        for index in 0..descriptor_count {
            let descriptor = self.import_rva + index * 20;
            let original_first_thunk = unsafe { self.u32_at(descriptor)? } as usize;
            let name_rva = unsafe { self.u32_at(descriptor + 12)? } as usize;
            let first_thunk = unsafe { self.u32_at(descriptor + 16)? } as usize;
            if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }
            if name_rva == 0 || first_thunk == 0 {
                continue;
            }
            let module = unsafe { self.string_at(name_rva, 128)? };
            if !module.eq_ignore_ascii_case(requested_module) {
                continue;
            }
            if original_first_thunk == 0 {
                return Err(format!(
                    "{requested_module} import has no original thunk table"
                ));
            }

            for thunk_index in 0..4096usize {
                let offset = thunk_index * mem::size_of::<u64>();
                let name_thunk = unsafe { self.u64_at(original_first_thunk + offset)? };
                if name_thunk == 0 {
                    break;
                }
                if name_thunk & IMAGE_ORDINAL_FLAG64 != 0 {
                    continue;
                }
                let name = unsafe { self.string_at(name_thunk as usize + 2, 256)? };
                let slot_rva = first_thunk
                    .checked_add(offset)
                    .ok_or_else(|| "IAT slot overflow".to_owned())?;
                let slot = unsafe { self.pointer_at(slot_rva)? };
                result.push(Import {
                    name,
                    slot,
                    current: unsafe { *slot },
                });
            }
        }
        Ok(result)
    }

    unsafe fn u32_at(&self, rva: usize) -> Result<u32, String> {
        self.range(rva, 4)?;
        Ok(unsafe { read_u32(self.base, rva) })
    }

    unsafe fn u64_at(&self, rva: usize) -> Result<u64, String> {
        self.range(rva, 8)?;
        Ok(unsafe { ptr::read_unaligned(self.base.add(rva).cast::<u64>()) })
    }

    unsafe fn pointer_at(&self, rva: usize) -> Result<*mut *mut c_void, String> {
        self.range(rva, mem::size_of::<usize>())?;
        Ok(unsafe { self.base.add(rva).cast() })
    }

    unsafe fn string_at(&self, rva: usize, maximum: usize) -> Result<String, String> {
        self.range(rva, 1)?;
        let available = (self.size - rva).min(maximum);
        let bytes = unsafe { std::slice::from_raw_parts(self.base.add(rva), available) };
        let length = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| "unterminated import string".to_owned())?;
        let name = &bytes[..length];
        if !name.is_ascii() {
            return Err("non-ASCII import string".to_owned());
        }
        Ok(unsafe { std::str::from_utf8_unchecked(name) }.to_owned())
    }

    fn range(&self, rva: usize, length: usize) -> Result<(), String> {
        if rva.checked_add(length).is_some_and(|end| end <= self.size) {
            Ok(())
        } else {
            Err("import table reference is outside the image".to_owned())
        }
    }
}

unsafe fn read_u16(base: *mut u8, offset: usize) -> u16 {
    unsafe { ptr::read_unaligned(base.add(offset).cast::<u16>()) }
}

unsafe fn read_u32(base: *mut u8, offset: usize) -> u32 {
    unsafe { ptr::read_unaligned(base.add(offset).cast::<u32>()) }
}

// Adapted from the UnlockTheFps reference project to keep its object-local vtable and
// Overlay-chain guarantees covered while this project changes the hooked DXGI methods.
#[cfg(test)]
mod tests {
    use super::*;

    #[link(name = "dxgi")]
    unsafe extern "system" {
        #[link_name = "CreateDXGIFactory1"]
        fn system_create_dxgi_factory1(riid: *const Guid, factory: *mut *mut c_void) -> HResult;
    }

    static TEST_CHAIN_LOCK: Mutex<()> = Mutex::new(());
    static TEST_UNLOCK_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_EXTERNAL_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_BASE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_BASE_A_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_BASE_B_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_RELEASE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_EXTERNAL_NEXT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    const TEST_REGION_SIZE: usize = 64 * 1024;
    const TEST_VTABLE_OFFSET: usize = 4096 + mem::size_of::<*mut c_void>();
    const TEST_VTABLE_CAPACITY: usize = 512;

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn fake_pointer(tag: usize) -> *mut c_void {
        (0x10_000usize + tag * 0x10) as *mut c_void
    }

    fn leak_vtable(entry_count: usize) -> *mut *mut c_void {
        assert!(entry_count <= TEST_VTABLE_CAPACITY);
        leak_vtable_at_offset(TEST_VTABLE_OFFSET, TEST_VTABLE_CAPACITY)
    }

    fn leak_vtable_at_offset(offset: usize, entry_capacity: usize) -> *mut *mut c_void {
        assert_eq!(offset % mem::align_of::<*mut c_void>(), 0);
        assert!(offset + entry_capacity * mem::size_of::<*mut c_void>() <= TEST_REGION_SIZE);
        let region = unsafe { windows::allocate_read_write(TEST_REGION_SIZE) }.unwrap();
        let vtable = unsafe { region.add(offset) }.cast::<*mut c_void>();
        for index in 0..entry_capacity {
            unsafe { *vtable.add(index) = fake_pointer(index + 1) };
        }
        vtable
    }

    fn leak_object(vtable: *mut *mut c_void) -> *mut c_void {
        Box::into_raw(Box::new(vtable)).cast()
    }

    fn present_pointer(function: Present) -> *mut c_void {
        function as *const () as *mut c_void
    }

    fn present_hook() -> [VtableHook; 1] {
        [VtableHook {
            index: SWAP_CHAIN_PRESENT_INDEX,
            replacement: present_pointer(test_unlock_present),
            name: "test Present",
        }]
    }

    fn reset_test_call_counts() {
        TEST_UNLOCK_CALLS.store(0, Ordering::Release);
        TEST_EXTERNAL_CALLS.store(0, Ordering::Release);
        TEST_BASE_CALLS.store(0, Ordering::Release);
        TEST_BASE_A_CALLS.store(0, Ordering::Release);
        TEST_BASE_B_CALLS.store(0, Ordering::Release);
        TEST_RELEASE_CALLS.store(0, Ordering::Release);
        TEST_EXTERNAL_NEXT.store(ptr::null_mut(), Ordering::Release);
    }

    unsafe fn call_present(object: *mut c_void) -> HResult {
        let vtable = unsafe { object_vtable(object) }.unwrap();
        let function: Present = unsafe { mem::transmute(*vtable.add(SWAP_CHAIN_PRESENT_INDEX)) };
        unsafe { function(object, 0, 0) }
    }

    unsafe extern "system" fn test_unlock_present(
        this: *mut c_void,
        sync_interval: u32,
        flags: u32,
    ) -> HResult {
        TEST_UNLOCK_CALLS.fetch_add(1, Ordering::AcqRel);
        let next = unsafe {
            load_object_function::<Present>(this, ShadowKind::SwapChain, SWAP_CHAIN_PRESENT_INDEX)
        }
        .expect("test shadow must retain its original Present");
        unsafe { next(this, sync_interval, flags) }
    }

    unsafe extern "system" fn test_base_present(
        _this: *mut c_void,
        _sync_interval: u32,
        _flags: u32,
    ) -> HResult {
        TEST_BASE_CALLS.fetch_add(1, Ordering::AcqRel);
        S_OK_MINIMUM
    }

    unsafe extern "system" fn test_base_a_present(
        _this: *mut c_void,
        _sync_interval: u32,
        _flags: u32,
    ) -> HResult {
        TEST_BASE_A_CALLS.fetch_add(1, Ordering::AcqRel);
        S_OK_MINIMUM
    }

    unsafe extern "system" fn test_base_b_present(
        _this: *mut c_void,
        _sync_interval: u32,
        _flags: u32,
    ) -> HResult {
        TEST_BASE_B_CALLS.fetch_add(1, Ordering::AcqRel);
        S_OK_MINIMUM
    }

    unsafe extern "system" fn test_external_before_present(
        this: *mut c_void,
        sync_interval: u32,
        flags: u32,
    ) -> HResult {
        TEST_EXTERNAL_CALLS.fetch_add(1, Ordering::AcqRel);
        unsafe { test_base_present(this, sync_interval, flags) }
    }

    unsafe extern "system" fn test_external_after_present(
        this: *mut c_void,
        sync_interval: u32,
        flags: u32,
    ) -> HResult {
        TEST_EXTERNAL_CALLS.fetch_add(1, Ordering::AcqRel);
        let next = TEST_EXTERNAL_NEXT.load(Ordering::Acquire);
        assert!(!next.is_null());
        let next: Present = unsafe { mem::transmute(next) };
        unsafe { next(this, sync_interval, flags) }
    }

    unsafe extern "system" fn test_release_zero(_this: *mut c_void) -> u32 {
        TEST_RELEASE_CALLS.fetch_add(1, Ordering::AcqRel);
        0
    }

    #[test]
    fn dxgi_struct_layout_matches_the_windows_abi() {
        assert_eq!(mem::size_of::<Rational>(), 8);
        assert_eq!(mem::size_of::<ModeDesc>(), 28);
        assert_eq!(mem::size_of::<SwapChainDesc>(), 72);
        assert_eq!(mem::offset_of!(SwapChainDesc, output_window), 48);
        assert_eq!(mem::offset_of!(SwapChainDesc, windowed), 56);
    }

    #[test]
    fn real_dxgi_factory_survives_shadowing_and_final_release() {
        let mut factory = ptr::null_mut();
        let result = unsafe { system_create_dxgi_factory1(&IID_IDXGI_FACTORY6, &mut factory) };
        assert!(result >= S_OK_MINIMUM);
        assert!(!factory.is_null());

        let root = unsafe { object_vtable(factory) }.unwrap();
        unsafe { patch_factory(factory, FACTORY6_VTABLE_LEN) }.unwrap();
        let shadow = unsafe { object_vtable(factory) }.unwrap();
        assert_ne!(shadow, root);

        let release: Release = unsafe { mem::transmute(*shadow.add(UNKNOWN_RELEASE_INDEX)) };
        assert_eq!(unsafe { release(factory) }, 0);
    }

    #[test]
    fn locates_named_import_slots_in_a_loaded_image() {
        let mut bytes = vec![0u8; 0x800];
        put_u32(&mut bytes, 0x200, 0x300);
        put_u32(&mut bytes, 0x20c, 0x280);
        put_u32(&mut bytes, 0x210, 0x400);
        bytes[0x280..0x289].copy_from_slice(b"dxgi.dll\0");
        put_u64(&mut bytes, 0x300, 0x500);
        bytes[0x502..0x515].copy_from_slice(b"CreateDXGIFactory1\0");
        put_u64(&mut bytes, 0x400, 0x1234_5678);

        let image = Image {
            base: bytes.as_mut_ptr(),
            size: bytes.len(),
            import_rva: 0x200,
            import_size: 40,
        };
        let imports = unsafe { image.imports("DXGI.DLL") }.unwrap();

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "CreateDXGIFactory1");
        assert_eq!(imports[0].current as usize, 0x1234_5678);
        assert_eq!(imports[0].slot as usize, bytes.as_ptr() as usize + 0x400);
    }

    #[test]
    fn maps_all_dxgi_iids_to_exact_vtable_lengths() {
        let factory_interfaces = [
            (&IID_IDXGI_FACTORY, FACTORY_VTABLE_LEN),
            (&IID_IDXGI_FACTORY1, FACTORY1_VTABLE_LEN),
            (&IID_IDXGI_FACTORY2, FACTORY2_VTABLE_LEN),
            (&IID_IDXGI_FACTORY3, FACTORY3_VTABLE_LEN),
            (&IID_IDXGI_FACTORY4, FACTORY4_VTABLE_LEN),
            (&IID_IDXGI_FACTORY5, FACTORY5_VTABLE_LEN),
            (&IID_IDXGI_FACTORY6, FACTORY6_VTABLE_LEN),
            (&IID_IDXGI_FACTORY7, FACTORY7_VTABLE_LEN),
        ];
        for (iid, expected) in factory_interfaces {
            assert_eq!(factory_vtable_len(iid), Some(expected));
        }

        let swap_chain_interfaces = [
            (&IID_IDXGI_SWAP_CHAIN, SWAP_CHAIN_VTABLE_LEN),
            (&IID_IDXGI_SWAP_CHAIN1, SWAP_CHAIN1_VTABLE_LEN),
            (&IID_IDXGI_SWAP_CHAIN2, SWAP_CHAIN2_VTABLE_LEN),
            (&IID_IDXGI_SWAP_CHAIN3, SWAP_CHAIN3_VTABLE_LEN),
            (&IID_IDXGI_SWAP_CHAIN4, SWAP_CHAIN4_VTABLE_LEN),
        ];
        for (iid, expected) in swap_chain_interfaces {
            assert_eq!(swap_chain_vtable_len(iid), Some(expected));
        }

        let unknown = Guid {
            data1: 0,
            data2: 0,
            data3: 0,
            data4: [0; 8],
        };
        assert_eq!(factory_vtable_len(&unknown), None);
        assert_eq!(swap_chain_vtable_len(&unknown), None);
    }

    #[test]
    fn shadows_only_the_target_object() {
        let root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        let original_present = unsafe { *root.add(SWAP_CHAIN_PRESENT_INDEX) };
        let first = leak_object(root);
        let second = leak_object(root);

        unsafe {
            install_shadow_vtable(
                first,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();

        let first_vtable = unsafe { object_vtable(first) }.unwrap();
        let second_vtable = unsafe { object_vtable(second) }.unwrap();
        assert_ne!(first_vtable, root);
        assert_eq!(second_vtable, root);
        assert_eq!(
            unsafe { *root.add(SWAP_CHAIN_PRESENT_INDEX) },
            original_present
        );
        assert_eq!(
            unsafe { *first_vtable.add(SWAP_CHAIN_PRESENT_INDEX) },
            present_pointer(test_unlock_present)
        );
    }

    #[test]
    fn chains_external_hook_installed_before_shadow() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_external_before_present);
        }
        let object = leak_object(root);

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);

        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_EXTERNAL_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_BASE_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn chains_external_hook_installed_after_shadow() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_present);
        }
        let object = leak_object(root);

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let shadow = unsafe { object_vtable(object) }.unwrap();
        let unlock = unsafe { *shadow.add(SWAP_CHAIN_PRESENT_INDEX) };
        TEST_EXTERNAL_NEXT.store(unlock, Ordering::Release);
        unsafe {
            windows::write_pointer(
                shadow.add(SWAP_CHAIN_PRESENT_INDEX),
                present_pointer(test_external_after_present),
            )
        }
        .unwrap();

        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);
        assert_eq!(TEST_EXTERNAL_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_BASE_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn follows_late_root_hooks_only_for_a_known_persistent_root() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_present);
        }
        let object = leak_object(root);

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let shadow = unsafe { object_vtable(object) }.unwrap();
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_external_before_present);
        }

        let record = unsafe { find_shadow_record(object, ShadowKind::SwapChain) }.unwrap();
        assert!(!unsafe { &*record }.live_root_slots.load(Ordering::Acquire));
        assert_eq!(
            unsafe { *shadow.add(SWAP_CHAIN_PRESENT_INDEX) },
            present_pointer(test_unlock_present)
        );
        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);
        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_EXTERNAL_CALLS.load(Ordering::Acquire), 0);
        assert_eq!(TEST_BASE_CALLS.load(Ordering::Acquire), 1);

        reset_test_call_counts();
        unsafe { &*record }
            .live_root_slots
            .store(true, Ordering::Release);
        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);
        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_EXTERNAL_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_BASE_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn expansion_preserves_an_outer_in_place_hook() {
        let root = leak_vtable(SWAP_CHAIN4_VTABLE_LEN);
        let root_present = unsafe { *root.add(SWAP_CHAIN_PRESENT_INDEX) };
        let root_last_swap_chain3 = unsafe { *root.add(SWAP_CHAIN3_VTABLE_LEN - 1) };
        let object = leak_object(root);

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let short_shadow = unsafe { object_vtable(object) }.unwrap();
        let outer = present_pointer(test_external_after_present);
        unsafe { windows::write_pointer(short_shadow.add(SWAP_CHAIN_PRESENT_INDEX), outer) }
            .unwrap();

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN3_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let expanded = unsafe { object_vtable(object) }.unwrap();
        assert_eq!(expanded, short_shadow);
        assert_eq!(unsafe { *expanded.add(SWAP_CHAIN_PRESENT_INDEX) }, outer);
        assert_eq!(
            unsafe { *expanded.add(SWAP_CHAIN3_VTABLE_LEN - 1) },
            root_last_swap_chain3
        );
        assert_eq!(
            unsafe {
                load_object_function::<*mut c_void>(
                    object,
                    ShadowKind::SwapChain,
                    SWAP_CHAIN_PRESENT_INDEX,
                )
            },
            Some(root_present)
        );

        unsafe {
            *object.cast::<*mut *mut c_void>() = short_shadow;
        }
        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN4_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let expanded_from_restored_short = unsafe { object_vtable(object) }.unwrap();
        assert_eq!(
            unsafe { *expanded_from_restored_short.add(SWAP_CHAIN_PRESENT_INDEX) },
            outer
        );
        assert_eq!(
            unsafe { *expanded_from_restored_short.add(SWAP_CHAIN4_VTABLE_LEN - 1) },
            unsafe { *root.add(SWAP_CHAIN4_VTABLE_LEN - 1) }
        );
    }

    #[test]
    fn factory_expansion_installs_hooks_beyond_the_short_interface() {
        let root = leak_vtable(FACTORY4_VTABLE_LEN);
        let root_entries =
            unsafe { std::slice::from_raw_parts(root, FACTORY4_VTABLE_LEN) }.to_vec();
        let object = leak_object(root);
        let query_interface = fake_pointer(101);
        let create_swap_chain = fake_pointer(102);
        let create_for_hwnd = fake_pointer(103);
        let create_for_core_window = fake_pointer(104);
        let create_for_composition = fake_pointer(105);
        let factory1_hooks = [
            VtableHook {
                index: FACTORY_QUERY_INTERFACE_INDEX,
                replacement: query_interface,
                name: "test factory QueryInterface",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_INDEX,
                replacement: create_swap_chain,
                name: "test factory CreateSwapChain",
            },
        ];
        let factory4_hooks = [
            VtableHook {
                index: FACTORY_QUERY_INTERFACE_INDEX,
                replacement: query_interface,
                name: "test factory QueryInterface",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_INDEX,
                replacement: create_swap_chain,
                name: "test factory CreateSwapChain",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX,
                replacement: create_for_hwnd,
                name: "test factory CreateSwapChainForHwnd",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX,
                replacement: create_for_core_window,
                name: "test factory CreateSwapChainForCoreWindow",
            },
            VtableHook {
                index: FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX,
                replacement: create_for_composition,
                name: "test factory CreateSwapChainForComposition",
            },
        ];

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::Factory,
                FACTORY1_VTABLE_LEN,
                &factory1_hooks,
            )
        }
        .unwrap();
        let short_shadow = unsafe { object_vtable(object) }.unwrap();

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::Factory,
                FACTORY4_VTABLE_LEN,
                &factory4_hooks,
            )
        }
        .unwrap();
        let expanded = unsafe { object_vtable(object) }.unwrap();

        assert_eq!(expanded, short_shadow);
        for (index, expected) in [
            (FACTORY_QUERY_INTERFACE_INDEX, query_interface),
            (FACTORY_CREATE_SWAP_CHAIN_INDEX, create_swap_chain),
            (FACTORY_CREATE_SWAP_CHAIN_FOR_HWND_INDEX, create_for_hwnd),
            (
                FACTORY_CREATE_SWAP_CHAIN_FOR_CORE_WINDOW_INDEX,
                create_for_core_window,
            ),
            (
                FACTORY_CREATE_SWAP_CHAIN_FOR_COMPOSITION_INDEX,
                create_for_composition,
            ),
        ] {
            assert_eq!(unsafe { *expanded.add(index) }, expected);
        }
        assert_eq!(
            unsafe { std::slice::from_raw_parts(root, FACTORY4_VTABLE_LEN) },
            root_entries
        );
    }

    #[test]
    fn supports_a_vtable_at_the_start_of_its_memory_region() {
        let root = leak_vtable_at_offset(0, SWAP_CHAIN4_VTABLE_LEN);
        let root_last_entry = unsafe { *root.add(SWAP_CHAIN4_VTABLE_LEN - 1) };
        let object = leak_object(root);

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();

        let shadow = unsafe { object_vtable(object) }.unwrap();
        assert_ne!(shadow, root);
        assert_eq!(
            unsafe { *shadow.add(SWAP_CHAIN_PRESENT_INDEX) },
            present_pointer(test_unlock_present)
        );

        let record = unsafe { find_shadow_record(object, ShadowKind::SwapChain) }.unwrap();
        let generation = unsafe { &*record }.generations.load(Ordering::Acquire);
        assert!(!generation.is_null());
        let prefix_size = shadow as usize - unsafe { &*generation }.region_base;
        assert!(prefix_size >= mem::size_of::<*mut c_void>());
        assert!(unsafe { *shadow.sub(1) }.is_null());

        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN4_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        assert_eq!(unsafe { object_vtable(object) }.unwrap(), shadow);
        assert_eq!(
            unsafe { *shadow.add(SWAP_CHAIN4_VTABLE_LEN - 1) },
            root_last_entry
        );
    }

    #[test]
    fn rejects_expansion_beyond_the_cloned_region_capacity() {
        let offset = TEST_REGION_SIZE - SWAP_CHAIN_VTABLE_LEN * mem::size_of::<*mut c_void>();
        let root = leak_vtable_at_offset(offset, SWAP_CHAIN_VTABLE_LEN);
        let object = leak_object(root);
        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let short_shadow = unsafe { object_vtable(object) }.unwrap();

        let error = unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN4_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap_err();

        assert!(error.contains("only has room"));
        assert_eq!(unsafe { object_vtable(object) }.unwrap(), short_shadow);
        let record = unsafe { find_shadow_record(object, ShadowKind::SwapChain) }.unwrap();
        assert_eq!(
            unsafe { &*record }.entry_count.load(Ordering::Acquire),
            SWAP_CHAIN_VTABLE_LEN
        );
    }

    #[test]
    fn dispatches_the_original_function_per_object() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let first_root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        let second_root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *first_root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_a_present);
            *second_root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_b_present);
        }
        let first = leak_object(first_root);
        let second = leak_object(second_root);

        unsafe {
            install_shadow_vtable(
                first,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        unsafe {
            install_shadow_vtable(
                second,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();

        assert_eq!(unsafe { call_present(first) }, S_OK_MINIMUM);
        assert_eq!(unsafe { call_present(second) }, S_OK_MINIMUM);
        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 2);
        assert_eq!(TEST_BASE_A_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_BASE_B_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn retires_a_record_before_the_same_object_address_is_reused() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let first_root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *first_root.add(UNKNOWN_RELEASE_INDEX) = test_release_zero as *const () as *mut c_void;
            *first_root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_a_present);
        }
        let object = leak_object(first_root);
        let hooks = [
            VtableHook {
                index: UNKNOWN_RELEASE_INDEX,
                replacement: swap_chain_release_hook as *const () as *mut c_void,
                name: "test swap-chain Release",
            },
            VtableHook {
                index: SWAP_CHAIN_PRESENT_INDEX,
                replacement: present_pointer(test_unlock_present),
                name: "test Present",
            },
        ];

        unsafe {
            install_shadow_vtable(object, ShadowKind::SwapChain, SWAP_CHAIN_VTABLE_LEN, &hooks)
        }
        .unwrap();
        let first_shadow = unsafe { object_vtable(object) }.unwrap();
        let release: Release = unsafe { mem::transmute(*first_shadow.add(UNKNOWN_RELEASE_INDEX)) };
        assert_eq!(unsafe { release(object) }, 0);
        assert_eq!(TEST_RELEASE_CALLS.load(Ordering::Acquire), 1);

        let second_root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *second_root.add(UNKNOWN_RELEASE_INDEX) = test_release_zero as *const () as *mut c_void;
            *second_root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_b_present);
            *object.cast::<*mut *mut c_void>() = second_root;
        }
        unsafe {
            install_shadow_vtable(object, ShadowKind::SwapChain, SWAP_CHAIN_VTABLE_LEN, &hooks)
        }
        .unwrap();

        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);
        assert_eq!(TEST_BASE_A_CALLS.load(Ordering::Acquire), 0);
        assert_eq!(TEST_BASE_B_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn leaves_a_foreign_outer_vtable_in_control_without_recursive_rewrapping() {
        let _guard = TEST_CHAIN_LOCK.lock().unwrap();
        reset_test_call_counts();
        let root = leak_vtable(SWAP_CHAIN4_VTABLE_LEN);
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_base_present);
        }
        let object = leak_object(root);
        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();
        let shadow = unsafe { object_vtable(object) }.unwrap();
        let unlock = unsafe { *shadow.add(SWAP_CHAIN_PRESENT_INDEX) };

        let foreign = leak_vtable(SWAP_CHAIN4_VTABLE_LEN);
        unsafe {
            *foreign.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_external_after_present);
            *object.cast::<*mut *mut c_void>() = foreign;
        }
        TEST_EXTERNAL_NEXT.store(unlock, Ordering::Release);
        unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN4_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap();

        assert_eq!(unsafe { object_vtable(object) }.unwrap(), foreign);
        assert_eq!(unsafe { call_present(object) }, S_OK_MINIMUM);
        assert_eq!(TEST_EXTERNAL_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_UNLOCK_CALLS.load(Ordering::Acquire), 1);
        assert_eq!(TEST_BASE_CALLS.load(Ordering::Acquire), 1);
    }

    #[test]
    fn rejects_an_original_entry_that_is_already_our_hook() {
        let root = leak_vtable(SWAP_CHAIN_VTABLE_LEN);
        unsafe {
            *root.add(SWAP_CHAIN_PRESENT_INDEX) = present_pointer(test_unlock_present);
        }
        let object = leak_object(root);

        let error = unsafe {
            install_shadow_vtable(
                object,
                ShadowKind::SwapChain,
                SWAP_CHAIN_VTABLE_LEN,
                &present_hook(),
            )
        }
        .unwrap_err();

        assert!(error.contains("already points at our replacement"));
        assert_eq!(unsafe { object_vtable(object) }.unwrap(), root);
    }
}
