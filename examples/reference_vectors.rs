use std::error::Error;
use std::io::{self, Read};

use uv_k5_rust_firmware::channel_codec::{SETTINGS_BLOCK_OFFSETS, Settings};
use uv_k5_rust_firmware::channel_validity::{ChannelAttributes, is_valid_channel};
use uv_k5_rust_firmware::tuning_signalling::{
    DcsCodeType, TxLockMode, TxPermissions, ctcss_code, dcs_code_word, decode_dcs_code,
    round_to_step, rx_frequency_check, tx_frequency_check,
};

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn parse_value<T: std::str::FromStr>(
    fields: &[(String, String)],
    key: &str,
) -> Result<T, Box<dyn Error>>
where
    T::Err: Error + 'static,
{
    fields
        .iter()
        .find(|(field, _)| field == key)
        .ok_or_else(|| invalid_input(format!("missing input field {key}")))?
        .1
        .parse()
        .map_err(Into::into)
}

fn parse_bool(fields: &[(String, String)], key: &str) -> Result<bool, Box<dyn Error>> {
    match parse_value::<u8>(fields, key)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_input(format!("{key} must be 0 or 1")).into()),
    }
}

fn parse_assignments(input: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    input
        .split(',')
        .map(|assignment| {
            let (key, value) = assignment
                .split_once('=')
                .ok_or_else(|| invalid_input("invalid key=value input"))?;
            if key.is_empty() || value.is_empty() {
                return Err(invalid_input("empty key or value").into());
            }
            Ok((key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn evaluate_scalar(category: &str, input: &str) -> Result<String, Box<dyn Error>> {
    let fields = parse_assignments(input)?;
    let result = match category {
        "channel" => u8::from(is_valid_channel(
            parse_value::<u16>(&fields, "channel")?,
            ChannelAttributes {
                band: parse_value::<u8>(&fields, "band")?,
                scan_list_1: parse_bool(&fields, "scanlist1")?,
                scan_list_2: parse_bool(&fields, "scanlist2")?,
            },
            parse_bool(&fields, "check")?,
            parse_value::<u8>(&fields, "list")?,
            parse_value::<u8>(&fields, "priority1")?,
            parse_value::<u8>(&fields, "priority2")?,
        ))
        .to_string(),
        "ctcss" => ctcss_code(parse_value::<i32>(&fields, "tenths_hz")?).to_string(),
        "dcs" if fields.iter().any(|(key, _)| key == "word") => {
            decode_dcs_code(parse_value::<u32>(&fields, "word")?).to_string()
        }
        "dcs" => {
            let code_type = match parse_value::<u8>(&fields, "type")? {
                2 => DcsCodeType::Normal,
                3 => DcsCodeType::Reverse,
                _ => return Err(invalid_input("unsupported DCS code type").into()),
            };
            dcs_code_word(code_type, parse_value::<usize>(&fields, "option")?)
                .ok_or_else(|| invalid_input("DCS option is out of range"))?
                .to_string()
        }
        "frequency" if fields.iter().any(|(key, _)| key == "lock") => {
            let lock = match fields
                .iter()
                .find(|(key, _)| key == "lock")
                .map(|(_, value)| value.as_str())
            {
                Some("default") => TxLockMode::Default,
                Some("fcc") => TxLockMode::Fcc,
                Some("none") => TxLockMode::None,
                Some("all") => TxLockMode::All,
                _ => return Err(invalid_input("unsupported transmit lock mode").into()),
            };
            tx_frequency_check(
                parse_value::<u32>(&fields, "ticks")?,
                lock,
                TxPermissions::default(),
            )
            .to_string()
        }
        "frequency" if fields.iter().any(|(key, _)| key == "step") => round_to_step(
            parse_value::<u32>(&fields, "ticks")?,
            parse_value::<u16>(&fields, "step")?,
        )
        .ok_or_else(|| invalid_input("frequency cannot be rounded"))?
        .to_string(),
        "frequency" => rx_frequency_check(parse_value::<u32>(&fields, "ticks")?).to_string(),
        _ => {
            return Err(
                invalid_input(format!("unsupported Rust vector category {category}")).into(),
            );
        }
    };
    Ok(result)
}

fn standard_settings() -> Settings {
    Settings {
        call_channel: 7,
        squelch_level: 5,
        tx_timeout_timer: 4,
        key_lock: false,
        vox_switch: false,
        vox_level: 3,
        mic_sensitivity: 2,
        backlight_min: 2,
        backlight_max: 8,
        channel_display_mode: 2,
        cross_band_rx_tx: 1,
        battery_save: 3,
        dual_watch: 1,
        backlight_time: 5,
        tail_tone_elimination: false,
        vfo_open: true,
        beep_control: false,
        key_m_long_press_action: 6,
        key_1_short_press_action: 3,
        key_1_long_press_action: 4,
        key_2_short_press_action: 5,
        key_2_long_press_action: 7,
        scan_resume_mode: 1,
        auto_keypad_lock: false,
        power_on_display_mode: 2,
        rssi_s0_level: 130,
        rssi_s9_level: 76,
        roger: 1,
        repeater_tail_tone_elimination: 5,
        tx_vfo: 0,
        battery_type: 2,
        dtmf_side_tone: false,
        dtmf_separate_code: b'*',
        dtmf_group_call_code: b'#',
        dtmf_decode_response: 1,
        dtmf_auto_reset_seconds: 10,
        dtmf_preload_ms: 300,
        dtmf_first_code_persist_ms: 500,
        dtmf_hash_code_persist_ms: 600,
        dtmf_code_persist_ms: 700,
        dtmf_code_interval_ms: 800,
        permit_remote_kill: false,
        scan_list_default: 1,
        scan_list_enabled: [true, false],
        scan_list_priority_ch1: [10, 12],
        scan_list_priority_ch2: [11, 13],
        tx_lock_mode: 0,
        tx_350_enabled: false,
        killed: false,
        tx_200_enabled: false,
        tx_500_enabled: false,
        tx_350_unlocked: false,
        scramble_enabled: false,
        live_dtmf_decoder: true,
        battery_text: 1,
        mic_bar: true,
        am_fix: false,
        backlight_on_tx_rx: 2,
    }
}

fn boundary_settings() -> Settings {
    let mut settings = standard_settings();
    settings.call_channel = 199;
    settings.squelch_level = 9;
    settings.tx_timeout_timer = 10;
    settings.key_lock = true;
    settings.vox_switch = true;
    settings.vox_level = 9;
    settings.mic_sensitivity = 4;
    settings.backlight_min = 9;
    settings.backlight_max = 10;
    settings.channel_display_mode = 3;
    settings.cross_band_rx_tx = 2;
    settings.battery_save = 4;
    settings.dual_watch = 2;
    settings.backlight_time = 9;
    settings.tail_tone_elimination = true;
    settings.vfo_open = false;
    settings.beep_control = true;
    settings.key_m_long_press_action = 14;
    settings.key_1_short_press_action = 14;
    settings.key_1_long_press_action = 14;
    settings.key_2_short_press_action = 14;
    settings.key_2_long_press_action = 14;
    settings.scan_resume_mode = 2;
    settings.auto_keypad_lock = true;
    settings.power_on_display_mode = 3;
    settings.rssi_s0_level = 150;
    settings.rssi_s9_level = 60;
    settings.roger = 2;
    settings.repeater_tail_tone_elimination = 10;
    settings.tx_vfo = 1;
    settings.battery_type = 4;
    settings.dtmf_side_tone = true;
    settings.dtmf_separate_code = b'A';
    settings.dtmf_group_call_code = b'D';
    settings.dtmf_decode_response = 3;
    settings.dtmf_auto_reset_seconds = 60;
    settings.dtmf_preload_ms = 1000;
    settings.dtmf_first_code_persist_ms = 1000;
    settings.dtmf_hash_code_persist_ms = 1000;
    settings.dtmf_code_persist_ms = 1000;
    settings.dtmf_code_interval_ms = 1000;
    settings.permit_remote_kill = true;
    settings.scan_list_default = 2;
    settings.scan_list_enabled = [false, true];
    settings.scan_list_priority_ch1 = [199, 199];
    settings.scan_list_priority_ch2 = [199, 199];
    settings.tx_lock_mode = 7;
    settings.tx_350_enabled = true;
    settings.tx_200_enabled = true;
    settings.tx_500_enabled = true;
    settings.tx_350_unlocked = true;
    settings.scramble_enabled = true;
    settings.live_dtmf_decoder = false;
    settings.battery_text = 2;
    settings.mic_bar = false;
    settings.am_fix = true;
    settings.backlight_on_tx_rx = 3;
    settings
}

fn encode_settings(profile: &str) -> Result<String, Box<dyn Error>> {
    let settings = match profile {
        "standard" => standard_settings(),
        "boundary" => boundary_settings(),
        _ => return Err(invalid_input(format!("unsupported settings profile {profile}")).into()),
    };
    let image = settings
        .encode()
        .map_err(|error| invalid_input(format!("settings encoding failed: {error:?}")))?;
    let mut blocks = Vec::new();
    for offset in SETTINGS_BLOCK_OFFSETS {
        let bytes = image
            .block(offset)
            .ok_or_else(|| invalid_input("missing encoded settings block"))?;
        let encoded = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        blocks.push(format!("0x{offset:04x}={encoded}"));
    }
    Ok(blocks.join(","))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut requests = String::new();
    io::stdin().read_to_string(&mut requests)?;
    for (line_number, line) in requests.lines().enumerate() {
        let mut fields = line.splitn(2, '\t');
        let vector_id = fields.next().ok_or_else(|| {
            invalid_input(format!("request {} has no vector ID", line_number + 1))
        })?;
        let category = vector_id
            .split_once('/')
            .map(|(category, _)| category)
            .ok_or_else(|| invalid_input(format!("invalid vector ID {vector_id}")))?;
        let input = fields
            .next()
            .ok_or_else(|| invalid_input(format!("request {vector_id} has no input")))?;
        let actual = if category == "settings" {
            encode_settings(input)?
        } else {
            evaluate_scalar(category, input)?
        };
        println!("{vector_id}\t{actual}");
    }
    Ok(())
}
