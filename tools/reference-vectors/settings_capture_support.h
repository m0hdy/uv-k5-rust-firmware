/*
 * Modified, test-only subset of EEPROM_Config_t from upstream settings.h:
 * https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.h
 *
 * Copyright 2023 Dual Tachyon
 * Licensed under the Apache License, Version 2.0; see LICENSE-APACHE-2.0.
 * The upstream declaration has been reduced and modified for host capture.
 */
#ifndef SETTINGS_CAPTURE_SUPPORT_H
#define SETTINGS_CAPTURE_SUPPORT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "frequencies.h"
#include "misc.h"

#define SETTINGS_H

typedef struct {
	uint8_t CHAN_1_CALL;
	uint8_t SQUELCH_LEVEL;
	uint8_t TX_TIMEOUT_TIMER;
	bool KEY_LOCK;
	bool VOX_SWITCH;
	uint8_t VOX_LEVEL;
	uint8_t MIC_SENSITIVITY;
	uint8_t BACKLIGHT_MIN;
	uint8_t BACKLIGHT_MAX;
	uint8_t CHANNEL_DISPLAY_MODE;
	uint8_t CROSS_BAND_RX_TX;
	uint8_t BATTERY_SAVE;
	uint8_t DUAL_WATCH;
	uint8_t BACKLIGHT_TIME;
	bool TAIL_TONE_ELIMINATION;
	bool VFO_OPEN;
	bool BEEP_CONTROL;
	uint8_t KEY_M_LONG_PRESS_ACTION;
	uint8_t KEY_1_SHORT_PRESS_ACTION;
	uint8_t KEY_1_LONG_PRESS_ACTION;
	uint8_t KEY_2_SHORT_PRESS_ACTION;
	uint8_t KEY_2_LONG_PRESS_ACTION;
	uint8_t SCAN_RESUME_MODE;
	bool AUTO_KEYPAD_LOCK;
	uint8_t POWER_ON_DISPLAY_MODE;
	uint8_t S0_LEVEL;
	uint8_t S9_LEVEL;
	uint8_t ROGER;
	uint8_t REPEATER_TAIL_TONE_ELIMINATION;
	uint8_t TX_VFO;
	uint8_t BATTERY_TYPE;
	bool DTMF_SIDE_TONE;
	char DTMF_SEPARATE_CODE;
	char DTMF_GROUP_CALL_CODE;
	uint8_t DTMF_DECODE_RESPONSE;
	uint8_t DTMF_auto_reset_time;
	uint16_t DTMF_PRELOAD_TIME;
	uint16_t DTMF_FIRST_CODE_PERSIST_TIME;
	uint16_t DTMF_HASH_CODE_PERSIST_TIME;
	uint16_t DTMF_CODE_PERSIST_TIME;
	uint16_t DTMF_CODE_INTERVAL_TIME;
	bool PERMIT_REMOTE_KILL;
	uint8_t SCAN_LIST_DEFAULT;
	bool SCAN_LIST_ENABLED[2];
	uint8_t SCANLIST_PRIORITY_CH1[2];
	uint8_t SCANLIST_PRIORITY_CH2[2];
} EEPROM_Config_t;

extern EEPROM_Config_t gEeprom;
extern ChannelAttributes_t gMR_ChannelAttributes[207];
extern bool gSetting_350TX;
extern bool gSetting_200TX;
extern bool gSetting_500TX;
extern bool gSetting_350EN;
extern bool gSetting_ScrambleEnable;
extern bool gSetting_KILLED;
extern bool gSetting_live_DTMF_decoder;
extern uint8_t gSetting_battery_text;
extern uint8_t gSetting_F_LOCK;
extern bool gSetting_mic_bar;
extern bool gSetting_AM_fix;
extern enum BacklightOnRxTx_t gSetting_backlight_on_tx_rx;

void EEPROM_WriteBuffer(uint16_t address, const void *data);
void SETTINGS_SaveSettings(void);
bool RADIO_CheckValidChannel(uint16_t channel, bool checkScanList, uint8_t scanList);

#endif
