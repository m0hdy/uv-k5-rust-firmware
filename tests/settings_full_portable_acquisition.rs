use std::convert::Infallible;
use uv_k5_rust_firmware::eeprom_read_transport::{Acknowledgement, I2cTransport};
use uv_k5_rust_firmware::settings_boot_composition::Features;
use uv_k5_rust_firmware::settings_channel_acquisition::{
    ChannelSelectionBootDefaults, acquire_full_portable_with_transport,
};
use uv_k5_rust_firmware::settings_extended_boot::{EEPROM_OFFSETS, ExtendedBootFeatures};

struct MockTransport {
    responses: Vec<[u8; 8]>,
    read_offsets: Vec<u16>,
    writes: Vec<u8>,
}

impl I2cTransport for MockTransport {
    type Error = Infallible;

    fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error> {
        self.writes.push(byte);
        Ok(Acknowledgement::Ack)
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        let index = self.read_offsets.len();
        let offset = (u16::from(self.writes[self.writes.len() - 3]) << 8)
            | u16::from(self.writes[self.writes.len() - 2]);
        self.read_offsets.push(offset);
        output.copy_from_slice(&self.responses[index]);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn full_portable_acquisition_reads_and_decodes_the_standard_settings_sequence() {
    let mut responses = vec![[0; 8]; 11];
    responses[2] = [5, 205, 200, 6, 4, 203, 207, 216];
    responses[3] = [1, 2, 3, 4, 5, 2, 1, 3];
    responses[4] = [1, 2, 3, 4, 5, 6, 7, 8];
    responses[5] = [2, 130, 76, 0, 0, 0, 0, 0];
    responses[6] = [0, 1, 2, 0, 0, 0, 0, 0];
    responses[7] = [1, b'*', b'#', 2, 10, 30, 40, 50];
    responses[8] = [60, 70, 0, 0, 0, 0, 0, 0];
    responses[9] = [2, 1, 10, 11, 0, 12, 13, 0];
    responses[10] = [0, 1, 0, 1, 0, 1, 0, 0];
    let mut bus = MockTransport {
        responses,
        read_offsets: Vec::new(),
        writes: Vec::new(),
    };

    let acquired = acquire_full_portable_with_transport(
        &mut bus,
        Features {
            noaa_enabled: true,
            vox_enabled: true,
            blmin_tmp_off_enabled: false,
        },
        ExtendedBootFeatures {
            voice_enabled: false,
            rssi_bar_enabled: true,
            alarm_enabled: false,
            dtmf_calling_enabled: true,
            audio_bar_enabled: false,
            am_fix_enabled: false,
            battery_type_count: 5,
            lock_mode_count: 8,
        },
    )
    .unwrap();

    let mut expected_offsets = vec![0x0e70, 0x0e78, 0x0e80];
    expected_offsets.extend(EEPROM_OFFSETS);
    assert_eq!(bus.read_offsets, expected_offsets);
    assert_eq!(
        acquired.base.channel_selection,
        ChannelSelectionBootDefaults {
            screen_channel: [5, 6],
            mr_channel: [0, 4],
            freq_channel: [200, 203],
            noaa_channel: Some([207, 216]),
        }
    );
    assert_eq!(acquired.extended.power_on_password, [1, 2, 3, 4]);
    assert_eq!(acquired.extended.dtmf.preload_ms, 300);
    assert_eq!(acquired.extended.scan_lists.enabled, [true, false]);
}
