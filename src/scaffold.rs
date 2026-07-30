//! Macros that assemble a C entry point out of the primitives in [`crate::ffi`].
//!
//! # What is repeated, and why a macro
//!
//! [`crate::ffi`] hands out the *materials* — a last-error slot, a panic guard, the two
//! boundary failure reasons. Assembling them is five steps, and only one of them says
//! anything about the library:
//!
//! 1. clear the slot, so a caller polling it does not read a previous failure
//! 2. reject a null handle with [`crate::ffi::invalid_argument`] and return the sentinel
//! 3. run the body inside [`crate::ffi::catch`] ← the only step with domain content
//! 4. move the result across the ABI, reporting [`crate::ffi::invalid_output`] if it cannot
//! 5. record a failure and return the sentinel
//!
//! Steps 1, 2, 4 and 5 are about eighteen lines with no library content in them, and the
//! sentinel in steps 2 and 5 differs by return type — null for a pointer, `-1` for a
//! count. That is why these are several macros rather than one: a single general macro
//! would have to be told the sentinel, and being told is how the wrong one gets used.
//!
//! # Names are spelled out, not built
//!
//! Every macro takes the exported function name in full, following
//! [`crate::export_last_error_abi!`]: `macro_rules!` cannot concatenate identifiers, and
//! an exported symbol appearing literally in the source is what someone greps for.
//! Doc comments are passed through from the call site, because what an entry point means
//! is not something this crate knows.

/// Read a nullable C string argument as `&str`, classifying both ways it can fail.
///
/// Expands to a `Result`, so the usual form is `with_c_str!(path)?` inside a
/// [`catch`](crate::ffi::catch) closure. The null message is built from the argument's own
/// name — `with_c_str!(resource_id)` reports `"resource_id is null"`, which is the wording
/// the family already uses.
///
/// Use this where the entry point has no out-parameter, so that classifying a null
/// argument inside the closure is indistinguishable from classifying it before. Where
/// there *is* an out-parameter, check for null first and use
/// [`ffi::c_str_utf8`](crate::ffi::c_str_utf8) here instead — see
/// [`export_bytes_getter!`](crate::export_bytes_getter).
///
/// # Safety
///
/// `$ptr` must be null or point to a NUL-terminated string. This expands to an unsafe
/// read, so it has to appear in an `unsafe` context.
///
/// ```
/// # use std::ffi::{c_char, CString};
/// # use uncore::ffi::FfiError;
/// unsafe fn name_length(path: *const c_char) -> Result<usize, FfiError> {
///     let path = uncore::with_c_str!(path)?;
///     Ok(path.len())
/// }
///
/// let path = CString::new("a.txt").unwrap();
/// assert_eq!(unsafe { name_length(path.as_ptr()) }.unwrap(), 5);
/// assert_eq!(
///     unsafe { name_length(std::ptr::null()) }.unwrap_err().1,
///     "path is null"
/// );
/// ```
#[macro_export]
macro_rules! with_c_str {
    ($ptr:ident) => {
        if $ptr.is_null() {
            ::std::result::Result::Err($crate::ffi::invalid_argument(concat!(
                stringify!($ptr),
                " is null"
            )))
        } else {
            unsafe { $crate::ffi::c_str_utf8($ptr) }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_char, CString};

    use crate::ffi::FfiError;

    unsafe fn read(ptr: *const c_char) -> Result<&'static str, FfiError> {
        crate::with_c_str!(ptr)
    }

    #[test]
    fn a_null_c_string_argument_is_named_in_its_own_message() {
        let failure = unsafe { read(std::ptr::null()) }.unwrap_err();
        assert_eq!(failure.0, crate::kind::INVALID_ARGUMENT);
        assert_eq!(
            failure.1, "ptr is null",
            "the message is built from the argument's own name"
        );
    }

    #[test]
    fn a_valid_c_string_argument_reads_through() {
        let text = CString::new("document.hwp").unwrap();
        assert_eq!(unsafe { read(text.as_ptr()) }.unwrap(), "document.hwp");
    }

    #[test]
    fn a_non_utf8_c_string_argument_is_an_invalid_argument() {
        // 0xFF is not a valid UTF-8 lead byte.
        let raw = [0xFFu8, 0x00];
        let failure = unsafe { read(raw.as_ptr() as *const c_char) }.unwrap_err();
        assert_eq!(failure.0, crate::kind::INVALID_ARGUMENT);
        assert!(
            !failure.1.is_empty(),
            "the conversion error must survive as a message"
        );
    }
}
