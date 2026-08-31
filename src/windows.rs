use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

pub type Module = *mut c_void;
pub type Handle = *mut c_void;
pub type ThreadStart = unsafe extern "system" fn(*mut c_void) -> u32;

const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const PAGE_EXECUTE_READ: u32 = 0x20;
const PAGE_EXECUTE: u32 = 0x10;
const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
const PAGE_READONLY: u32 = 0x02;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_NOACCESS: u32 = 0x01;
const PAGE_GUARD: u32 = 0x100;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x0000_0002;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x0000_0004;

#[repr(C)]
struct MemoryBasicInformation {
    base_address: *mut c_void,
    allocation_base: *mut c_void,
    allocation_protect: u32,
    partition_id: u16,
    region_size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(object: Handle) -> i32;
    fn CreateThread(
        thread_attributes: *const c_void,
        stack_size: usize,
        start_address: Option<ThreadStart>,
        parameter: *mut c_void,
        creation_flags: u32,
        thread_id: *mut u32,
    ) -> Handle;
    fn DisableThreadLibraryCalls(module: Module) -> i32;
    fn FlushInstructionCache(process: Handle, base_address: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetModuleFileNameW(module: Module, filename: *mut u16, size: u32) -> u32;
    fn GetModuleHandleExW(
        flags: u32,
        module_name_or_address: *const u16,
        module: *mut Module,
    ) -> i32;
    fn GetModuleHandleW(module_name: *const u16) -> Module;
    fn Sleep(milliseconds: u32);
    fn VirtualAlloc(
        address: *mut c_void,
        size: usize,
        allocation_type: u32,
        protect: u32,
    ) -> *mut c_void;
    fn VirtualProtect(
        address: *mut c_void,
        size: usize,
        new_protect: u32,
        old_protect: *mut u32,
    ) -> i32;
    fn VirtualQuery(
        address: *const c_void,
        information: *mut MemoryBasicInformation,
        length: usize,
    ) -> usize;
}

pub unsafe fn disable_thread_notifications(module: Module) {
    let _ = unsafe { DisableThreadLibraryCalls(module) };
}

pub unsafe fn spawn_thread(start: ThreadStart) -> bool {
    let handle = unsafe {
        CreateThread(
            ptr::null(),
            0,
            Some(start),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )
    };
    if handle.is_null() {
        return false;
    }

    let _ = unsafe { CloseHandle(handle) };
    true
}

pub fn sleep(milliseconds: u32) {
    unsafe { Sleep(milliseconds) };
}

pub unsafe fn main_module() -> Result<Module, String> {
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if module.is_null() {
        Err("GetModuleHandleW(NULL) failed".to_owned())
    } else {
        Ok(module)
    }
}

pub unsafe fn address_is_in_module(address: *const c_void, module_name: &str) -> bool {
    if address.is_null() {
        return false;
    }

    let wide_name = module_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let expected = unsafe { GetModuleHandleW(wide_name.as_ptr()) };
    if expected.is_null() {
        return false;
    }

    let mut actual = ptr::null_mut();
    let flags =
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
    unsafe {
        GetModuleHandleExW(flags, address.cast::<u16>(), &mut actual) != 0 && actual == expected
    }
}

pub unsafe fn address_is_executable(address: *const c_void) -> bool {
    if address.is_null() {
        return false;
    }

    let mut information = MemoryBasicInformation {
        base_address: ptr::null_mut(),
        allocation_base: ptr::null_mut(),
        allocation_protect: 0,
        partition_id: 0,
        region_size: 0,
        state: 0,
        protect: 0,
        kind: 0,
    };
    let queried = unsafe {
        VirtualQuery(
            address,
            &mut information,
            std::mem::size_of::<MemoryBasicInformation>(),
        )
    };
    queried == std::mem::size_of::<MemoryBasicInformation>()
        && information.state == MEM_COMMIT
        && information.protect & (PAGE_NOACCESS | PAGE_GUARD) == 0
        && information.protect
            & (PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY)
            != 0
}

pub unsafe fn module_path(module: Module) -> Result<PathBuf, String> {
    let mut capacity = 512usize;

    loop {
        let mut buffer = vec![0u16; capacity];
        let length = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), capacity as u32) };
        if length == 0 {
            return Err("GetModuleFileNameW failed".to_owned());
        }
        if (length as usize) < capacity - 1 {
            buffer.truncate(length as usize);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }

        capacity *= 2;
        if capacity > 32_768 {
            return Err("module path exceeds the Windows path limit".to_owned());
        }
    }
}

pub unsafe fn write_memory(address: *mut u8, bytes: &[u8]) -> Result<(), String> {
    if address.is_null() || bytes.is_empty() {
        return Err("invalid memory patch range".to_owned());
    }

    let mut old_protect = 0u32;
    if unsafe {
        VirtualProtect(
            address.cast(),
            bytes.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    } == 0
    {
        return Err("VirtualProtect(PAGE_EXECUTE_READWRITE) failed".to_owned());
    }

    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), address, bytes.len()) };
    let process = unsafe { GetCurrentProcess() };
    let flush_ok = unsafe { FlushInstructionCache(process, address.cast(), bytes.len()) } != 0;

    let mut ignored = 0u32;
    let restore_ok =
        unsafe { VirtualProtect(address.cast(), bytes.len(), old_protect, &mut ignored) } != 0;

    if !flush_ok {
        return Err("FlushInstructionCache failed after applying the patch".to_owned());
    }
    if !restore_ok {
        return Err("VirtualProtect failed to restore page protection".to_owned());
    }

    Ok(())
}

pub unsafe fn write_pointer(address: *mut *mut c_void, value: *mut c_void) -> Result<(), String> {
    unsafe { write_memory(address.cast(), &usize::to_ne_bytes(value as usize)) }
}

pub unsafe fn clone_memory_region_containing(
    address: *const c_void,
    required_bytes_before_address: usize,
    required_bytes_after_address: usize,
    maximum_region_size: usize,
) -> Result<(*mut u8, *mut c_void, usize), String> {
    if address.is_null() {
        return Err("cannot clone memory around a null address".to_owned());
    }

    let mut information = MemoryBasicInformation {
        base_address: ptr::null_mut(),
        allocation_base: ptr::null_mut(),
        allocation_protect: 0,
        partition_id: 0,
        region_size: 0,
        state: 0,
        protect: 0,
        kind: 0,
    };
    let queried = unsafe {
        VirtualQuery(
            address,
            &mut information,
            std::mem::size_of::<MemoryBasicInformation>(),
        )
    };
    if queried != std::mem::size_of::<MemoryBasicInformation>() {
        return Err("VirtualQuery failed for the COM vtable".to_owned());
    }
    if information.state != MEM_COMMIT
        || information.protect & (PAGE_NOACCESS | PAGE_GUARD) != 0
        || information.region_size == 0
    {
        return Err("COM vtable is not in a readable committed region".to_owned());
    }
    let base = information.base_address as usize;
    let source = address as usize;
    let offset = source
        .checked_sub(base)
        .ok_or_else(|| "COM vtable is outside its queried memory region".to_owned())?;
    let required_end = offset
        .checked_add(required_bytes_after_address)
        .ok_or_else(|| "COM vtable clone size overflow".to_owned())?;
    if required_end > information.region_size {
        return Err("COM vtable extends beyond its readable memory region".to_owned());
    }

    // Some DXGI implementations place their COM vtable at the first byte of a
    // committed region, while others leave space for a negative metadata slot.
    // Preserve an existing prefix when present and synthesize zero-filled space
    // when it is absent so the shadow vtable works with both layouts.
    let prefix_padding = required_bytes_before_address.saturating_sub(offset);
    let clone_size = information
        .region_size
        .checked_add(prefix_padding)
        .ok_or_else(|| "COM vtable clone size overflow".to_owned())?;
    if clone_size > maximum_region_size {
        return Err(format!(
            "COM vtable memory clone is unexpectedly large ({clone_size} bytes)"
        ));
    }

    let clone = unsafe { allocate_read_write(clone_size) }?;
    unsafe {
        ptr::write_bytes(clone, 0, prefix_padding);
        ptr::copy_nonoverlapping(
            information.base_address.cast::<u8>(),
            clone.add(prefix_padding),
            information.region_size,
        )
    };
    let cloned_address = unsafe { clone.add(prefix_padding + offset) }.cast::<c_void>();
    Ok((clone, cloned_address, clone_size))
}

pub unsafe fn allocate_read_write(size: usize) -> Result<*mut u8, String> {
    if size == 0 {
        return Err("cannot allocate an empty memory region".to_owned());
    }
    let allocation = unsafe {
        VirtualAlloc(
            ptr::null_mut(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    }
    .cast::<u8>();
    if allocation.is_null() {
        Err("VirtualAlloc(PAGE_READWRITE) failed".to_owned())
    } else {
        Ok(allocation)
    }
}

pub unsafe fn protect_read_only(address: *mut u8, size: usize) -> Result<(), String> {
    if address.is_null() || size == 0 {
        return Err("invalid read-only memory range".to_owned());
    }
    let mut old_protect = 0u32;
    if unsafe { VirtualProtect(address.cast(), size, PAGE_READONLY, &mut old_protect) } == 0 {
        Err("VirtualProtect(PAGE_READONLY) failed".to_owned())
    } else {
        Ok(())
    }
}

pub unsafe fn protect_read_write(address: *mut u8, size: usize) -> Result<(), String> {
    if address.is_null() || size == 0 {
        return Err("invalid read-write memory range".to_owned());
    }
    let mut old_protect = 0u32;
    if unsafe { VirtualProtect(address.cast(), size, PAGE_READWRITE, &mut old_protect) } == 0 {
        Err("VirtualProtect(PAGE_READWRITE) failed".to_owned())
    } else {
        Ok(())
    }
}

pub unsafe fn protect_execute_read(address: *mut u8, size: usize) -> Result<(), String> {
    if address.is_null() || size == 0 {
        return Err("invalid executable memory range".to_owned());
    }
    let mut old_protect = 0u32;
    if unsafe { VirtualProtect(address.cast(), size, PAGE_EXECUTE_READ, &mut old_protect) } == 0 {
        return Err("VirtualProtect(PAGE_EXECUTE_READ) failed".to_owned());
    }
    let process = unsafe { GetCurrentProcess() };
    if unsafe { FlushInstructionCache(process, address.cast(), size) } == 0 {
        return Err("FlushInstructionCache failed for executable memory".to_owned());
    }
    Ok(())
}

pub unsafe fn compare_exchange_pointer(
    address: *mut *mut c_void,
    expected: *mut c_void,
    replacement: *mut c_void,
) -> Result<bool, String> {
    if address.is_null() || !address.is_aligned() {
        return Err("invalid atomic pointer slot".to_owned());
    }
    let atomic = unsafe { AtomicPtr::from_ptr(address) };
    Ok(atomic
        .compare_exchange(expected, replacement, Ordering::AcqRel, Ordering::Acquire)
        .is_ok())
}
