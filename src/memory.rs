//! Memory management for WASM linear memory.
//!
//! This module provides functions for allocating and deallocating memory
//! that can be called from the host (e.g., Java via Chicory). It also
//! provides utilities for packing and unpacking pointer+length pairs.

use std::alloc::{alloc, dealloc as std_dealloc, Layout};
use thiserror::Error;

/// Allocates a block of memory in the WASM linear memory.
///
/// # Safety
/// This function is safe to call from the host. The returned pointer
/// is valid for writes of `len` bytes. The caller is responsible for
/// eventually calling `dealloc` with the same pointer and length.
///
/// # Arguments
/// * `len` - Number of bytes to allocate
///
/// # Returns
/// Pointer to the allocated memory, or null if allocation fails
///
/// # Example
/// ```
/// // From Java/Chicory:
/// // long ptr = instance.export("alloc").apply(100)[0];
/// ```
#[no_mangle]
pub extern "C" fn wasm_alloc(len: u32) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }

    // SAFETY: We ensure alignment is valid (1 byte) and len > 0
    unsafe {
        let layout = match Layout::from_size_align(len as usize, 1) {
            Ok(layout) => layout,
            Err(_) => return std::ptr::null_mut(),
        };
        alloc(layout)
    }
}

/// Deallocates a block of memory previously allocated with `alloc`.
///
/// # Safety
/// - `ptr` must have been returned by a previous call to `alloc`
/// - `len` must be the same length that was passed to `alloc`
/// - The memory must not have been deallocated already
///
/// # Arguments
/// * `ptr` - Pointer to the memory to deallocate
/// * `len` - Size of the allocation in bytes
///
/// # Note
/// This function is exposed as a safe FFI boundary for WASM runtimes.
/// The safety requirements are documented and must be upheld by the caller.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn wasm_dealloc(ptr: *mut u8, len: u32) {
    if ptr.is_null() || len == 0 {
        return;
    }

    // SAFETY: Caller guarantees ptr was allocated with the same len.
    // This is an FFI boundary where the caller is responsible for safety.
    // We check for null and zero-length above.
    let layout = match Layout::from_size_align(len as usize, 1) {
        Ok(layout) => layout,
        Err(_) => return,
    };

    // SAFETY: We have verified the pointer is non-null and the layout is valid.
    // The caller guarantees the pointer was allocated with wasm_alloc with the same len.
    unsafe {
        std_dealloc(ptr, layout);
    }
}

/// Packs a pointer and length into a single u64 value.
///
/// The upper 32 bits contain the pointer, and the lower 32 bits contain the length.
/// This allows returning both values from a single WASM function.
///
/// # Arguments
/// * `ptr` - Pointer to pack (will be truncated to 32 bits)
/// * `len` - Length to pack
///
/// # Returns
/// A u64 with the pointer in the upper 32 bits and length in the lower 32 bits
#[inline]
pub fn pack_ptr_len(ptr: *const u8, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

/// Unpacks a u64 value into a pointer and length.
///
/// # Arguments
/// * `packed` - The packed u64 value
///
/// # Returns
/// A tuple of (pointer, length)
#[inline]
pub fn unpack_ptr_len(packed: u64) -> (*const u8, u32) {
    let ptr = (packed >> 32) as *const u8;
    let len = (packed & 0xFFFFFFFF) as u32;
    (ptr, len)
}

/// Memory allocation error for WASM operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Failed to allocate WASM memory")]
pub struct MemoryAllocationError;

/// Writes a string to newly allocated memory and returns a packed pointer+length.
///
/// # Arguments
/// * `s` - The string to write to memory
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits).
/// Returns 0 if allocation fails (distinguishable from empty string which returns valid ptr + len 0).
///
/// # Note
/// For empty strings, this allocates 1 byte to ensure a non-null pointer,
/// allowing callers to distinguish empty strings from allocation failures.
pub fn string_to_memory(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;

    // For empty strings, allocate 1 byte to ensure non-null pointer
    // This allows callers to distinguish empty string (valid ptr, len=0)
    // from allocation failure (ptr=0, len=0)
    let alloc_size = if len == 0 { 1 } else { len };
    let ptr = wasm_alloc(alloc_size);

    if ptr.is_null() {
        return 0;
    }

    // SAFETY: We just allocated this memory and know it's valid
    if len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len as usize);
        }
    }

    pack_ptr_len(ptr, len)
}

/// Writes binary data to newly allocated memory and returns a packed pointer+length.
///
/// # Arguments
/// * `data` - The binary data to write to memory
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits).
/// Returns 0 if allocation fails.
pub fn bytes_to_memory(data: &[u8]) -> u64 {
    let len = data.len() as u32;

    // For empty data, allocate 1 byte to ensure non-null pointer
    let alloc_size = if len == 0 { 1 } else { len };
    let ptr = wasm_alloc(alloc_size);

    if ptr.is_null() {
        return 0;
    }

    // SAFETY: We just allocated this memory and know it's valid
    if len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, len as usize);
        }
    }

    pack_ptr_len(ptr, len)
}

/// Writes a string to newly allocated memory, returning a Result for explicit error handling.
///
/// This is the preferred API for Rust callers who want explicit error handling.
///
/// # Arguments
/// * `s` - The string to write to memory
///
/// # Returns
/// * `Ok(u64)` - Packed pointer+length on success
/// * `Err(MemoryAllocationError)` - If allocation fails
pub fn string_to_memory_checked(s: &str) -> Result<u64, MemoryAllocationError> {
    let packed = string_to_memory(s);
    let (ptr, _len) = unpack_ptr_len(packed);

    if ptr.is_null() {
        Err(MemoryAllocationError)
    } else {
        Ok(packed)
    }
}

/// Reads a string from WASM memory.
///
/// # Safety
/// - `ptr` must point to valid memory
/// - The memory from `ptr` to `ptr + len` must be valid UTF-8
///
/// # Arguments
/// * `ptr` - Pointer to the start of the string
/// * `len` - Length of the string in bytes
///
/// # Returns
/// The string, or an error if the memory is invalid
pub unsafe fn string_from_memory(ptr: *const u8, len: u32) -> Result<String, &'static str> {
    if ptr.is_null() {
        return Err("Null pointer provided");
    }

    if len == 0 {
        return Ok(String::new());
    }

    let slice = std::slice::from_raw_parts(ptr, len as usize);
    std::str::from_utf8(slice)
        .map(|s| s.to_string())
        .map_err(|_| "Invalid UTF-8 in memory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack() {
        let original_ptr = 0x12345678 as *const u8;
        let original_len = 42u32;

        let packed = pack_ptr_len(original_ptr, original_len);
        let (unpacked_ptr, unpacked_len) = unpack_ptr_len(packed);

        assert_eq!(unpacked_ptr, original_ptr);
        assert_eq!(unpacked_len, original_len);
    }

    #[test]
    fn test_alloc_dealloc() {
        let ptr = wasm_alloc(100);
        assert!(!ptr.is_null());
        wasm_dealloc(ptr, 100);
    }

    #[test]
    fn test_alloc_zero() {
        let ptr = wasm_alloc(0);
        assert!(ptr.is_null());
    }

    #[test]
    fn test_dealloc_null() {
        // Should not panic
        wasm_dealloc(std::ptr::null_mut(), 100);
    }

    #[test]
    fn test_string_to_memory() {
        let test_str = "Hello, World!";
        let packed = string_to_memory(test_str);

        assert_ne!(packed, 0);

        // In WASM (32-bit pointers), the packing works correctly.
        // In native tests (64-bit pointers), we need to extract the original pointer
        // before the truncation by re-allocating.
        // For this test, we verify the length is correct and the packed value is non-zero.
        let len = (packed & 0xFFFFFFFF) as u32;
        assert_eq!(len as usize, test_str.len());

        // For a more complete test in native mode, allocate and test separately
        let ptr = wasm_alloc(test_str.len() as u32);
        assert!(!ptr.is_null());

        unsafe {
            std::ptr::copy_nonoverlapping(test_str.as_bytes().as_ptr(), ptr, test_str.len());
            let result = string_from_memory(ptr, test_str.len() as u32).unwrap();
            assert_eq!(result, test_str);
            wasm_dealloc(ptr, test_str.len() as u32);
        }
    }

    #[test]
    fn test_string_to_memory_empty_string() {
        // Empty strings should return a non-null pointer with length 0
        // This distinguishes them from allocation failures (which return 0)
        let packed = string_to_memory("");
        assert_ne!(
            packed, 0,
            "Empty string should not return 0 (allocation failure)"
        );

        let (ptr, len) = unpack_ptr_len(packed);
        assert!(!ptr.is_null(), "Empty string should have non-null pointer");
        assert_eq!(len, 0, "Empty string should have length 0");
    }

    #[test]
    fn test_string_to_memory_checked() {
        // Normal string should succeed
        let result = string_to_memory_checked("hello");
        assert!(result.is_ok());

        // Empty string should also succeed
        let result = string_to_memory_checked("");
        assert!(result.is_ok());
    }

    #[test]
    fn test_memory_allocation_error_display() {
        let err = MemoryAllocationError;
        assert_eq!(format!("{}", err), "Failed to allocate WASM memory");
    }
}
