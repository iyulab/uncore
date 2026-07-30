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
///     let path = unsafe { uncore::with_c_str!(path) }?;
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
            $crate::ffi::c_str_utf8($ptr)
        }
    };
}

/// Declare an opaque document handle and the function that frees it.
///
/// The handle is what a C caller holds: a `#[repr(C)]` newtype around the library's own
/// document type with one private field. The field name is given rather than invented,
/// because every entry point projects through it (`(*doc).inner`) and that has to be
/// readable in the consuming source.
///
/// `#[repr(C)]` is emitted because the three shipped libraries declare it. It says
/// nothing useful about a Rust field, and it is kept only so that adopting this macro
/// leaves the declared type identical.
///
/// ```
/// # struct Document;
/// uncore::export_handle! {
///     /// Opaque handle to a parsed document.
///     handle DemoDoc { inner: Document },
///
///     /// Free a document handle.
///     ///
///     /// # Safety
///     ///
///     /// - `doc` must be a pointer a parse function returned, or null.
///     /// - After this call the handle is invalid and must not be used.
///     free demo_release_document,
/// }
/// ```
#[macro_export]
macro_rules! export_handle {
    (
        $(#[$handle_meta:meta])*
        handle $handle:ident { $field:ident: $inner:ty },

        $(#[$free_meta:meta])*
        free $free_fn:ident $(,)?
    ) => {
        $(#[$handle_meta])*
        #[repr(C)]
        pub struct $handle {
            $field: $inner,
        }

        $(#[$free_meta])*
        #[no_mangle]
        pub unsafe extern "C" fn $free_fn(doc: *mut $handle) {
            if !doc.is_null() {
                // Wrapped explicitly rather than relying on the unsafe fn body: this
                // expands both in crates that deny `unsafe_op_in_unsafe_fn` and in crates
                // that do not, and a block around a genuinely unsafe operation is correct
                // under either.
                let _ = unsafe { ::std::boxed::Box::from_raw(doc) };
            }
        }
    };
}

/// Declare the release function for strings this library hands out.
///
/// Every entry point returning `*mut c_char` transfers ownership, so exactly one of these
/// belongs beside them. Reclaiming the allocation has to happen in the library that made
/// it, which is why it cannot be `free()` on the caller's side.
///
/// ```
/// uncore::export_free_string!(
///     /// Free a string produced by this library.
///     ///
///     /// # Safety
///     ///
///     /// - `s` must be a pointer this library returned, or null.
///     demo_release_string
/// );
/// ```
#[macro_export]
macro_rules! export_free_string {
    ($(#[$meta:meta])* $free_fn:ident) => {
        $(#[$meta])*
        #[no_mangle]
        pub unsafe extern "C" fn $free_fn(s: *mut ::std::ffi::c_char) {
            if !s.is_null() {
                let _ = unsafe { ::std::ffi::CString::from_raw(s) };
            }
        }
    };
}

/// Declare the release function for byte buffers this library hands out.
///
/// The length is taken back because the buffer was a `Box<[u8]>`: reconstructing it needs
/// the same length the producing call reported. A zero length is ignored rather than
/// reclaimed — an empty boxed slice has no allocation to return.
///
/// ```
/// uncore::export_free_bytes!(
///     /// Free a byte buffer produced by this library.
///     ///
///     /// # Safety
///     ///
///     /// - `data` must be a pointer this library returned, or null.
///     /// - `len` must be the length that call reported.
///     demo_release_bytes
/// );
/// ```
#[macro_export]
macro_rules! export_free_bytes {
    ($(#[$meta:meta])* $free_fn:ident) => {
        $(#[$meta])*
        #[no_mangle]
        pub unsafe extern "C" fn $free_fn(data: *mut u8, len: usize) {
            if !data.is_null() && len > 0 {
                let _ = unsafe {
                    ::std::boxed::Box::from_raw(::std::ptr::slice_from_raw_parts_mut(
                        data, len,
                    ))
                };
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_char, CString};

    use crate::ffi::FfiError;

    unsafe fn read(ptr: *const c_char) -> Result<&'static str, FfiError> {
        unsafe { crate::with_c_str!(ptr) }
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

    use std::collections::HashMap;

    use crate::ffi::LastErrorSlot;

    /// Stands in for a library's document. Deliberately not a document: the fields are
    /// only the *shapes* an entry point returns — a string, an absent string, a count,
    /// a byte buffer keyed by name.
    // `body`, `title` and `blobs` are each read by a macro added in Task 3, 4 and 5.
    // Remove this allow at the end of Task 6, when all three have readers.
    #[allow(dead_code)]
    #[derive(Default)]
    struct Demo {
        body: String,
        title: Option<String>,
        blobs: HashMap<String, Vec<u8>>,
    }

    thread_local! {
        static DEMO_ERROR: LastErrorSlot = const { LastErrorSlot::new() };
    }

    crate::export_last_error_abi!(DEMO_ERROR, demo_last_error, demo_last_error_kind);

    crate::export_handle! {
        /// Opaque handle to a demo document.
        handle DemoDocument { inner: Demo },

        /// Free a demo handle.
        ///
        /// # Safety
        ///
        /// - `doc` must be a pointer this test module produced, or null.
        /// - After this call the handle is invalid and must not be used.
        free demo_free_document,
    }

    crate::export_free_string!(
        /// Free a string produced by a demo entry point.
        ///
        /// # Safety
        ///
        /// - `s` must be a pointer a demo entry point returned, or null.
        demo_free_string
    );

    crate::export_free_bytes!(
        /// Free a byte buffer produced by a demo entry point.
        ///
        /// # Safety
        ///
        /// - `data` must be a pointer a demo entry point returned, or null.
        /// - `len` must be the length that call reported.
        demo_free_bytes
    );

    fn demo(inner: Demo) -> *mut DemoDocument {
        Box::into_raw(Box::new(DemoDocument { inner }))
    }

    #[test]
    fn a_handle_round_trips_through_its_free_function() {
        let doc = demo(Demo::default());
        assert!(!doc.is_null());
        // A smoke test, deliberately: a stock `cargo test` run has no leak detector, so
        // this cannot show the allocation was returned. What it does show is that the
        // pair agrees on the pointer — a mismatched type or an extra indirection would
        // abort here.
        unsafe { demo_free_document(doc) };
    }

    #[test]
    fn every_free_function_accepts_null() {
        unsafe {
            demo_free_document(std::ptr::null_mut());
            demo_free_string(std::ptr::null_mut());
            demo_free_bytes(std::ptr::null_mut(), 0);
        }
    }

    /// A zero length must leave the caller's buffer alone.
    ///
    /// This pins the observable contract, not the guard: dropping a zero-length
    /// `Box<[u8]>` would not reach the allocator either, since `Global::deallocate`
    /// skips a zero-size layout. The `len > 0` guard earns its keep elsewhere — it means
    /// no `Box` is ever reconstructed from a pointer at all, which is what a
    /// provenance-checking run would object to.
    #[test]
    fn free_bytes_ignores_a_zero_length_buffer() {
        let mut byte = 7u8;
        unsafe { demo_free_bytes(&mut byte, 0) };
        assert_eq!(byte, 7);
    }
}
