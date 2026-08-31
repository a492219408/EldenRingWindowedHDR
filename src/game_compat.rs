//! Resolves every version-sensitive Elden Ring HDR target as one validated bundle.
//!
//! Full-file hashes classify known builds but never substitute for executable-section
//! signatures, control-flow relationships, MSVC RTTI/vtable checks, and memory bounds.

use std::{ffi::c_void, mem, time::Instant};

use crate::{logger::Logger, windows};

const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const MAX_IMAGE_SIZE: usize = 1024 * 1024 * 1024;
const MAX_SECTION_COUNT: usize = 96;
const MAX_RTTI_NAME_LENGTH: usize = 160;
pub(crate) const HDR_MENU_GATE_INVOKE_INDEX: usize = 2;

const AVAILABILITY_SIGNATURE: Signature = Signature {
    length: 101,
    segments: &[
        SignatureSegment {
            offset: 0,
            bytes: &[0x48, 0x83, 0xEC, 0x48, 0x48, 0x8B, 0x05],
        },
        SignatureSegment {
            offset: 11,
            bytes: &[0x48, 0x33, 0xC4, 0x48, 0x89, 0x44, 0x24, 0x30, 0xE8],
        },
        SignatureSegment {
            offset: 24,
            bytes: &[
                0x84, 0xC0, 0x74, 0x35, 0x33, 0xC0, 0x48, 0x8D, 0x4C, 0x24, 0x20, 0xB2, 0x01, 0x48,
                0x89, 0x44, 0x24, 0x20, 0x89, 0x44, 0x24, 0x28, 0xE8,
            ],
        },
        SignatureSegment {
            offset: 51,
            bytes: &[
                0x84, 0xC0, 0x75, 0x1A, 0x38, 0x44, 0x24, 0x20, 0x74, 0x14, 0xB0, 0x01, 0x48, 0x8B,
                0x4C, 0x24, 0x30, 0x48, 0x33, 0xCC, 0xE8,
            ],
        },
        SignatureSegment {
            offset: 76,
            bytes: &[
                0x48, 0x83, 0xC4, 0x48, 0xC3, 0x32, 0xC0, 0x48, 0x8B, 0x4C, 0x24, 0x30, 0x48, 0x33,
                0xCC, 0xE8,
            ],
        },
        SignatureSegment {
            offset: 96,
            bytes: &[0x48, 0x83, 0xC4, 0x48, 0xC3],
        },
    ],
};

const GRAPHICS_CONFIG_SIGNATURE: Signature = Signature {
    length: 259,
    segments: &[
        SignatureSegment {
            offset: 0,
            bytes: &[
                0x40, 0x53, 0x48, 0x83, 0xEC, 0x20, 0x0F, 0xB6, 0x02, 0x48, 0x8B, 0xDA, 0x88, 0x01,
                0x0F, 0xB6, 0x42, 0x01, 0x88, 0x41, 0x01, 0x0F, 0xB6, 0x42, 0x02, 0x88, 0x41, 0x02,
                0x0F, 0xB6, 0x42, 0x03, 0x88, 0x41, 0x03,
            ],
        },
        SignatureSegment {
            offset: 196,
            bytes: &[0x0F, 0xB6, 0x42, 0x15, 0x88, 0x41, 0x1B],
        },
        SignatureSegment {
            offset: 245,
            bytes: &[
                0x0F, 0xB6, 0x42, 0x60, 0x88, 0x41, 0x22, 0x0F, 0xB6, 0x42, 0x61, 0x88, 0x41, 0x23,
            ],
        },
    ],
};

const BACKEND_ACTUAL_SIGNATURE: Signature = Signature {
    length: 76,
    segments: &[
        SignatureSegment {
            offset: 0,
            bytes: &[
                0x40, 0x53, 0x57, 0x48, 0x83, 0xEC, 0x28, 0xF6, 0x41, 0x32, 0x04, 0x48, 0x8B, 0xFA,
                0x48, 0x8B, 0xD9, 0x75, 0x09, 0x32, 0xC0, 0x48, 0x83, 0xC4, 0x28, 0x5F, 0x5B, 0xC3,
            ],
        },
        SignatureSegment {
            offset: 63,
            bytes: &[0x48, 0x8B, 0x01, 0xFF, 0x50, 0x58],
        },
        SignatureSegment {
            offset: 69,
            bytes: &[0x85, 0xC0, 0x74, 0x0E, 0x8B, 0xC8, 0xE8],
        },
    ],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameTargets {
    pub menu_gate_vtable_rva: usize,
    pub menu_gate_complete_object_locator_rva: usize,
    pub menu_gate_invoke_rva: usize,
    pub menu_gate_entries: [usize; 6],
    pub menu_gate_invoke_bytes: [u8; 19],
    pub common_availability_rva: usize,
    pub common_availability_prologue: [u8; 14],
    pub security_cookie_rva: usize,
    pub graphics_config_apply_rva: usize,
    pub graphics_config_apply_prologue: [u8; 14],
    pub backend_actual_query_rva: usize,
    pub backend_actual_query_prologue: [u8; 14],
}

struct SignatureSegment {
    offset: usize,
    bytes: &'static [u8],
}

struct Signature {
    length: usize,
    segments: &'static [SignatureSegment],
}

impl Signature {
    fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() >= self.length
            && self.segments.iter().all(|segment| {
                segment
                    .offset
                    .checked_add(segment.bytes.len())
                    .is_some_and(|end| {
                        end <= self.length && bytes[segment.offset..end] == *segment.bytes
                    })
            })
    }
}

#[derive(Clone, Copy)]
struct KnownBuild {
    name: &'static str,
    size: u64,
    sha256: &'static str,
    expected: KnownTargets,
}

#[derive(Clone, Copy)]
struct KnownTargets {
    menu_gate_vtable_rva: usize,
    menu_gate_complete_object_locator_rva: usize,
    menu_gate_invoke_rva: usize,
    menu_gate_entries: [usize; 6],
    common_availability_rva: usize,
    security_cookie_rva: usize,
    graphics_config_apply_rva: usize,
    backend_actual_query_rva: usize,
}

const KNOWN_BUILDS: &[KnownBuild] = &[
    KnownBuild {
        name: "App Ver. 1.16.2 / file version 2.6.2.0",
        size: 86_998_096,
        sha256: "34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB",
        expected: KnownTargets {
            menu_gate_vtable_rva: 0x02B1_2248,
            menu_gate_complete_object_locator_rva: 0x0331_D7F8,
            menu_gate_invoke_rva: 0x0096_1A00,
            menu_gate_entries: [
                0x0095_E1A0,
                0x0096_3910,
                0x0096_1A00,
                0x0096_54A0,
                0x0096_0630,
                0x0096_2530,
            ],
            common_availability_rva: 0x0095_2870,
            security_cookie_rva: 0x03C5_ADB0,
            graphics_config_apply_rva: 0x0025_C7B0,
            backend_actual_query_rva: 0x01E9_D6D0,
        },
    },
    KnownBuild {
        name: "App Ver. 1.17 / file version 2.7.0.0",
        size: 87_024_720,
        sha256: "D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134",
        expected: KnownTargets {
            menu_gate_vtable_rva: 0x02B1_52C8,
            menu_gate_complete_object_locator_rva: 0x0332_0AB8,
            menu_gate_invoke_rva: 0x0096_2B30,
            menu_gate_entries: [
                0x0095_F2C0,
                0x0096_4A30,
                0x0096_2B30,
                0x0096_6620,
                0x0096_1750,
                0x0096_36B0,
            ],
            common_availability_rva: 0x0095_3A10,
            security_cookie_rva: 0x03C5_EE10,
            graphics_config_apply_rva: 0x0025_C780,
            backend_actual_query_rva: 0x01E9_F4D0,
        },
    },
];

pub(crate) unsafe fn resolve(
    logger: &Logger,
    executable_size: u64,
    executable_hash: &str,
) -> Result<GameTargets, String> {
    let started = Instant::now();
    let known_build = known_build(executable_size, executable_hash);
    match known_build {
        Some(build) => logger.line(format!(
            "COMPATIBILITY: recognized {name}; resolving all runtime targets instead of trusting fixed RVAs",
            name = build.name
        )),
        None => logger.line(format!(
            "COMPATIBILITY: unrecognized executable fingerprint (size={executable_size}, SHA-256={executable_hash}); attempting strict structural resolution"
        )),
    }

    // The Windows loader owns the main image for the process lifetime. Every
    // slice created by LoadedImage is constrained to a readable PE section.
    let image = unsafe { LoadedImage::main_module()? };
    logger.line(format!(
        "COMPATIBILITY: loaded image size={} bytes; executable scan sections={}",
        image.size,
        image.executable_section_summary()
    ));

    let availability_hits = unsafe { image.find_signature(&AVAILABILITY_SIGNATURE)? };
    logger.line(format!(
        "COMPATIBILITY: common HDR availability signature matches={} ({})",
        availability_hits.len(),
        format_rvas(&availability_hits)
    ));
    let common_availability_rva =
        require_unique("common HDR availability signature", &availability_hits)?;

    let direct_callers = unsafe { image.find_direct_calls_to(common_availability_rva)? };
    logger.line(format!(
        "COMPATIBILITY: common HDR availability direct callers={} ({})",
        direct_callers.len(),
        format_rvas(&direct_callers)
    ));
    if direct_callers.len() != 2 {
        return Err(format!(
            "common HDR availability must have exactly 2 direct callers, found {} ({})",
            direct_callers.len(),
            format_rvas(&direct_callers)
        ));
    }

    let mut gate_hits = Vec::new();
    for &call_rva in &direct_callers {
        if let Some(gate_rva) = call_rva.checked_sub(4)
            && unsafe { menu_gate_body_matches(&image, gate_rva, common_availability_rva)? }
        {
            gate_hits.push(gate_rva);
        }
    }
    logger.line(format!(
        "COMPATIBILITY: HDR menu-gate caller candidates={} ({})",
        gate_hits.len(),
        format_rvas(&gate_hits)
    ));
    let menu_gate_invoke_rva = require_unique("HDR menu-gate caller", &gate_hits)?;

    let vtable = unsafe { resolve_menu_gate_vtable(&image, menu_gate_invoke_rva)? };
    logger.line(format!(
        "COMPATIBILITY: HDR menu-gate vtable RVA=0x{vtable:08X}, COL RVA=0x{col:08X}, executable LEA references={references}, RTTI={rtti}",
        vtable = vtable.rva,
        col = vtable.complete_object_locator_rva,
        references = vtable.reference_count,
        rtti = vtable.rtti_name
    ));

    let config_hits = unsafe { image.find_signature(&GRAPHICS_CONFIG_SIGNATURE)? };
    logger.line(format!(
        "COMPATIBILITY: graphics-config apply signature matches={} ({})",
        config_hits.len(),
        format_rvas(&config_hits)
    ));
    let graphics_config_apply_rva =
        require_unique("graphics-config apply signature", &config_hits)?;

    let backend_hits = unsafe { image.find_signature(&BACKEND_ACTUAL_SIGNATURE)? };
    logger.line(format!(
        "COMPATIBILITY: HDR backend actual-state signature matches={} ({})",
        backend_hits.len(),
        format_rvas(&backend_hits)
    ));
    let backend_actual_query_rva =
        require_unique("HDR backend actual-state signature", &backend_hits)?;

    let common_availability_prologue = unsafe { image.array_at::<14>(common_availability_rva)? };
    let security_cookie_rva = unsafe { resolve_security_cookie(&image, common_availability_rva)? };
    logger.line(format!(
        "COMPATIBILITY: availability security cookie resolved to RVA 0x{security_cookie_rva:08X} in {}",
        image.section_name_for(security_cookie_rva, 8)?
    ));

    let targets = GameTargets {
        menu_gate_vtable_rva: vtable.rva,
        menu_gate_complete_object_locator_rva: vtable.complete_object_locator_rva,
        menu_gate_invoke_rva,
        menu_gate_entries: vtable.entries,
        menu_gate_invoke_bytes: unsafe { image.array_at::<19>(menu_gate_invoke_rva)? },
        common_availability_rva,
        common_availability_prologue,
        security_cookie_rva,
        graphics_config_apply_rva,
        graphics_config_apply_prologue: unsafe { image.array_at::<14>(graphics_config_apply_rva)? },
        backend_actual_query_rva,
        backend_actual_query_prologue: unsafe { image.array_at::<14>(backend_actual_query_rva)? },
    };

    if let Some(build) = known_build {
        verify_known_targets(build, &targets)?;
        logger.line(format!(
            "COMPATIBILITY: all resolved RVAs and semantic checks match the known {} profile",
            build.name
        ));
    } else {
        logger.line(
            "COMPATIBILITY WARNING: unknown executable accepted because every signature, caller relationship, RTTI/vtable check, security-cookie relocation, and unique-match requirement passed; this build is structurally compatible but has not been verified in the real game",
        );
    }

    logger.line(format!(
        "COMPATIBILITY: target bundle resolved in {} ms: availability=0x{:08X}, menu_gate=0x{:08X}, menu_vtable=0x{:08X}, config_apply=0x{:08X}, backend_actual=0x{:08X}",
        started.elapsed().as_millis(),
        targets.common_availability_rva,
        targets.menu_gate_invoke_rva,
        targets.menu_gate_vtable_rva,
        targets.graphics_config_apply_rva,
        targets.backend_actual_query_rva
    ));
    Ok(targets)
}

fn known_build(size: u64, hash: &str) -> Option<&'static KnownBuild> {
    KNOWN_BUILDS
        .iter()
        .find(|build| build.size == size && build.sha256.eq_ignore_ascii_case(hash))
}

fn verify_known_targets(build: &KnownBuild, actual: &GameTargets) -> Result<(), String> {
    let expected = build.expected;
    let checks = [
        (
            "menu vtable",
            expected.menu_gate_vtable_rva,
            actual.menu_gate_vtable_rva,
        ),
        (
            "menu COL",
            expected.menu_gate_complete_object_locator_rva,
            actual.menu_gate_complete_object_locator_rva,
        ),
        (
            "menu invoke",
            expected.menu_gate_invoke_rva,
            actual.menu_gate_invoke_rva,
        ),
        (
            "common availability",
            expected.common_availability_rva,
            actual.common_availability_rva,
        ),
        (
            "security cookie",
            expected.security_cookie_rva,
            actual.security_cookie_rva,
        ),
        (
            "graphics-config apply",
            expected.graphics_config_apply_rva,
            actual.graphics_config_apply_rva,
        ),
        (
            "backend actual query",
            expected.backend_actual_query_rva,
            actual.backend_actual_query_rva,
        ),
    ];
    for (label, expected_rva, actual_rva) in checks {
        if expected_rva != actual_rva {
            return Err(format!(
                "known {} profile resolved an unexpected {label} RVA: expected 0x{expected_rva:08X}, found 0x{actual_rva:08X}",
                build.name
            ));
        }
    }
    if expected.menu_gate_entries != actual.menu_gate_entries {
        return Err(format!(
            "known {} profile resolved unexpected HDR menu-gate vtable entries: expected {}, found {}",
            build.name,
            format_rvas(&expected.menu_gate_entries),
            format_rvas(&actual.menu_gate_entries)
        ));
    }
    Ok(())
}

fn require_unique(label: &str, matches: &[usize]) -> Result<usize, String> {
    match matches {
        [single] => Ok(*single),
        _ => Err(format!(
            "{label} must match exactly once in executable PE sections, found {} ({})",
            matches.len(),
            format_rvas(matches)
        )),
    }
}

unsafe fn menu_gate_body_matches(
    image: &LoadedImage,
    gate_rva: usize,
    availability_rva: usize,
) -> Result<bool, String> {
    let body = unsafe { image.bytes_at(gate_rva, 19)? };
    if body[..4] != [0x48, 0x83, 0xEC, 0x28]
        || body[4] != 0xE8
        || body[9..] != [0x84, 0xC0, 0x0F, 0x94, 0xC0, 0x48, 0x83, 0xC4, 0x28, 0xC3]
    {
        return Ok(false);
    }
    let displacement = i32::from_le_bytes(body[5..9].try_into().expect("fixed call range"));
    Ok(relative_target(gate_rva + 9, displacement) == Some(availability_rva))
}

unsafe fn resolve_security_cookie(
    image: &LoadedImage,
    availability_rva: usize,
) -> Result<usize, String> {
    let prologue = unsafe { image.bytes_at(availability_rva, 14)? };
    if prologue[..7] != [0x48, 0x83, 0xEC, 0x48, 0x48, 0x8B, 0x05]
        || prologue[11..] != [0x48, 0x33, 0xC4]
    {
        return Err(
            "common HDR availability no longer begins with the verified security-cookie prologue"
                .to_owned(),
        );
    }
    let displacement = i32::from_le_bytes(
        prologue[7..11]
            .try_into()
            .expect("fixed security-cookie displacement range"),
    );
    let cookie_rva = relative_target(availability_rva + 11, displacement)
        .ok_or_else(|| "common HDR availability security-cookie relocation overflow".to_owned())?;
    if !cookie_rva.is_multiple_of(mem::align_of::<usize>())
        || !image.is_readable_non_executable(cookie_rva, mem::size_of::<usize>())
    {
        return Err(format!(
            "common HDR availability resolved an invalid security-cookie RVA 0x{cookie_rva:08X}"
        ));
    }
    Ok(cookie_rva)
}

struct VtableMatch {
    rva: usize,
    complete_object_locator_rva: usize,
    entries: [usize; 6],
    reference_count: usize,
    rtti_name: String,
}

unsafe fn resolve_menu_gate_vtable(
    image: &LoadedImage,
    gate_rva: usize,
) -> Result<VtableMatch, String> {
    let mut candidates: Vec<VtableMatch> = Vec::new();
    for section in image.executable_sections() {
        let bytes = unsafe { image.section_bytes(section)? };
        if bytes.len() < 7 {
            continue;
        }
        for offset in 0..=bytes.len() - 7 {
            let rex = bytes[offset];
            let mod_rm = bytes[offset + 2];
            if rex & 0xF8 != 0x48 || bytes[offset + 1] != 0x8D || mod_rm & 0xC7 != 0x05 {
                continue;
            }
            let displacement = i32::from_le_bytes(
                bytes[offset + 3..offset + 7]
                    .try_into()
                    .expect("fixed LEA displacement range"),
            );
            let Some(target_rva) = relative_target(section.rva + offset + 7, displacement) else {
                continue;
            };
            let Some(locator_slot_rva) = target_rva.checked_sub(mem::size_of::<usize>()) else {
                continue;
            };
            if !target_rva.is_multiple_of(mem::align_of::<usize>())
                || !image.is_readable_non_executable(locator_slot_rva, mem::size_of::<usize>() * 7)
            {
                continue;
            }
            let Ok(gate_pointer) = (unsafe {
                image.read_usize(target_rva + HDR_MENU_GATE_INVOKE_INDEX * mem::size_of::<usize>())
            }) else {
                continue;
            };
            if image.va_to_rva(gate_pointer) != Some(gate_rva) {
                continue;
            }
            let Ok(validated) = (unsafe { validate_menu_gate_vtable(image, target_rva, gate_rva) })
            else {
                continue;
            };
            if let Some(existing) = candidates
                .iter_mut()
                .find(|candidate| candidate.rva == validated.rva)
            {
                existing.reference_count += 1;
            } else {
                candidates.push(validated);
            }
        }
    }

    if candidates.len() != 1 {
        let rvas = candidates
            .iter()
            .map(|candidate| candidate.rva)
            .collect::<Vec<_>>();
        return Err(format!(
            "HDR menu-gate RTTI/vtable must resolve to exactly one candidate referenced by executable code, found {} ({})",
            candidates.len(),
            format_rvas(&rvas)
        ));
    }
    let candidate = candidates.pop().expect("one candidate was checked");
    if candidate.reference_count < 2 {
        return Err(format!(
            "HDR menu-gate vtable RVA 0x{:08X} has only {} executable LEA reference; expected at least 2",
            candidate.rva, candidate.reference_count
        ));
    }
    Ok(candidate)
}

unsafe fn validate_menu_gate_vtable(
    image: &LoadedImage,
    vtable_rva: usize,
    gate_rva: usize,
) -> Result<VtableMatch, String> {
    if !vtable_rva.is_multiple_of(mem::align_of::<usize>()) {
        return Err("vtable is not pointer-aligned".to_owned());
    }
    let locator_slot_rva = vtable_rva
        .checked_sub(mem::size_of::<usize>())
        .ok_or_else(|| "vtable has no Complete Object Locator slot".to_owned())?;
    let total_length = mem::size_of::<usize>() * 7;
    if !image.is_readable_non_executable(locator_slot_rva, total_length) {
        return Err("vtable is not in a readable non-executable PE section".to_owned());
    }

    let gate_pointer = unsafe {
        image.read_usize(vtable_rva + HDR_MENU_GATE_INVOKE_INDEX * mem::size_of::<usize>())?
    };
    if image.va_to_rva(gate_pointer) != Some(gate_rva) {
        return Err("vtable invoke slot does not point to the resolved gate".to_owned());
    }

    let locator_pointer = unsafe { image.read_usize(locator_slot_rva)? };
    let locator_rva = image
        .va_to_rva(locator_pointer)
        .ok_or_else(|| "vtable Complete Object Locator is outside the main image".to_owned())?;
    if !image.is_readable_non_executable(locator_rva, 24) {
        return Err("vtable Complete Object Locator is not readable data".to_owned());
    }
    let locator = unsafe { image.bytes_at(locator_rva, 24)? };
    let signature = u32::from_le_bytes(locator[0..4].try_into().expect("COL signature range"));
    let object_offset = u32::from_le_bytes(locator[4..8].try_into().expect("COL offset range"));
    let constructor_offset =
        u32::from_le_bytes(locator[8..12].try_into().expect("COL constructor range"));
    let type_descriptor_rva =
        u32::from_le_bytes(locator[12..16].try_into().expect("COL type range")) as usize;
    let class_descriptor_rva =
        u32::from_le_bytes(locator[16..20].try_into().expect("COL class range")) as usize;
    let self_rva = u32::from_le_bytes(locator[20..24].try_into().expect("COL self range")) as usize;
    if signature != 1 || object_offset != 0 || constructor_offset != 0 || self_rva != locator_rva {
        return Err("vtable Complete Object Locator fields do not match MSVC x64 RTTI".to_owned());
    }
    if !image.is_readable_non_executable(type_descriptor_rva, 17)
        || !image.is_readable_non_executable(class_descriptor_rva, 4)
    {
        return Err("vtable RTTI descriptors are outside readable data".to_owned());
    }
    let rtti_name =
        unsafe { image.read_ascii_c_string(type_descriptor_rva + 16, MAX_RTTI_NAME_LENGTH)? };
    if !is_hdr_menu_lambda_rtti_name(&rtti_name) {
        return Err("vtable RTTI type is not the HDR menu std::function lambda".to_owned());
    }

    let mut entries = [0usize; 6];
    for (index, entry) in entries.iter_mut().enumerate() {
        let pointer = unsafe { image.read_usize(vtable_rva + index * mem::size_of::<usize>())? };
        let rva = image
            .va_to_rva(pointer)
            .ok_or_else(|| format!("vtable entry {index} points outside the main image"))?;
        if !image.is_executable(rva, 1) {
            return Err(format!("vtable entry {index} is not executable PE code"));
        }
        *entry = rva;
    }

    Ok(VtableMatch {
        rva: vtable_rva,
        complete_object_locator_rva: locator_rva,
        entries,
        reference_count: 1,
        rtti_name,
    })
}

fn is_hdr_menu_lambda_rtti_name(name: &str) -> bool {
    const PREFIX: &str = ".?AV?$_Func_impl@V<lambda_";
    const SUFFIX: &str = ">@@V?$allocator@H@std@@_N$$V@std@@";
    name.strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
        .is_some_and(|hash| hash.len() == 32 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn relative_target(next_instruction_rva: usize, displacement: i32) -> Option<usize> {
    let result = next_instruction_rva as i128 + displacement as i128;
    (0..=usize::MAX as i128)
        .contains(&result)
        .then_some(result as usize)
}

fn format_rvas(rvas: &[usize]) -> String {
    if rvas.is_empty() {
        return "none".to_owned();
    }
    let mut formatted = rvas
        .iter()
        .take(8)
        .map(|rva| format!("0x{rva:08X}"))
        .collect::<Vec<_>>()
        .join(", ");
    if rvas.len() > 8 {
        formatted.push_str(&format!(", ... +{}", rvas.len() - 8));
    }
    formatted
}

#[derive(Clone)]
struct Section {
    name: String,
    rva: usize,
    size: usize,
    characteristics: u32,
}

impl Section {
    fn contains(&self, rva: usize, length: usize) -> bool {
        rva >= self.rva
            && rva
                .checked_add(length)
                .zip(self.rva.checked_add(self.size))
                .is_some_and(|(end, section_end)| end <= section_end)
    }

    fn is_readable(&self) -> bool {
        self.characteristics & IMAGE_SCN_MEM_READ != 0
    }

    fn is_executable(&self) -> bool {
        self.characteristics & IMAGE_SCN_MEM_EXECUTE != 0
    }
}

struct LoadedImage {
    base: *mut u8,
    size: usize,
    sections: Vec<Section>,
}

impl LoadedImage {
    unsafe fn main_module() -> Result<Self, String> {
        let base = unsafe { windows::main_module()? }.cast::<u8>();
        if unsafe { read_header_u16(base, 0)? } != 0x5A4D {
            return Err("main module has no MZ signature".to_owned());
        }
        let nt = unsafe { read_header_u32(base, 0x3C)? } as usize;
        if nt > 1024 * 1024 || unsafe { read_header_u32(base, nt)? } != 0x0000_4550 {
            return Err("main module has no usable PE signature".to_owned());
        }
        let section_count = unsafe { read_header_u16(base, nt + 6)? } as usize;
        if section_count == 0 || section_count > MAX_SECTION_COUNT {
            return Err(format!(
                "main module has an invalid PE section count ({section_count})"
            ));
        }
        let optional_size = unsafe { read_header_u16(base, nt + 20)? } as usize;
        let optional = nt + 24;
        if optional_size < 112 || unsafe { read_header_u16(base, optional)? } != 0x20B {
            return Err("main module is not a usable PE32+ image".to_owned());
        }
        let size = unsafe { read_header_u32(base, optional + 56)? } as usize;
        let headers_size = unsafe { read_header_u32(base, optional + 60)? } as usize;
        if !(4096..=MAX_IMAGE_SIZE).contains(&size) || headers_size == 0 || headers_size > size {
            return Err(format!("main module has an invalid image size ({size})"));
        }
        let section_table = optional
            .checked_add(optional_size)
            .ok_or_else(|| "PE section table offset overflow".to_owned())?;
        let section_table_end = section_table
            .checked_add(section_count * 40)
            .ok_or_else(|| "PE section table size overflow".to_owned())?;
        if section_table_end > headers_size
            || !unsafe {
                windows::range_is_readable(base.add(section_table).cast(), section_count * 40)
            }
        {
            return Err("PE section table is outside readable image headers".to_owned());
        }

        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let header = section_table + index * 40;
            let name_bytes = unsafe { read_header_bytes(base, header, 8)? };
            let name_length = name_bytes.iter().position(|byte| *byte == 0).unwrap_or(8);
            let name = String::from_utf8_lossy(&name_bytes[..name_length]).into_owned();
            let virtual_size = unsafe { read_header_u32(base, header + 8)? } as usize;
            let rva = unsafe { read_header_u32(base, header + 12)? } as usize;
            let raw_size = unsafe { read_header_u32(base, header + 16)? } as usize;
            let characteristics = unsafe { read_header_u32(base, header + 36)? };
            let mapped_size = virtual_size.max(raw_size);
            if mapped_size == 0 {
                continue;
            }
            if rva.checked_add(mapped_size).is_none_or(|end| end > size) {
                return Err(format!(
                    "PE section {name} extends outside the loaded image"
                ));
            }
            sections.push(Section {
                name,
                rva,
                size: mapped_size,
                characteristics,
            });
        }
        if !sections.iter().any(Section::is_executable) {
            return Err("main module has no executable PE sections".to_owned());
        }
        Ok(Self {
            base,
            size,
            sections,
        })
    }

    fn executable_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections
            .iter()
            .filter(|section| section.is_executable())
    }

    fn executable_section_summary(&self) -> String {
        self.executable_sections()
            .map(|section| {
                format!(
                    "{}@0x{:08X}+0x{:X}",
                    section.name, section.rva, section.size
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    unsafe fn find_signature(&self, signature: &Signature) -> Result<Vec<usize>, String> {
        let mut result = Vec::new();
        for section in self.executable_sections() {
            let bytes = unsafe { self.section_bytes(section)? };
            if bytes.len() < signature.length {
                continue;
            }
            for offset in 0..=bytes.len() - signature.length {
                if signature.matches(&bytes[offset..offset + signature.length]) {
                    result.push(section.rva + offset);
                }
            }
        }
        Ok(result)
    }

    unsafe fn find_direct_calls_to(&self, target_rva: usize) -> Result<Vec<usize>, String> {
        let mut result = Vec::new();
        for section in self.executable_sections() {
            let bytes = unsafe { self.section_bytes(section)? };
            if bytes.len() < 5 {
                continue;
            }
            for offset in 0..=bytes.len() - 5 {
                if bytes[offset] != 0xE8 {
                    continue;
                }
                let displacement = i32::from_le_bytes(
                    bytes[offset + 1..offset + 5]
                        .try_into()
                        .expect("fixed CALL displacement range"),
                );
                let call_rva = section.rva + offset;
                if relative_target(call_rva + 5, displacement) == Some(target_rva) {
                    result.push(call_rva);
                }
            }
        }
        Ok(result)
    }

    unsafe fn section_bytes<'a>(&'a self, section: &Section) -> Result<&'a [u8], String> {
        if !section.is_readable()
            || !unsafe {
                windows::range_is_readable(self.base.add(section.rva).cast(), section.size)
            }
        {
            return Err(format!(
                "PE section {} is not fully readable at runtime",
                section.name
            ));
        }
        // The PE range and current page protections were checked immediately
        // above; the loader keeps the image allocation alive for this process.
        Ok(unsafe { std::slice::from_raw_parts(self.base.add(section.rva), section.size) })
    }

    unsafe fn bytes_at(&self, rva: usize, length: usize) -> Result<&[u8], String> {
        self.sections
            .iter()
            .find(|section| section.contains(rva, length) && section.is_readable())
            .ok_or_else(|| {
                format!("RVA 0x{rva:08X}+0x{length:X} is outside readable PE sections")
            })?;
        if !unsafe { windows::range_is_readable(self.base.add(rva).cast(), length) } {
            return Err(format!(
                "RVA 0x{rva:08X}+0x{length:X} is not readable at runtime"
            ));
        }
        // The containing section and current memory protection were validated.
        Ok(unsafe { std::slice::from_raw_parts(self.base.add(rva), length) })
    }

    unsafe fn array_at<const N: usize>(&self, rva: usize) -> Result<[u8; N], String> {
        unsafe { self.bytes_at(rva, N)? }
            .try_into()
            .map_err(|_| "fixed-size PE read failed".to_owned())
    }

    unsafe fn read_usize(&self, rva: usize) -> Result<usize, String> {
        let bytes = unsafe { self.bytes_at(rva, mem::size_of::<usize>())? };
        Ok(usize::from_le_bytes(
            bytes.try_into().expect("pointer-sized PE range"),
        ))
    }

    unsafe fn read_ascii_c_string(&self, rva: usize, maximum: usize) -> Result<String, String> {
        let section = self
            .sections
            .iter()
            .find(|section| section.contains(rva, 1) && section.is_readable())
            .ok_or_else(|| format!("ASCII string RVA 0x{rva:08X} is outside readable data"))?;
        let available = (section.rva + section.size - rva).min(maximum);
        let bytes = unsafe { self.bytes_at(rva, available)? };
        let length = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| format!("ASCII string at RVA 0x{rva:08X} is not terminated"))?;
        if !bytes[..length].is_ascii() {
            return Err(format!("string at RVA 0x{rva:08X} is not ASCII"));
        }
        Ok(std::str::from_utf8(&bytes[..length])
            .expect("ASCII is valid UTF-8")
            .to_owned())
    }

    fn section_name_for(&self, rva: usize, length: usize) -> Result<&str, String> {
        self.sections
            .iter()
            .find(|section| section.contains(rva, length))
            .map(|section| section.name.as_str())
            .ok_or_else(|| format!("RVA 0x{rva:08X} is outside PE sections"))
    }

    fn is_executable(&self, rva: usize, length: usize) -> bool {
        self.sections
            .iter()
            .any(|section| section.contains(rva, length) && section.is_executable())
    }

    fn is_readable_non_executable(&self, rva: usize, length: usize) -> bool {
        self.sections.iter().any(|section| {
            section.contains(rva, length) && section.is_readable() && !section.is_executable()
        })
    }

    fn va_to_rva(&self, address: usize) -> Option<usize> {
        let base = self.base as usize;
        let rva = address.checked_sub(base)?;
        (rva < self.size).then_some(rva)
    }
}

unsafe fn read_header_bytes(
    base: *mut u8,
    offset: usize,
    length: usize,
) -> Result<Vec<u8>, String> {
    let address = unsafe { base.add(offset) };
    if !unsafe { windows::range_is_readable(address.cast::<c_void>(), length) } {
        return Err(format!(
            "main module PE header range 0x{offset:X}+0x{length:X} is not readable"
        ));
    }
    Ok(unsafe { std::slice::from_raw_parts(address, length) }.to_vec())
}

unsafe fn read_header_u16(base: *mut u8, offset: usize) -> Result<u16, String> {
    let bytes = unsafe { read_header_bytes(base, offset, 2)? };
    Ok(u16::from_le_bytes(bytes.try_into().expect("u16 PE field")))
}

unsafe fn read_header_u32(base: *mut u8, offset: usize) -> Result<u32, String> {
    let bytes = unsafe { read_header_bytes(base, offset, 4)? };
    Ok(u32::from_le_bytes(bytes.try_into().expect("u32 PE field")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AVAILABILITY_1162: [u8; 101] = hex_101([
        0x48, 0x83, 0xEC, 0x48, 0x48, 0x8B, 0x05, 0x35, 0x85, 0x30, 0x03, 0x48, 0x33, 0xC4, 0x48,
        0x89, 0x44, 0x24, 0x30, 0xE8, 0x58, 0x85, 0xE3, 0xFF, 0x84, 0xC0, 0x74, 0x35, 0x33, 0xC0,
        0x48, 0x8D, 0x4C, 0x24, 0x20, 0xB2, 0x01, 0x48, 0x89, 0x44, 0x24, 0x20, 0x89, 0x44, 0x24,
        0x28, 0xE8, 0xED, 0x47, 0x53, 0x01, 0x84, 0xC0, 0x75, 0x1A, 0x38, 0x44, 0x24, 0x20, 0x74,
        0x14, 0xB0, 0x01, 0x48, 0x8B, 0x4C, 0x24, 0x30, 0x48, 0x33, 0xCC, 0xE8, 0x74, 0x82, 0xBA,
        0x01, 0x48, 0x83, 0xC4, 0x48, 0xC3, 0x32, 0xC0, 0x48, 0x8B, 0x4C, 0x24, 0x30, 0x48, 0x33,
        0xCC, 0xE8, 0x60, 0x82, 0xBA, 0x01, 0x48, 0x83, 0xC4, 0x48, 0xC3,
    ]);

    const AVAILABILITY_117: [u8; 101] = hex_101([
        0x48, 0x83, 0xEC, 0x48, 0x48, 0x8B, 0x05, 0xF5, 0xB3, 0x30, 0x03, 0x48, 0x33, 0xC4, 0x48,
        0x89, 0x44, 0x24, 0x30, 0xE8, 0x38, 0x82, 0xE3, 0xFF, 0x84, 0xC0, 0x74, 0x35, 0x33, 0xC0,
        0x48, 0x8D, 0x4C, 0x24, 0x20, 0xB2, 0x01, 0x48, 0x89, 0x44, 0x24, 0x20, 0x89, 0x44, 0x24,
        0x28, 0xE8, 0x4D, 0x54, 0x53, 0x01, 0x84, 0xC0, 0x75, 0x1A, 0x38, 0x44, 0x24, 0x20, 0x74,
        0x14, 0xB0, 0x01, 0x48, 0x8B, 0x4C, 0x24, 0x30, 0x48, 0x33, 0xCC, 0xE8, 0xE4, 0x98, 0xBA,
        0x01, 0x48, 0x83, 0xC4, 0x48, 0xC3, 0x32, 0xC0, 0x48, 0x8B, 0x4C, 0x24, 0x30, 0x48, 0x33,
        0xCC, 0xE8, 0xD0, 0x98, 0xBA, 0x01, 0x48, 0x83, 0xC4, 0x48, 0xC3,
    ]);

    const fn hex_101(bytes: [u8; 101]) -> [u8; 101] {
        bytes
    }

    fn materialize_signature(signature: &Signature) -> Vec<u8> {
        let mut bytes = vec![0xCC; signature.length];
        for segment in signature.segments {
            let end = segment.offset + segment.bytes.len();
            bytes[segment.offset..end].copy_from_slice(segment.bytes);
        }
        bytes
    }

    #[test]
    fn availability_signature_ignores_only_relocations_across_known_builds() {
        assert!(AVAILABILITY_SIGNATURE.matches(&AVAILABILITY_1162));
        assert!(AVAILABILITY_SIGNATURE.matches(&AVAILABILITY_117));
        let mut changed_control_flow = AVAILABILITY_117;
        changed_control_flow[27] = 0x34;
        assert!(!AVAILABILITY_SIGNATURE.matches(&changed_control_flow));
    }

    #[test]
    fn every_signature_segment_is_in_bounds_and_semantic_bytes_are_required() {
        for signature in [
            &AVAILABILITY_SIGNATURE,
            &GRAPHICS_CONFIG_SIGNATURE,
            &BACKEND_ACTUAL_SIGNATURE,
        ] {
            let mut bytes = materialize_signature(signature);
            assert!(signature.matches(&bytes));
            for segment in signature.segments {
                assert!(segment.offset + segment.bytes.len() <= signature.length);
            }
            let semantic_offset = signature.segments.last().unwrap().offset;
            bytes[semantic_offset] ^= 1;
            assert!(!signature.matches(&bytes));
        }
    }

    #[test]
    fn identifies_both_known_fingerprints() {
        for build in KNOWN_BUILDS {
            assert_eq!(
                known_build(build.size, &build.sha256.to_ascii_lowercase()).map(|item| item.name),
                Some(build.name)
            );
            assert_eq!(
                build.expected.menu_gate_entries[HDR_MENU_GATE_INVOKE_INDEX],
                build.expected.menu_gate_invoke_rva
            );
        }
        assert!(known_build(87_024_720, "00").is_none());
    }

    #[test]
    fn accepts_lambda_hash_changes_but_not_another_rtti_shape() {
        assert!(is_hdr_menu_lambda_rtti_name(
            ".?AV?$_Func_impl@V<lambda_a057e0ed719cb0e74a1562f63717e283>@@V?$allocator@H@std@@_N$$V@std@@"
        ));
        assert!(is_hdr_menu_lambda_rtti_name(
            ".?AV?$_Func_impl@V<lambda_0123456789abcdef0123456789ABCDEF>@@V?$allocator@H@std@@_N$$V@std@@"
        ));
        assert!(!is_hdr_menu_lambda_rtti_name(
            ".?AV?$_Func_impl@V<lambda_short>@@V?$allocator@H@std@@_N$$V@std@@"
        ));
    }

    #[test]
    fn resolves_signed_relative_targets_without_wrapping() {
        assert_eq!(relative_target(0x1000, -0x20), Some(0x0FE0));
        assert_eq!(relative_target(0x1000, 0x20), Some(0x1020));
        assert_eq!(relative_target(0, -1), None);
    }
}
