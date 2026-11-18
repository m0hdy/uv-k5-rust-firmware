#include <stdio.h>
#include <string.h>

#include "dcs.h"
#include "frequencies.h"
#include "settings_capture_support.h"

EEPROM_Config_t gEeprom;
bool gSetting_350TX;
bool gSetting_200TX;
bool gSetting_500TX;
bool gSetting_350EN;
bool gSetting_ScrambleEnable;
bool gSetting_KILLED;
bool gSetting_live_DTMF_decoder;
uint8_t gSetting_battery_text;
uint8_t gSetting_F_LOCK;
bool gSetting_mic_bar;
bool gSetting_AM_fix;
enum BacklightOnRxTx_t gSetting_backlight_on_tx_rx;

static uint8_t eeprom[UINT16_MAX + 1u];
static uint16_t write_addresses[16];
static size_t write_count;

void EEPROM_WriteBuffer(uint16_t address, const void *data)
{
	memcpy(&eeprom[address], data, 8);
	write_addresses[write_count++] = address;
}

static void emit_settings(const char *profile)
{
	for (size_t i = 0; i < write_count; i++) {
		printf("settings\t%s\t0x%04x\t", profile, write_addresses[i]);
		for (size_t j = 0; j < 8; j++)
			printf("%02x", eeprom[write_addresses[i] + j]);
		putchar('\n');
	}
}

static void capture_settings(const char *profile, bool boundary)
{
	memset(&gEeprom, 0, sizeof(gEeprom));
	memset(eeprom, 0, sizeof(eeprom));
	write_count = 0;

	gEeprom.CHAN_1_CALL = boundary ? 199 : 7;
	gEeprom.SQUELCH_LEVEL = boundary ? 9 : 5;
	gEeprom.TX_TIMEOUT_TIMER = boundary ? 10 : 4;
	gEeprom.KEY_LOCK = boundary;
	gEeprom.VOX_SWITCH = boundary;
	gEeprom.VOX_LEVEL = boundary ? 9 : 3;
	gEeprom.MIC_SENSITIVITY = boundary ? 4 : 2;
	gEeprom.BACKLIGHT_MIN = boundary ? 9 : 2;
	gEeprom.BACKLIGHT_MAX = boundary ? 10 : 8;
	gEeprom.CHANNEL_DISPLAY_MODE = boundary ? 3 : 2;
	gEeprom.CROSS_BAND_RX_TX = boundary ? 2 : 1;
	gEeprom.BATTERY_SAVE = boundary ? 4 : 3;
	gEeprom.DUAL_WATCH = boundary ? 2 : 1;
	gEeprom.BACKLIGHT_TIME = boundary ? 9 : 5;
	gEeprom.TAIL_TONE_ELIMINATION = boundary;
	gEeprom.VFO_OPEN = !boundary;
	gEeprom.BEEP_CONTROL = boundary;
	gEeprom.KEY_M_LONG_PRESS_ACTION = boundary ? 14 : 6;
	gEeprom.KEY_1_SHORT_PRESS_ACTION = boundary ? 14 : 3;
	gEeprom.KEY_1_LONG_PRESS_ACTION = boundary ? 14 : 4;
	gEeprom.KEY_2_SHORT_PRESS_ACTION = boundary ? 14 : 5;
	gEeprom.KEY_2_LONG_PRESS_ACTION = boundary ? 14 : 7;
	gEeprom.SCAN_RESUME_MODE = boundary ? 2 : 1;
	gEeprom.AUTO_KEYPAD_LOCK = boundary;
	gEeprom.POWER_ON_DISPLAY_MODE = boundary ? 3 : 2;
	gEeprom.S0_LEVEL = boundary ? 150 : 130;
	gEeprom.S9_LEVEL = boundary ? 60 : 76;
	gEeprom.ROGER = boundary ? 2 : 1;
	gEeprom.REPEATER_TAIL_TONE_ELIMINATION = boundary ? 10 : 5;
	gEeprom.TX_VFO = boundary ? 1 : 0;
	gEeprom.BATTERY_TYPE = boundary ? 4 : 2;
	gEeprom.DTMF_SIDE_TONE = boundary;
	gEeprom.DTMF_SEPARATE_CODE = boundary ? 'A' : '*';
	gEeprom.DTMF_GROUP_CALL_CODE = boundary ? 'D' : '#';
	gEeprom.DTMF_DECODE_RESPONSE = boundary ? 3 : 1;
	gEeprom.DTMF_auto_reset_time = boundary ? 60 : 10;
	gEeprom.DTMF_PRELOAD_TIME = boundary ? 1000 : 300;
	gEeprom.DTMF_FIRST_CODE_PERSIST_TIME = boundary ? 1000 : 500;
	gEeprom.DTMF_HASH_CODE_PERSIST_TIME = boundary ? 1000 : 600;
	gEeprom.DTMF_CODE_PERSIST_TIME = boundary ? 1000 : 700;
	gEeprom.DTMF_CODE_INTERVAL_TIME = boundary ? 1000 : 800;
	gEeprom.PERMIT_REMOTE_KILL = boundary;
	gEeprom.SCAN_LIST_DEFAULT = boundary ? 2 : 1;
	gEeprom.SCAN_LIST_ENABLED[0] = !boundary;
	gEeprom.SCANLIST_PRIORITY_CH1[0] = boundary ? 199 : 10;
	gEeprom.SCANLIST_PRIORITY_CH2[0] = boundary ? 199 : 11;
	gEeprom.SCAN_LIST_ENABLED[1] = boundary;
	gEeprom.SCANLIST_PRIORITY_CH1[1] = boundary ? 199 : 12;
	gEeprom.SCANLIST_PRIORITY_CH2[1] = boundary ? 199 : 13;

	gSetting_F_LOCK = boundary ? F_LOCK_NONE : F_LOCK_DEF;
	gSetting_350TX = boundary;
	gSetting_200TX = boundary;
	gSetting_500TX = boundary;
	gSetting_350EN = boundary;
	gSetting_ScrambleEnable = boundary;
	gSetting_live_DTMF_decoder = !boundary;
	gSetting_battery_text = boundary ? 2 : 1;
	gSetting_mic_bar = !boundary;
	gSetting_AM_fix = boundary;
	gSetting_backlight_on_tx_rx = boundary ? BACKLIGHT_ON_TR_TXRX : BACKLIGHT_ON_TR_RX;

	SETTINGS_SaveSettings();
	emit_settings(profile);
}

static void emit_value(const char *category, const char *name, const char *input, int64_t value)
{
	printf("%s\t%s\t%s\t%lld\n", category, name, input, (long long)value);
}

int main(void)
{
	emit_value("ctcss", "first_tone", "tenths_hz=670", DCS_GetCtcssCode(670));
	emit_value("ctcss", "last_tone", "tenths_hz=2541", DCS_GetCtcssCode(2541));
	emit_value("ctcss", "below_range", "tenths_hz=-1", DCS_GetCtcssCode(-1));
	emit_value("ctcss", "above_range", "tenths_hz=9999", DCS_GetCtcssCode(9999));
	emit_value("dcs", "first_normal_word", "type=2,option=0", DCS_GetGolayCodeWord(CODE_TYPE_DIGITAL, 0));
	emit_value("dcs", "last_normal_word", "type=2,option=103", DCS_GetGolayCodeWord(CODE_TYPE_DIGITAL, 103));
	emit_value("dcs", "first_reverse_word", "type=3,option=0", DCS_GetGolayCodeWord(CODE_TYPE_REVERSE_DIGITAL, 0));
	emit_value("dcs", "decode_first_reverse_word", "word=641004", DCS_GetCdcssCode(DCS_GetGolayCodeWord(CODE_TYPE_REVERSE_DIGITAL, 0)));
	emit_value("dcs", "decode_invalid_word", "word=0", DCS_GetCdcssCode(0));
	emit_value("frequency", "rx_below_lower", "ticks=1799999", RX_freq_check(1799999));
	emit_value("frequency", "rx_at_lower", "ticks=1800000", RX_freq_check(1800000));
	emit_value("frequency", "rx_chip_gap_lower", "ticks=63000000", RX_freq_check(63000000));
	emit_value("frequency", "rx_at_chip_gap_upper", "ticks=84000000", RX_freq_check(84000000));
	emit_value("frequency", "rx_at_upper", "ticks=130000000", RX_freq_check(130000000));
	emit_value("frequency", "rx_above_upper", "ticks=130000001", RX_freq_check(130000001));
	gSetting_F_LOCK = F_LOCK_DEF;
	emit_value("frequency", "tx_default_lower", "lock=default,ticks=13700000", TX_freq_check(13700000));
	emit_value("frequency", "tx_default_last_tick", "lock=default,ticks=17399999", TX_freq_check(17399999));
	emit_value("frequency", "tx_default_next_band", "lock=default,ticks=17400000", TX_freq_check(17400000));
	gSetting_F_LOCK = F_LOCK_FCC;
	emit_value("frequency", "tx_fcc_lower", "lock=fcc,ticks=14400000", TX_freq_check(14400000));
	emit_value("frequency", "tx_fcc_upper", "lock=fcc,ticks=14800000", TX_freq_check(14800000));
	gSetting_F_LOCK = F_LOCK_NONE;
	emit_value("frequency", "tx_unlocked_band_edge", "lock=none,ticks=17400000", TX_freq_check(17400000));
	gSetting_F_LOCK = F_LOCK_ALL;
	emit_value("frequency", "tx_all_locked", "lock=all,ticks=14400000", TX_freq_check(14400000));
	emit_value("frequency", "round_half_step", "ticks=14500125,step=250", FREQUENCY_RoundToStep(14500125, 250));
	emit_value("frequency", "round_833_channel", "ticks=14502000,step=833", FREQUENCY_RoundToStep(14502000, 833));

	capture_settings("standard", false);
	capture_settings("boundary", true);
	return 0;
}
