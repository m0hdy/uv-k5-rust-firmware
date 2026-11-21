# Opt-in firmware startup integration

`src/bin/firmware.rs::firmware_main` retains the default spin-only bootstrap.
Enabling Cargo feature `firmware-integration` selects a target-only concrete
path calls the board sequencer, acquires core and display settings plus the
welcome mode through a distinct welcome-extended composition over GPIO I²C,
hands the bus pins back after STOP, samples four real ADC
conversions with the bounded battery executor, then continues on the display
already initialized by board startup. It uses `CARGO_PKG_VERSION`. The
`None` and `FullScreen` welcome modes produce no version-row transfer. On
success, the entry transfers PA10/PA11 to the keypad adapter and starts the
Rust application scheduler. SysTick drives its 10 ms keypad poll and 500 ms
application state transitions.

On the first typed stage error, no later startup or display stage runs. EEPROM
failure performs the transport executor's cleanup STOP and returns the GPIO
pins to keypad mode. The entry then masks interrupt delivery and parks with
`WFI`. Successful startup installs its tick context and then unmasks SysTick
delivery in the application loop. The current loop polls keys and advances
ported state transitions; full foreground radio/display work is not wired.
The default bootstrap remains the default Cargo configuration and does not
perform peripheral effects.

## Required build policy

The feature is accepted only for `thumbv6m-none-eabi`. The build script
requires every variable below, rejects missing, malformed, or out-of-range
values, and has no fallback values:

| Variable | Required value |
| --- | --- |
| `UVK5_ENABLE_SWD` | Explicit `true` or `false`; must be `false` because PB11 is the display reset output. |
| `UVK5_ENABLE_FMRADIO` | Explicit `true` or `false`; controls the board FM pin setup and conditional BK1080 shutdown. |
| `UVK5_ENABLE_UART` | Explicit `true` or `false`; used by the board CRC feature condition. |
| `UVK5_ENABLED_AIRCOPY` | Explicit `true` or `false`; used by the board CRC feature condition. |
| `UVK5_ENABLE_NOAA` | Explicit `true` or `false`; enables the corresponding settings field. |
| `UVK5_ENABLE_VOX` | Explicit `true` or `false`; enables the corresponding settings fields. |
| `UVK5_ENABLE_BLMIN_TMP_OFF` | Explicit `true` or `false`; enables the corresponding display setting. |
| `UVK5_ADC_EXTTRIG_SEL` | Unsigned 16-bit value forwarded unchanged to the SARADC adapter. Zero is accepted as an explicit selection; it is never supplied automatically. |
| `UVK5_ST7565_FIFO_POLL_LIMIT` | Nonzero unsigned 32-bit per-byte polling budget. |
| `UVK5_BATTERY_POLL_BUDGET` | Nonzero unsigned 32-bit per-conversion polling budget. |
| `UVK5_BATTERY_CALIBRATION` | Nonzero unsigned 16-bit battery calibration value. |
| `UVK5_SYSTICK_TICKS_PER_MICROSECOND` | Caller-verified unsigned calibration from 1 through 35,791, supplied to the existing SysTick counter delay provider. The upper bound keeps the 120 ms startup delay within `u32`; it does not establish accurate timing. |

These are required caller inputs, not recommended production values. Build-time
validation proves only that their syntax and numeric ranges are acceptable.
It does not prove the board revision, ADC trigger selection, battery
calibration, clock source, actual core frequency, or elapsed-time accuracy.
The pinned C initializer does not initialize `EXTTRIG_SEL`; neither its
uninitialized field nor a structural-test value may be treated as production
policy. A deployment owner must verify the clock and calibration assumptions
from authoritative board/device evidence before selecting this feature for a
device. If any value is unavailable, do not enable the feature.

## Target ownership and interrupt contract

The selected path relies on the DP32G030 mappings and whole-register ownership
requirements of the existing adapters. Startup is single-threaded; no keypad
scanner or application interrupt work is running. GPIOA, GPIOB, GPIOC, PORTCON,
SYSCON clock gates, PWM, SARADC, SPI0, CRC, and optionally FM transactions
must remain exclusively owned through their respective sequences. In
particular, serialize GPIOA DATA/DIR and PORTA input-enable/open-drain updates
against all other writers, including unrelated-bit read-modify-write users.
The PA10/PA11 bus owner completes every I²C transaction with STOP, then
explicitly restores the board's keypad pin mode before battery sampling.

Reset masks interrupts before RAM initialization or any peripheral effect. The
vector table installs a valid returning `SysTick_Handler`, and board startup
enables SysTick for the existing counter-poll delay provider. Its frequency is
not inferred or measured by this path. Before ADC setup, the adapter disables
the SARADC NVIC line and clears only SARADC interrupt-enable source fields,
preserving unrelated register bits. It does not install an ADC handler, clear
NVIC pending state, or depend on an ADC ISR. Interrupts stay masked through
startup; the application loop enables delivery only after the SysTick handler
and scheduler state are ready. Startup errors remain masked and parked.

## Offline evidence and limits

Host composition tests exercise the real board/settings/battery/welcome
executors through typed recording backends. They verify ordering, typed
first-error propagation, I²C handoff, the exact `M0HDY <package-version>` row
for text modes, and absence of a version-row transfer in `None` and
`FullScreen` modes. They do not emulate the target peripherals.

CI separately builds the selected target entry using explicit offline-only
structural inputs, checks the linked Cortex-M0 `Reset_Handler` ->
`firmware_main` -> integration -> executor call path, validates startup layout,
and runs the memory-budget check. Those inputs are evidence fixtures, not
runtime defaults or device calibration. The selected ELF is not executed,
flashed, packed, published, or treated as a final installable release. User
device validation, bootloader/recovery evidence, final application work, and
the parent issue's completion remain separate acceptance work.
