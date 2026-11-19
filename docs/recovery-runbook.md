# No-flash preflight and rollback

## Status and scope

**STOP: no recovery image is yet proven on the test radio.** This runbook is
preparation, not permission to flash Rust. A baseline-only recovery attempt in
[#12](https://github.com/m0hdy/uv-k5-rust-firmware/issues/12) may proceed once
its prerequisites below are met; no device recovery is claimed here. The
intended target is the Quansheng UV-K5(8), also
marketed as UV-K6, but the exact board revision, MCU marking, bootloader
version and compatible image for the test radio are not yet established.
No other model, revision, or build configuration is covered by this runbook.
Record and verify these identifiers against the image and updater documentation
for each radio; a model label alone is insufficient. If the revision or its
compatibility cannot be established, stop.

The [pinned C baseline](reference-baseline.md) is an open reimplementation
whose upstream describes the UV-K5/K6/5R family; that description is not a
hardware-revision compatibility list. The [upstream firmware collection](https://github.com/amnemonic/Quansheng_UV-K5_Firmware)
lists distinct K5 and K6 image versions and links the manufacturer support
pages. Do not infer interchangeability or bypass a model/version refusal.

## Recovery kit and prerequisites

1. In #12, select and approve **one exact updater-ready recovery image and
   SHA-256 for the identified model, board/MCU revision, bootloader and
   configuration** for the baseline attempt. Record its source, version,
   configuration, checksum and compatibility evidence, and verify the actual
   file against that checksum before the attempt. This approval is not proof
   that the image restores this particular radio: that proof requires the #12
   re-flash, cold boot and checks below. Keep the actual file, approved updater
   and required drivers on an offline-capable machine or offline media; verify
   the file hash against the approved record before each upload. If the image,
   compatibility evidence or offline updater is missing, stop.
2. The only recorded C build is the default configuration at commit
   `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`: updater-formatted
   `firmware.packed.bin`, SHA-256
   `bf24474da3607610c0f213827a01daa54b478c58beac950f7cd3238dc57592ae`.
   See the [build record](reference-baseline.md#results). It is an
   **unqualified candidate, not an approved or device-proven recovery
   image**: reproducibility does not establish compatibility or restoration on
   this radio. Do not approve it merely because its hash is recorded. Select
   and approve the actual artifact for the baseline attempt in #12.
3. Use a supported serial updater for the selected image and exact radio. The
   [original C upstream](https://github.com/DualTachyon/uv-k5-firmware#flashing-with-the-official-updater)
   specifies `firmware.packed.bin` for the official updater; the
   [pinned C upstream](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/README.md#github-codespace-build-method)
   also points to its browser flasher. Neither statement proves acceptance on
   this device. Choose and record the exact updater name, version and source;
   verify that it and its dependencies work offline before bench work. An
   online-only flasher is not an offline recovery kit.
4. Use a compatible, firmly seated Baofeng/Kenwood-style USB-to-serial
   programming cable, the correct port and driver, and a fully charged,
   healthy battery or manufacturer-supported stable supply. Secure the cable
   and power for the entire operation; do not disconnect or interrupt power
   during an upload. Stop for uncertain power, cable, port, image compatibility
   or updater behavior.
5. Before any upload, back up settings and device-specific EEPROM/calibration
   using a documented tool and version known to support the installed firmware
   and this exact radio. Record backup scope, read completeness, integrity and
   a compatible restore procedure; keep encrypted/access-controlled copies
   offline. Verify the backup can be read and that the restore method and
   covered ranges are understood. The reverse-engineered
   [configuration map](https://github.com/amnemonic/Quansheng_UV-K5_Firmware/blob/94a36006b75ad2024d9b88f2b33b222c7efe53ba/docs/cfg_mem_map.md)
   and [serial tools](https://github.com/amnemonic/Quansheng_UV-K5_Firmware/tree/94a36006b75ad2024d9b88f2b33b222c7efe53ba/python-utils)
   describe settings and calibration, not a proven universal restore method.
   Do not copy calibration between radios or use an unverified full-memory
   write. After recovery, compare settings with the private backup and restore
   only supported, authorized ranges if needed, then read back and cold-boot
   to check persistence. Re-flashing firmware does not itself restore EEPROM.
   Stop if a necessary backup or safe restore procedure is unavailable.

### Standalone bootloader entry

The [upstream flashing instructions](https://github.com/egzumer/uv-k5-firmware-custom/wiki/Flashing-the-firmware)
describe switching the radio off, holding PTT while switching it on, checking
that the flashlight illuminates, and firmly inserting the serial cable. This
power-on key path does not require an application menu or command. Separately
verify that the chosen updater detects the bootloader on the actual radio,
recording both the LED indication and updater detection. Require separate
evidence for this device and bootloader that entry and detection do not depend
on the application; link the evidence in the bench record. An LED alone is not
proof of detection or recovery, and a documented key sequence is not proof
that it works with a non-running application. Stop if either check or the
independence evidence is missing.

## Exact Rust candidate gate

Before any Rust upload, retain the exact source commit, tool versions, ELF,
map, raw payload, packed container, reports and individual SHA-256 hashes.
Use the [documented image gate](target-contract.md#automated-image-budget-gate)
and [offline reproduction](offline-image-evidence.md#reproduction) on that
same set of files. The CI development-image job runs
[`check_firmware_image.py`](../scripts/check_firmware_image.py) and records
individual ELF, map, raw and packed SHA-256 values in its summary. Compare
the hashes of the actual files selected for the bench against the reviewed,
approved exact-candidate record; stop on any mismatch. Hash recording alone
is not an automated approval or an identity comparison.

The gate checks startup/ELF/map layout, the exact file-backed ELF load bytes
against the raw payload including padding, application flash bounds
`[0x00000000, 0x0000F000)`, RAM/stack budgets, and packed metadata, CRC and
unpacked payload equality. The reserved tail `[0x0000F000, 0x00010000)` is a
conservative contract, not a proven physical bootloader boundary. Layout and
container checks **do not** prove that runtime code or the updater cannot
damage recovery. The [existing bootstrap evidence](offline-image-evidence.md)
is explicitly non-flashable and cannot qualify a later candidate.

Require a reviewable, exact-candidate audit of the Rust application, linked
dependencies and update process, including flash-controller access, indirect
calls, erase granularity, address mapping and updater behavior on the
identified device. The audit must establish that neither application nor
update process writes or erases outside the permitted application region,
changes the bootloader, option bytes, debug or readout protection, or passes a
raw or witness artifact to the updater. Only the approved packed candidate is
an updater input; witnesses and raw payloads are never updater inputs. Cite
the code/tool versions, findings, reviewer and evidence. If any property
cannot be established, **stop** and open separately tracked audit gaps with
owners and evidence requirements; link them in the bench record and resolve
them before proceeding. No runtime or updater safety audit is established by
this runbook. Do not use forced flashing, bootloader replacement, mass erase,
or debug-probe recovery as routine alternatives.

## Bench stop/go sequence

Perform one radio session at a time. Every check required for the current
stage must pass; failure, unrun, uncertain identity, hash mismatch, rejected
image or unexpected behavior means stop, preserve evidence and do not proceed.
Do not retry with a forced or different procedure to bypass a failure.

1. For the baseline-only #12 attempt, complete the recovery kit, backup,
   standalone entry/detection and approved updater/recovery path prerequisites
   above. Select and approve the recovery artifact in #12, recording its
   identity, compatibility evidence and verified hash. With any prerequisite
   missing, baseline flashing is blocked. No Rust candidate or Rust candidate
   audit is required for #12.
2. In [#12](https://github.com/m0hdy/uv-k5-rust-firmware/issues/12), before
   **any** Rust upload, use the approved updater to re-flash that exact
   recovery image. Record detection and upload result; disconnect only after
   completion, power fully off, cold boot, and check display, keys, receive and
   audio with a controlled signal, channel/frequency readback and settings.
   Verify settings recovery separately as above. Record results in #12; only a
   pass establishes restoration of that image on this radio. The same radio,
   image hash, updater and path must pass before any Rust upload.
3. Before #59 and any Rust upload, complete the exact Rust candidate image
   checks and application/update-process audit above. After those gates pass,
   conduct the separately authorized staged attempt in
   [#59](https://github.com/m0hdy/uv-k5-rust-firmware/issues/59).
   Record its packed hash, detection, upload and cold-boot/basic results.
   After the attempt, power off, re-enter and detect the standalone bootloader,
   and restore the **same** recovery image through the same approved updater.
   Cold boot and repeat the basic and settings checks. A successful upload
   alone is not restoration.
4. In #59 record a non-running-application recovery scenario only if it arises
   or can be reproduced without intentional flash corruption or power
   interruption. Otherwise mark it **UNRUN**, not passed; do not manufacture
   the failure. A normal running-app rollback does not prove that scenario.
5. If recovery is unproven or any required check fails, stop: no further Rust
   upload and no public installable release. Track the unresolved scenario or
   failed check separately and obtain the missing evidence before proceeding.

## Redacted evidence template

Publish only the redacted record in #12 or #59. Keep serial numbers, identifying
settings, credentials, backups and unredacted logs private. Use an opaque
test-device ID. Each result is **PASS / FAIL / UNRUN** with a redacted evidence
reference; an unrun check required for that stage is not a pass. Rust-specific
fields may remain UNRUN in the baseline-only #12 record.

| Field | Record |
| --- | --- |
| Session / stage / date / opaque device ID | UNRUN |
| Model, board revision, MCU marking, bootloader version / compatibility source | UNRUN |
| #12 recovery image name, source, version, configuration, compatibility evidence, SHA-256 / approval and offline availability | UNRUN |
| #59 Rust source commit; ELF, map, raw and packed SHA-256 / gate report | UNRUN |
| Updater name, version, source, offline readiness; cable, port and power | UNRUN |
| #59 audit reviewer, exact candidate and updater, evidence / separately tracked gaps | UNRUN |
| Settings/EEPROM backup scope, integrity and safe restore method | UNRUN |
| Standalone bootloader LED, updater detection and independent-entry evidence | UNRUN |
| #12 recovery upload / cold boot / display, keys, receive/audio, readback / device restoration proof | UNRUN |
| #12 settings comparison, restore if needed, readback and persistence | UNRUN |
| #59 Rust detection / upload / cold boot / basic checks | UNRUN |
| #59 post-attempt bootloader detection / same-image upload / cold boot / basic checks | UNRUN |
| #59 settings comparison, restore if needed, readback and persistence | UNRUN |
| #59 non-running-app recovery scenario / safe reproduction basis | UNRUN |
| Overall decision and outstanding issue links | UNRUN |
