const VERSION_OFFSET: usize = 0x2000;
const VERSION_SIZE: usize = 16;
const CRC_SIZE: usize = 2;
// Reused from pinned fw-pack.py; see docs/firmware-container-rust-reuse.md.
const OBFUSCATION: [u8; 128] = [
    0x47, 0x22, 0xc0, 0x52, 0x5d, 0x57, 0x48, 0x94, 0xb1, 0x60, 0x60, 0xdb, 0x6f, 0xe3, 0x4c, 0x7c,
    0xd8, 0x4a, 0xd6, 0x8b, 0x30, 0xec, 0x25, 0xe0, 0x4c, 0xd9, 0x00, 0x7f, 0xbf, 0xe3, 0x54, 0x05,
    0xe9, 0x3a, 0x97, 0x6b, 0xb0, 0x6e, 0x0c, 0xfb, 0xb1, 0x1a, 0xe2, 0xc9, 0xc1, 0x56, 0x47, 0xe9,
    0xba, 0xf1, 0x42, 0xb6, 0x67, 0x5f, 0x0f, 0x96, 0xf7, 0xc9, 0x3c, 0x84, 0x1b, 0x26, 0xe1, 0x4e,
    0x3b, 0x6f, 0x66, 0xe6, 0xa0, 0x6a, 0xb0, 0xbf, 0xc6, 0xa5, 0x70, 0x3a, 0xba, 0x18, 0x9e, 0x27,
    0x1a, 0x53, 0x5b, 0x71, 0xb1, 0x94, 0x1e, 0x18, 0xf2, 0xd6, 0x81, 0x02, 0x22, 0xfd, 0x5a, 0x28,
    0x91, 0xdb, 0xba, 0x5d, 0x64, 0xc6, 0xfe, 0x86, 0x83, 0x9c, 0x50, 0x1c, 0x73, 0x03, 0x11, 0xd6,
    0xaf, 0x30, 0xf4, 0x2c, 0x77, 0xb2, 0x7d, 0xbb, 0x3f, 0x29, 0x28, 0x57, 0x22, 0xd6, 0x92, 0x8b,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerError {
    NonAsciiVersion,
    OutputTooSmall,
    TooShort,
    CrcMismatch,
    VersionMarkerMissing,
    VersionMismatch,
}

fn version_field(author: &str, version: &str) -> Result<[u8; VERSION_SIZE], ContainerError> {
    if !author.is_ascii() || !version.is_ascii() {
        return Err(ContainerError::NonAsciiVersion);
    }

    let mut field = [0; VERSION_SIZE];
    for (index, byte) in core::iter::once(b'*')
        .chain(author.bytes())
        .chain(core::iter::once(b' '))
        .chain(version.bytes())
        .take(VERSION_SIZE)
        .enumerate()
    {
        field[index] = byte;
    }
    Ok(field)
}

pub fn crc_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

pub fn pack_payload(
    payload: &[u8],
    author: &str,
    version: &str,
    output: &mut [u8],
) -> Result<usize, ContainerError> {
    let field = version_field(author, version)?;
    let packed_length = payload
        .len()
        .checked_add(VERSION_SIZE + CRC_SIZE)
        .ok_or(ContainerError::OutputTooSmall)?;
    if output.len() < packed_length {
        return Err(ContainerError::OutputTooSmall);
    }

    let insertion_offset = payload.len().min(VERSION_OFFSET);
    let transformed_length = packed_length - CRC_SIZE;
    for (index, byte) in payload[..insertion_offset].iter().enumerate() {
        output[index] = *byte ^ OBFUSCATION[index % OBFUSCATION.len()];
    }
    for (index, byte) in field.iter().enumerate() {
        let output_index = insertion_offset + index;
        output[output_index] = *byte ^ OBFUSCATION[output_index % OBFUSCATION.len()];
    }
    for (index, byte) in payload[insertion_offset..].iter().enumerate() {
        let output_index = insertion_offset + VERSION_SIZE + index;
        output[output_index] = *byte ^ OBFUSCATION[output_index % OBFUSCATION.len()];
    }

    let crc = crc_xmodem(&output[..transformed_length]);
    output[transformed_length] = crc as u8;
    output[transformed_length + 1] = (crc >> 8) as u8;
    Ok(packed_length)
}

pub fn validate_container(
    container: &[u8],
    author: &str,
    version: &str,
) -> Result<(), ContainerError> {
    let field = version_field(author, version)?;
    if container.len() < VERSION_SIZE + CRC_SIZE {
        return Err(ContainerError::TooShort);
    }

    let transformed_length = container.len() - CRC_SIZE;
    let expected_crc = crc_xmodem(&container[..transformed_length]);
    if container[transformed_length] != expected_crc as u8
        || container[transformed_length + 1] != (expected_crc >> 8) as u8
    {
        return Err(ContainerError::CrcMismatch);
    }

    let raw_length = transformed_length - VERSION_SIZE;
    let version_offset = raw_length.min(VERSION_OFFSET);
    let mut actual_field = [0; VERSION_SIZE];
    for (index, byte) in actual_field.iter_mut().enumerate() {
        let packed_index = version_offset + index;
        *byte = container[packed_index] ^ OBFUSCATION[packed_index % OBFUSCATION.len()];
    }
    if actual_field[0] != b'*' {
        return Err(ContainerError::VersionMarkerMissing);
    }
    if actual_field != field {
        return Err(ContainerError::VersionMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{ContainerError, crc_xmodem, pack_payload, validate_container};

    #[test]
    fn crc_xmodem_matches_the_standard_check_value() {
        assert_eq!(crc_xmodem(b"123456789").to_le_bytes(), [0xc3, 0x31]);
    }

    #[test]
    fn version_field_insertion_round_trips_at_the_boundary() {
        for length in [0, 1, 0x1fff, 0x2000, 0x2001] {
            let payload = (0..length)
                .map(|index| index as u8)
                .collect::<std::vec::Vec<_>>();
            let mut packed = std::vec![0; payload.len() + 18];
            let packed_length = pack_payload(&payload, "Golden", "1.0", &mut packed).unwrap();
            packed.truncate(packed_length);

            validate_container(&packed, "Golden", "1.0").unwrap();
            let transformed_length = packed.len() - 2;
            let insertion_offset = length.min(0x2000);
            let mut transformed = packed[..transformed_length]
                .iter()
                .enumerate()
                .map(|(index, byte)| {
                    let mask = super::OBFUSCATION[index % super::OBFUSCATION.len()];
                    byte ^ mask
                })
                .collect::<std::vec::Vec<_>>();
            assert_eq!(
                &transformed[insertion_offset..insertion_offset + 16],
                b"*Golden 1.0\0\0\0\0\0"
            );
            transformed.drain(insertion_offset..insertion_offset + 16);
            assert_eq!(transformed, payload);
        }
    }

    #[test]
    fn tampered_crc_and_mismatched_version_are_rejected() {
        let mut packed = [0; 18];
        let length = pack_payload(b"", "Golden", "1.0", &mut packed).unwrap();

        assert_eq!(
            validate_container(&packed[..length], "Other", "1.0"),
            Err(ContainerError::VersionMismatch)
        );
        packed[0] ^= 1;
        assert_eq!(
            validate_container(&packed[..length], "Golden", "1.0"),
            Err(ContainerError::CrcMismatch)
        );
    }

    #[test]
    fn non_ascii_version_and_small_output_are_rejected() {
        assert_eq!(
            pack_payload(b"", "Auöthor", "1.0", &mut [0; 18]),
            Err(ContainerError::NonAsciiVersion)
        );
        assert_eq!(
            pack_payload(b"", "Golden", "1.0", &mut [0; 17]),
            Err(ContainerError::OutputTooSmall)
        );
    }
}
