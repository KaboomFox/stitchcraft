//! The VectorCraft plug-in ABI v1 shim (wasm32 only).
//!
//! VectorCraft calls these exports with offsets into the module's linear memory. Turning those offsets
//! into slices needs `unsafe`; this module is the single place in StitchCraft where that is allowed
//! (`cargo xtask unsafe-audit` checks it). It only moves bytes: all logic lives in safe, natively tested
//! code (`crate::effect`, `crate::manifest`).
//!
//! Contract (VectorCraft `docs/plugins.md`): no imports; `vc_abi_version() -> 1`; `vc_manifest()` and
//! `vc_run()` return `(len << 32) | ptr` of UTF-8 JSON, or a negative code; `vc_alloc` blocks are never
//! freed because every run gets a fresh instance.
#![allow(unsafe_code)]

/// The plug-in ABI version this module implements.
#[unsafe(no_mangle)]
pub extern "C" fn vc_abi_version() -> u32 {
    1
}

/// `(len << 32) | ptr` of the manifest JSON.
#[unsafe(no_mangle)]
pub extern "C" fn vc_manifest() -> u64 {
    match serde_json::to_vec(&crate::manifest::hello()) {
        Ok(bytes) => pack(bytes.leak()),
        Err(_) => 0,
    }
}

/// A block of `size` bytes the host may write into, or 0 if it cannot be allocated.
#[unsafe(no_mangle)]
pub extern "C" fn vc_alloc(size: u32) -> u32 {
    let Ok(size) = usize::try_from(size) else { return 0 };
    let mut block = Vec::<u8>::new();
    if block.try_reserve_exact(size).is_err() {
        return 0;
    }
    let ptr = block.as_mut_ptr();
    std::mem::forget(block);
    // On wasm32 a pointer is 32 bits wide, so the conversion is exact.
    u32::try_from(ptr as usize).unwrap_or(0)
}

/// Runs the plug-in on `input` (the objects JSON) with `params` (the validated parameters JSON).
///
/// # Safety
///
/// The host guarantees that both blocks came from [`vc_alloc`] and hold `input_len` and `params_len`
/// initialized bytes (ABI v1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vc_run(input: *const u8, input_len: u32, params: *const u8, params_len: u32) -> i64 {
    let (Ok(input_len), Ok(params_len)) = (usize::try_from(input_len), usize::try_from(params_len)) else {
        return crate::effect::RunError::BadInput.code();
    };
    // SAFETY: per this function's contract, `input` points to `input_len` initialized bytes from `vc_alloc`.
    let input = unsafe { std::slice::from_raw_parts(input, input_len) };
    // SAFETY: per this function's contract, `params` points to `params_len` initialized bytes from `vc_alloc`.
    let params = unsafe { std::slice::from_raw_parts(params, params_len) };
    match crate::effect::hello(input, params) {
        Ok(bytes) => i64::try_from(pack(bytes.leak())).unwrap_or(crate::effect::RunError::Encode.code()),
        Err(e) => e.code(),
    }
}

/// `(len << 32) | ptr` for bytes that stay alive for the rest of this instance.
fn pack(bytes: &'static [u8]) -> u64 {
    let len = u64::try_from(bytes.len()).unwrap_or(0);
    let ptr = u64::try_from(bytes.as_ptr() as usize).unwrap_or(0);
    (len << 32) | ptr
}
