# EEPROM I2C bridge

[`src/eeprom_i2c_bridge.rs`](../src/eeprom_i2c_bridge.rs) adapts the existing
`I2cTransport` read executor to the offline bit-banged `I2cIo` flow. Its
`read_settings_boot` entry reuses the existing startup composition and reads
the core settings block at `0x0e70` followed by the display block at `0x0e78`.
It does not rebuild the EEPROM command plan or change either settings decoder.
The bridge is available from the `uv_k5_rust_firmware::eeprom_i2c_bridge`
library module.

The adapter maps bit-bang ACK/NACK results to the transport's typed
acknowledgement. Each write uses the existing 255-sample ACK flow, and buffer
reads use the existing bit-banged byte reader, which ACKs intermediate bytes
and NACKs the final byte. Repeated START and STOP likewise delegate directly
to the existing pin sequence. The adapter reports `Infallible` for pin I/O:
this means only that the modeled `I2cIo` methods cannot return errors. It does
not demonstrate that physical GPIO operations, bus timing, or the EEPROM
device are reliable; a sampled NACK remains detectable.

The host test uses mock pin operations and SDA samples for two distinct startup
responses, erased data in either block, ACK conversion and read counts, the
START/repeated-START and STOP pin sequences, intermediate ACK/final NACK, and a
first-block NACK with cleanup and no second transaction. It makes no claim
about physical hardware or EEPROM persistence.

From the repository root:

```sh
cargo test --locked --test eeprom_i2c_bridge
cargo clippy --locked --test eeprom_i2c_bridge -- -D warnings
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```
