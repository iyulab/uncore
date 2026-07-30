# uncore

Shared C-ABI plumbing and error-kind conventions for the `un*` document extraction
family — [unpdf](https://github.com/iyulab/unpdf) (PDF),
[undoc](https://github.com/iyulab/undoc) (DOCX/XLSX/PPTX),
[unhwp](https://github.com/iyulab/unhwp) (HWP/HWPX).

## Why this exists

Those three libraries have the same shape — parse a container, build an intermediate
representation, render Markdown — and expose it the same way: a Rust API, a C ABI, and
C#, Python and WebAssembly bindings over that ABI.

The document parts differ entirely and belong in each library. The plumbing does not:
thread-local last-error storage, catching panics before they cross `extern "C"`, and the
integer space the error classifications live in. That plumbing was written three times.
This crate is where it lives once.

## Scope

Two rules keep it from becoming a dumping ground:

- **No domain content.** No document model, no rendering, no format knowledge. If a type
  would mention a page, a paragraph or a spreadsheet cell, it belongs upstream.
- **No dependencies.** Three published cdylibs link this statically, so a dependency here
  is a dependency in all of them. Everything is `std`.

`std`-only (a `CString` in a `thread_local!`), which suits every consumer including
`wasm32-unknown-unknown`.

## What is in it

- `kind` — the error-kind values the family already shares, and the bands that keep future
  ones from colliding.
- `ffi` — the thread-local last-error slot, the panic guard, and the two boundary failure
  reasons.
- `scaffold` — macros that assemble a C entry point out of those primitives.

`ffi` hands out the materials; `scaffold` is the assembly. A string-returning entry point
is five steps and only one of them mentions the library:

```rust
uncore::export_string_getter!(
    /// The document rendered as Markdown.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid handle.
    /// - The returned string must be freed with `mylib_free_string`.
    LAST_ERROR,
    mylib_to_markdown(doc: MylibDocument, flags: c_int),
    { render(&(*doc).inner, flags).map_err(classify) }
);
```

The macros are split by return type rather than taking a sentinel, because null and `-1`
are not interchangeable and a sentinel that can be passed in can be passed wrong.
Exported names are written in full at the call site: `macro_rules!` cannot build
identifiers, and a symbol should be greppable where it is declared.

## What it gives you

```rust
use std::ffi::c_int;
use uncore::ffi::{self, FfiError, LastErrorSlot};

// The slot lives in your crate, not in uncore. That is what keeps it per-library —
// see below.
thread_local! {
    static LAST_ERROR: LastErrorSlot = const { LastErrorSlot::new() };
}

uncore::export_last_error_abi!(LAST_ERROR, mylib_last_error, mylib_last_error_kind);

/// Returns the page count, or -1 on failure.
#[no_mangle]
pub extern "C" fn mylib_page_count(handle: *const u8) -> c_int {
    let result: Result<c_int, FfiError> = ffi::catch(|| {
        if handle.is_null() {
            return Err(ffi::invalid_argument("handle was null"));
        }
        Ok(7)
    });

    match result {
        Ok(count) => {
            LAST_ERROR.with(|slot| slot.clear());
            count
        }
        Err(error) => {
            LAST_ERROR.with(|slot| slot.set_error(&error));
            -1
        }
    }
}
```

- `ffi::LastErrorSlot` — message and kind, written together so a message is never paired
  with a stale reason.
- `ffi::catch` — turns a panic into `kind::PANIC` instead of undefined behaviour.
- `ffi::FfiError` — `(kind, message)`, carried out of closures so the classification is
  not lost to an early `to_string()`.
- `export_last_error_abi!` — declares `<lib>_last_error` and `<lib>_last_error_kind`.
- `assert_stable_kinds!` — turns an accidental renumbering into a test failure.
- `kind` — the values the family already shares, and bands for future ones.

### Why the slot is yours

Each library ships as its own cdylib, so today each has its own statics. But this crate is
linked into all of them, and a Rust binary depending on two would share one copy — at
which point a single shared static would make `unpdf_last_error()` return whatever the
preceding `undoc` call recorded. Isolation cannot rest on how consumers happen to link, so
the slot is declared in the consuming crate. That makes it structural.

## Error-kind numbering

`kind` exports **only values that are already identical across the family**, and that
restraint is the design: a library adopting a constant whose value differs from what it
ships would be renumbering its public ABI. Above `UNKNOWN_FORMAT` the family already
disagrees — the low numbers were assigned independently, before anyone thought to align
them — so any value published here would be a renumbering request aimed at somebody.

For everything else it offers bands rather than values:

| Range | Meaning |
|---|---|
| `0` | Success. Never a valid kind |
| `1..=17` | Assigned before the convention existed. Frozen where shipped |
| `18..=99` | New reasons that genuinely apply to more than one library |
| `100..=199` | Failures of the ABI call itself (`100` invalid argument, `101` panic, `102` output cannot cross the ABI) |
| `200..` | One band per library, 100 wide — `unpdf` `200..=299`, `undoc` `300..=399`, `unhwp` `400..=499` |

The contract every library in the family promises its consumers:

1. A new reason takes a new number. Existing numbers are never reused or renumbered.
2. An unrecognised value is not an error — treat it as a generic failure and keep the
   number. A newer library stays usable by an older caller precisely because of this.
3. `0` means success, never a reason.
4. Reasons are `#[non_exhaustive]`: match with a `_ =>` arm.

## Versioning

`0.x`. Breaking changes are a normal means of getting the shape right, and the consumers
are versioned independently.

## License

MIT
