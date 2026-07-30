# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-30

### Added
- **`scaffold` module: macros that assemble a C entry point.** `ffi` handed out the
  materials — a last-error slot, a panic guard, two boundary reasons — while assembling
  them stayed a hand-written five-step preamble at every entry point. These macros are
  that assembly: `export_handle!`, `export_string_getter!`,
  `export_optional_string_getter!`, `export_count_getter!`, `export_bytes_getter!`,
  `export_free_string!`, `export_free_bytes!`.

  They are split by return type on purpose. Null and `-1` are not interchangeable
  sentinels, and a single macro told which one to use is a macro that can be told wrong.
- **`export_optional_string_getter!` keeps absence distinct from failure.** A value that
  was never set returns null with the slot left at success; a value that exists but holds
  an interior NUL returns null with `INVALID_OUTPUT`. Both return null, so the kind is
  what separates them — folding the two by defaulting absence to an empty string would
  tell a caller the field is set and blank.
- **`ffi::c_str_utf8` and `with_c_str!` for C string arguments.** `with_c_str!` rejects
  null and non-UTF-8 together, naming the argument in its own message. `c_str_utf8` does
  only the conversion, for an entry point whose failure path writes through an
  out-parameter and so must reject null before it starts producing a result.
- **`kind`: where a serialisation failure is attributed.** Failing to serialise a rendered
  result is a rendering failure — producing output is rendering, and it stays rendering
  when its last step is serialisation. Stated because two libraries had classified it
  differently with nothing to point at.

## [0.1.0] - 2026-07-30

### Added
- Initial release: `ffi::LastErrorSlot`, `ffi::catch`, `ffi::invalid_argument`,
  `ffi::invalid_output`, `export_last_error_abi!`, `assert_stable_kinds!`, and the `kind`
  numbering conventions — bands, the shared values, and the four rules consumers are
  promised.
