//! Ivybridge+ RDRAND feature.

/// rdrand set the carry flag to 1 if the random is well done, else loop while it works
pub fn rdrand() -> u32 {
    let mut result: u32;

    unsafe {
        asm!(
            "2:
            rdrand eax
            jnc 2b",
            out("eax") result,
            options(),
        );
    }
    result
}
