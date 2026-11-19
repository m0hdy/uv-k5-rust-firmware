# Startup boot coordinator

`src/boot_coordinator.rs` runs the existing board startup adapter sequence,
reads the core and display settings through the fallible EEPROM composition,
then collects and calibrates four bounded battery samples. It returns one
`ApplicationStartup` value only after all three stages succeed. A board,
EEPROM, or battery failure returns a typed `BootError`; settings or battery
data are not handed to the application on an error.

The supported subsequence is compared with pinned
[`main.c:61–109`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L61-L109):

| Coordinator stage | Relation to pinned C |
| --- | --- |
| Board adapter sequence | Models clock gates, SysTick, `BOARD_Init` calls and optional startup calls from `Main`. The target adapter still represents backlight, ADC, display, and other not-yet-executed board calls as typed placeholders. |
| Fallible settings read | Reads the existing core and display blocks corresponding to the beginning of `SETTINGS_InitEEPROM`; it does not implement the rest of that C routine, build-option writes, or calibration loading. |
| Bounded battery sampling | Runs four bounded channel-4/channel-9 acquisitions with caller-supplied calibration, corresponding to the four-sample loop after radio setup in this region. |

This is not full `Main` parity or a complete boot path. The coordinator omits
the initial `BOARD_ADC_GetBatteryInfo` before EEPROM settings; UART setup and
version output; authentic-device checks; DTMF initialization; the C BK4819
initialization; EEPROM build-option and calibration operations; radio/VFO
configuration; `BATTERY_GetReadings(false)` and its display/charging policy;
AM-fix setup; boot-mode and key-release handling; welcome UI; and application
loop setup. No battery calibration value or poll budget is supplied by
default.

Production use requires concrete implementations of all three I/O boundaries.
The board boundary must execute every required hardware stage (including
currently placeholder calls), with clocks, pin muxes, register mappings,
exclusive startup ownership, and a valid SysTick vector/handler established.
The EEPROM boundary must be backed by the device's initialized I2C transport.
The battery boundary requires the SARADC clock, channels 4 and 9 configured
for software-triggered sampling, exclusive SARADC access, a bounded poll
budget, and a calibration obtained from the appropriate settings/calibration
path. Those prerequisites and hardware behaviors are not validated by the host
test. The default firmware entry point and firmware artifact remain unchanged;
this coordinator is a library integration point, not a claim of hardware-ready
firmware.

The host integration test exercises the actual board sequencer adapter,
fallible EEPROM settings composer, and battery sampler with shared mock event
recording. It checks stage ordering, short-circuit failures, and that no
application startup value is returned early. Focused validation:

```sh
cargo test --locked --test boot_coordinator
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```
