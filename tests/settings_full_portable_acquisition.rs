use std::convert::Infallible;
use uv_k5_rust_firmware::eeprom_read_transport::{Acknowledgement, I2cTransport};
use uv_k5_rust_firmware::settings_boot_composition::Features;
use uv_k5_rust_firmware::settings_channel_acquisition::{
    ChannelSelectionBootDefaults, acquire_full_portable_with_transport,
};
use uv_k5_rust_firmware::settings_extended_boot::{
    CHANNEL_ATTRIBUTES_OFFSET, CUSTOM_AES_KEY_OFFSET, DTMF_ANI_OFFSET, DTMF_DOWN_OFFSET,
    DTMF_KILL_OFFSET, DTMF_REVIVE_OFFSET, DTMF_UP_OFFSET, EEPROM_OFFSETS, ExtendedBootFeatures,
    FM_CHANNELS_OFFSET, FM_CONFIG_OFFSET,
};

struct MockTransport {
    responses: Vec<Vec<u8>>,
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
    let mut responses = vec![vec![0; 8]; 20];
    responses[2] = vec![5, 205, 200, 6, 4, 203, 207, 216];
    responses[3] = vec![100, 0, 0, 1];
    let mut fm_channels = vec![0xff; 40];
    fm_channels[0] = 900_u16 as u8;
    fm_channels[1] = (900_u16 >> 8) as u8;
    responses[4] = fm_channels;
    responses[5] = vec![1, 2, 3, 4, 5, 2, 1, 3];
    responses[6] = vec![1, 2, 3, 4, 5, 6, 7, 8];
    responses[7] = vec![2, 130, 76, 0, 0, 0, 0, 0];
    responses[8] = vec![0, 1, 2, 0, 0, 0, 0, 0];
    responses[9] = vec![1, b'*', b'#', 2, 10, 30, 40, 50];
    responses[10] = vec![60, 70, 0, 0, 0, 0, 0, 0];
    responses[11] = b"123\0\0\0\0\0".to_vec();
    responses[12] = b"ABCD9\0\0\0".to_vec();
    responses[13] = b"9DCBA\0\0\0".to_vec();
    responses[14] = b"123456789ABCDEF*".to_vec();
    responses[15] = b"54321\0\0\0\0\0\0\0\0\0\0\0".to_vec();
    responses[16] = vec![2, 1, 10, 11, 0, 12, 13, 0];
    responses[17] = vec![0, 1, 0, 1, 0, 1, 0, 0];
    let mut attributes = vec![0xff; 207];
    attributes[1] = 0xe3;
    responses[18] = attributes;
    responses[19] = vec![0xff; 16];
    responses[19][0] = 0;
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
            fm_enabled: true,
            voice_enabled: false,
            rssi_bar_enabled: true,
            alarm_enabled: false,
            dtmf_calling_enabled: true,
            audio_bar_enabled: false,
            am_fix_enabled: false,
            lock_mode_count: 8,
        },
    )
    .unwrap();

    let mut expected_offsets = vec![0x0e70, 0x0e78, 0x0e80];
    expected_offsets.extend([FM_CONFIG_OFFSET, FM_CHANNELS_OFFSET]);
    expected_offsets.extend(EEPROM_OFFSETS[..6].iter().copied());
    expected_offsets.extend([DTMF_ANI_OFFSET, DTMF_KILL_OFFSET, DTMF_REVIVE_OFFSET]);
    expected_offsets.extend([DTMF_UP_OFFSET, DTMF_DOWN_OFFSET]);
    expected_offsets.extend(EEPROM_OFFSETS[6..].iter().copied());
    expected_offsets.push(CHANNEL_ATTRIBUTES_OFFSET);
    expected_offsets.push(CUSTOM_AES_KEY_OFFSET);
    assert_eq!(bus.read_offsets, expected_offsets);
    assert_eq!(acquired.fm.unwrap().playing_frequency, 900);
    assert_eq!(
        acquired.base.channel_selection,
        ChannelSelectionBootDefaults {
            screen_channel: [0, 4],
            mr_channel: [0, 4],
            freq_channel: [200, 203],
            noaa_channel: Some([207, 216]),
        }
    );
    assert_eq!(acquired.extended.power_on_password, [1, 2, 3, 4]);
    assert_eq!(acquired.extended.dtmf.preload_ms, 300);
    assert_eq!(&acquired.dtmf_codes.ani.unwrap().bytes[..3], b"123");
    assert_eq!(&acquired.dtmf_codes.kill.unwrap().bytes[..5], b"ABCD9");
    assert_eq!(&acquired.dtmf_codes.up.bytes[..5], b"12345");
    assert_eq!(acquired.extended.scan_lists.enabled, [true, false]);
    assert_eq!(acquired.channel_attributes[0].band(), 0x0f);
    assert_eq!(acquired.channel_attributes[1].band(), 3);
    assert_eq!(acquired.channel_attributes[1].compander(), 2);
    assert!(acquired.channel_attributes[1].scan_list_1());
    assert!(acquired.channel_attributes[1].scan_list_2());
    assert!(acquired.custom_aes_key.present);
    assert_eq!(acquired.custom_aes_key.bytes[0], 0);
    assert_eq!(acquired.custom_aes_key.bytes[15], 0xff);
}
