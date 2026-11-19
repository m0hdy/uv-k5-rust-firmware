# Rust/C ABI contract

## Scope and current interface

This contract defines the boundary for software-only Rust/C interoperability on
the Cortex-M0 target described in the [target contract](target-contract.md).
The C reference is pinned to
[`egzumer/uv-k5-firmware-custom` at `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](reference-provenance.md).
It is a source reference, not a component linked into the Rust firmware.

The only Rust symbol currently exported to the linker is:

| Symbol | Rust declaration | Contract |
| --- | --- | --- |
| `firmware_main` | `pub extern "C" fn firmware_main() -> !` with `#[unsafe(no_mangle)]` | ARM-only, no arguments, never returns; called by the assembly `Reset_Handler` after `.data` and `.bss` initialization. It is not a callable C service API. |

No C-callable service functions, shared data structures, or C headers are
implemented in the current Rust firmware. Any added boundary must be listed
explicitly in this document and implemented as a separate, reviewed change.

The separate opt-in `mixed-fallback` witness defines only
`mixed_rust_channel_valid(uint16_t) -> uint8_t` and
`mixed_c_channel_valid(uint16_t) -> uint8_t` to compare a pure predicate. These
fixture-only symbols are not linked into `uv-k5-rust-firmware` and do not add
a production service API or change the default firmware ABI.

## Types and layout

Use only fixed-width integers for scalar values and record fields at the
boundary:

| Rust | C |
| --- | --- |
| `u8`, `i8` | `uint8_t`, `int8_t` |
| `u16`, `i16` | `uint16_t`, `int16_t` |
| `u32`, `i32` | `uint32_t`, `int32_t` |

The only other permitted argument types are explicitly sized byte pointers:
Rust `*const u8`/`*mut u8` and C `const uint8_t *`/`uint8_t *`.

Shared records must use `#[repr(C)]` in Rust and a corresponding C `struct`.
Keep fields in the same order and use only the types above or fixed-size arrays
of them. Do not use Rust `bool`, `char`, `usize`, `isize`, references, slices,
tuples, or default-layout types; their C representations are not part of this
contract. Do not use C `bool`, plain `char`, `int`, `long`, `size_t`, bitfields,
or compiler-dependent packing in shared records. Do not pack records or rely
on implicit padding: make reserved bytes explicit and assert each record's
size, alignment, and field offsets on both sides.

The pinned C build uses `-fshort-enums` in its GCC and Clang configurations
([Makefile](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/Makefile#L186-L202)).
Consequently, a C enum's size can vary with its values and compiler options.
Never pass a C enum or a Rust enum across the boundary, even if the Rust enum
uses `#[repr(C)]`. Represent enum values as `uint32_t`/`u32` constants with
values fixed in the API definition. On input, convert the integer to an
internal Rust enum with an exhaustive checked conversion; reject unknown
values with `UVK5_STATUS_UNSUPPORTED_VALUE`. On output, map each internal
variant explicitly to its specified integer constant. Never transmute an
integer into a Rust enum.

`abi/layout.c` and `abi/layout.rs` are standalone layout witnesses, not
production headers or linked firmware code. They independently assert a
12-byte, 4-byte-aligned record with matching field offsets. The C witness also
asserts that a small C enum is one byte under `-fshort-enums`, demonstrating
why enums are excluded. Run the witnesses without changing the default Rust
build or CI workflow:

```sh
clang --target=arm-none-eabi -mcpu=cortex-m0 -mthumb -ffreestanding \
  -std=c11 -fshort-enums -Wall -Wextra -Werror -fsyntax-only abi/layout.c
rustc --target thumbv6m-none-eabi --crate-type lib --emit=obj \
  abi/layout.rs -o /tmp/uvk5-abi-layout.o
```

These compile-time checks establish the fixture layout only; they do not prove
that a C object and Rust object link together.

## Symbols and visibility

`firmware_main` is the only present C-ABI symbol. The linker entry is the
assembly `Reset_Handler`, which initializes RAM before calling `firmware_main`.
A future exported function must use the C ABI, have one deliberate unmangled
symbol name, and appear in the symbol table in this document with its complete
signature. Keep all other functions and statics free of C-export attributes;
ordinary Rust `pub` visibility does not itself create a C ABI symbol. Do not
export Rust-mangled names or C globals implicitly. C declarations must use
prototypes with explicit argument and return types and match the Rust signature
exactly.

Do not treat the C reference's internal globals, functions, startup labels, or
interrupt vectors as Rust service symbols. Their ownership and startup/link
contract are outside this software-only ABI.

## Ownership and calling rules

Rust implementation state belongs to Rust modules; C callers do not directly
read or write Rust statics. The future `src/abi.rs` module is the sole owner of
exported declarations, ABI record definitions, and integer-to-enum conversions.
Internal modules own their implementation state and are called by this
facade; they do not define additional exports.

Calls are synchronous. For any future pointer-and-length parameter, the caller
retains ownership and guarantees a valid, correctly aligned pointer for the
duration of the call; null is permitted only with a zero length. Input ranges
are read-only, output ranges are writable and must not overlap any other
argument range. The callee does not retain pointers, free caller memory,
transfer allocation ownership, or invoke callbacks. Output buffers are
unchanged when a call reports an error unless that function's entry explicitly
specifies otherwise.

## Status and failure rules

Every future fallible service function returns `int32_t`/`i32`; do not expose
Rust `Result`, `Option`, or panic behavior directly. Status values are stable
API constants:

| Constant | Value | Meaning |
| --- | ---: | --- |
| `UVK5_STATUS_OK` | `0` | Success |
| `UVK5_STATUS_INVALID_ARGUMENT` | `1` | Invalid length or forbidden null/nonzero-length pointer combination |
| `UVK5_STATUS_UNSUPPORTED_VALUE` | `2` | Unknown enum/integer value |
| `UVK5_STATUS_BUSY` | `3` | Operation cannot run in the current state |
| `UVK5_STATUS_INTERNAL_ERROR` | `4` | Other documented implementation failure |

Only zero means success. Callers must treat any unrecognized nonzero status as
failure. Functions must validate arguments and enum integers before changing
state, return a documented status rather than unwind across the ABI, and keep
the output unchanged on failure unless explicitly documented otherwise.

## Evidence and limits

The source provenance and Cortex-M0 target contract are linked above. The
standalone witnesses can verify fixed-record layout independently for both
compilers; their output is not a linked firmware artifact. Mixed-target
linkage/smoke tests are owned by issue #37 and remain gated by issue #29; they
are not claimed here. Startup review, firmware flashing, and per-device tests
are also out of scope for this ABI-only change. No device result or firmware
artifact hash is available from the standalone layout checks.
