use core::convert::Infallible;
use core::num::NonZeroU32;

use uv_k5_rust_firmware::adc_battery_register_adapter::{
    ReadRegister as MmioReadRegister, WriteRegister as MmioWriteRegister,
};
use uv_k5_rust_firmware::boot_coordinator::{
    self, BatteryReadRegister, BatteryRegisterIo, BatteryWriteRegister, BoardDefines, BootConfig,
    I2cTransport, StartupIo,
};
use uv_k5_rust_firmware::eeprom_i2c_bridge::I2cBitbangTransport;
use uv_k5_rust_firmware::i2c_bitbang_flow::target_gpio::{self, GpioI2cIo, TargetRegisters};
use uv_k5_rust_firmware::scheduler::interrupt;
use uv_k5_rust_firmware::settings_boot_composition::Features as SettingsFeatures;
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{
    DelayMs, RegisterIo as DisplayRegisterIo,
};
use uv_k5_rust_firmware::startup_version_display::LCD_WIDTH;
use uv_k5_rust_firmware::systick_delay_adapter::{SysTickDelay, VolatileSysTickCounter};

type Delay = SysTickDelay<VolatileSysTickCounter>;
type I2cBus = GpioI2cIo<TargetRegisters, Delay>;

#[inline(never)]
pub unsafe fn systick_interrupt() {
    // SAFETY: The only caller is the installed SysTick exception handler.
    unsafe { interrupt::on_systick() };
}

struct TargetBootIo {
    board: boot_coordinator::TargetBoardHardwareIo,
    bus: Option<I2cBus>,
    ticks_per_microsecond: NonZeroU32,
}

impl TargetBootIo {
    /// # Safety
    ///
    /// The caller must satisfy `TargetBoardHardwareIo::new` and the target
    /// GPIO I²C backend's DP32G030 mapping and exclusive pin/register ownership
    /// requirements. `ticks_per_microsecond` must come from verified clock
    /// calibration; construction itself performs no MMIO.
    unsafe fn new(ticks_per_microsecond: NonZeroU32) -> Self {
        Self {
            // SAFETY: This constructor forwards its complete target ownership
            // contract to the board backend.
            board: unsafe { boot_coordinator::TargetBoardHardwareIo::new() },
            bus: None,
            ticks_per_microsecond,
        }
    }

    fn with_i2c<T>(&mut self, operation: impl FnOnce(&mut I2cBus) -> T) -> T {
        if self.bus.is_none() {
            // SAFETY: The board sequence configures GPIOA/PA10/PA11 before
            // the first EEPROM transaction. The bus owns these pins until the
            // explicit handoff after settings acquisition.
            let delay = unsafe {
                SysTickDelay::new(VolatileSysTickCounter::new(), self.ticks_per_microsecond)
            };
            self.bus = Some(unsafe { GpioI2cIo::new_target(delay) });
        }
        operation(self.bus.as_mut().expect("I2C bus initialized"))
    }
}

impl StartupIo for TargetBootIo {
    type FmError = Infallible;

    fn device_clock_gates(&mut self) {
        self.board.device_clock_gates();
    }

    fn systick(
        &mut self,
        period_ticks: u32,
    ) -> Result<(), boot_coordinator::BoardStartupError<Self::FmError>> {
        self.board.systick(period_ticks)
    }

    fn portcon(&mut self, enable_swd: bool) {
        self.board.portcon(enable_swd);
    }

    fn gpio(&mut self, enable_fmradio: bool) {
        self.board.gpio(enable_fmradio);
    }

    fn backlight_init_hardware(&mut self) {
        self.board.backlight_init_hardware();
    }

    fn adc_init(&mut self, exttrig_sel: u16) {
        self.board.adc_init(exttrig_sel);
    }

    fn st7565_init(
        &mut self,
        delay: &mut dyn DelayMs,
        fifo_poll_limit: NonZeroU32,
    ) -> Result<(), uv_k5_rust_firmware::st7565_line_transport::LineError> {
        self.board.st7565_init(delay, fifo_poll_limit)
    }

    fn bk1080_shutdown(
        &mut self,
        delay: &mut dyn target_gpio::Delay1us,
    ) -> Result<(), uv_k5_rust_firmware::bk1080_startup_adapter::ShutdownError<Self::FmError>> {
        self.board.bk1080_shutdown(delay)
    }

    fn crc_init(&mut self) {
        self.board.crc_init();
    }
}

impl I2cTransport for TargetBootIo {
    type Error = Infallible;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.with_i2c(|bus| I2cBitbangTransport::new(bus).start())
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        self.with_i2c(|bus| I2cBitbangTransport::new(bus).repeated_start())
    }

    fn write_byte(
        &mut self,
        byte: u8,
    ) -> Result<uv_k5_rust_firmware::eeprom_read_transport::Acknowledgement, Self::Error> {
        self.with_i2c(|bus| I2cBitbangTransport::new(bus).write_byte(byte))
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        self.with_i2c(|bus| I2cBitbangTransport::new(bus).read_buffer(output))
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.with_i2c(|bus| I2cBitbangTransport::new(bus).stop())
    }
}

impl BatteryRegisterIo for TargetBootIo {
    fn read(&mut self, register: BatteryReadRegister) -> u32 {
        let register = match register {
            BatteryReadRegister::Start => MmioReadRegister::Start,
            BatteryReadRegister::Channel9Status => MmioReadRegister::Channel9Status,
            BatteryReadRegister::Channel4Data => MmioReadRegister::Channel4Data,
            BatteryReadRegister::Channel9Data => MmioReadRegister::Channel9Data,
        };
        let address =
            uv_k5_rust_firmware::adc_battery_register_adapter::read_register_address(register)
                .address();
        // SAFETY: The boot sequence owns the configured DP32G030 SARADC.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write(&mut self, register: BatteryWriteRegister, value: u32) {
        let register = match register {
            BatteryWriteRegister::Start => MmioWriteRegister::Start,
            BatteryWriteRegister::InterruptFlags => MmioWriteRegister::InterruptFlags,
        };
        let address =
            uv_k5_rust_firmware::adc_battery_register_adapter::write_register_address(register)
                .address();
        // SAFETY: The boot sequence owns the configured DP32G030 SARADC.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

impl DisplayRegisterIo for TargetBootIo {
    fn read(&mut self, address: usize) -> u32 {
        // SAFETY: Board startup established the DP32G030 display mapping.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write(&mut self, address: usize, value: u32) {
        // SAFETY: Board startup established exclusive SPI0/display ownership.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

impl boot_coordinator::BootIo for TargetBootIo {
    fn i2c_handoff(&mut self) {
        if let Some(bus) = self.bus.take() {
            bus.handoff_to_keypad();
        }
    }
}

fn policy() -> Option<(BootConfig, NonZeroU32)> {
    let nonzero_u32 = |value: &str| value.parse::<u32>().ok().and_then(NonZeroU32::new);
    let exttrig_sel = env!("UVK5_POLICY_ADC_EXTTRIG_SEL").parse::<u16>().ok()?;
    let battery_calibration = env!("UVK5_POLICY_BATTERY_CALIBRATION")
        .parse::<u16>()
        .ok()?;
    let config = BootConfig {
        board: BoardDefines {
            enable_fmradio: env!("UVK5_POLICY_ENABLE_FMRADIO") == "true",
            enable_uart: env!("UVK5_POLICY_ENABLE_UART") == "true",
            enabled_aircopy: env!("UVK5_POLICY_ENABLED_AIRCOPY") == "true",
        },
        enable_swd: env!("UVK5_POLICY_ENABLE_SWD") == "true",
        exttrig_sel,
        st7565_fifo_poll_limit: nonzero_u32(env!("UVK5_POLICY_ST7565_FIFO_POLL_LIMIT"))?,
        settings: SettingsFeatures {
            noaa_enabled: env!("UVK5_POLICY_ENABLE_NOAA") == "true",
            vox_enabled: env!("UVK5_POLICY_ENABLE_VOX") == "true",
            blmin_tmp_off_enabled: env!("UVK5_POLICY_ENABLE_BLMIN_TMP_OFF") == "true",
        },
        battery_poll_budget: usize::try_from(
            nonzero_u32(env!("UVK5_POLICY_BATTERY_POLL_BUDGET"))?.get(),
        )
        .ok()?,
        battery_calibration,
    };
    let ticks_per_microsecond = nonzero_u32(env!("UVK5_POLICY_SYSTICK_TICKS_PER_MICROSECOND"))?;
    Some((config, ticks_per_microsecond))
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn firmware_integration_entry() -> ! {
    let Some((config, ticks_per_microsecond)) = policy() else {
        firmware_fail_closed();
    };
    // SAFETY: The selected policy is required at build time and carries the
    // externally verified target mapping, ownership, and timing contract.
    let mut io = unsafe { TargetBootIo::new(ticks_per_microsecond) };
    // SAFETY: These providers use the documented Cortex-M0 SysTick registers;
    // board startup configures the counter before any delay is requested.
    let mut display_delay =
        unsafe { SysTickDelay::new(VolatileSysTickCounter::new(), ticks_per_microsecond) };
    let mut fm_delay =
        unsafe { SysTickDelay::new(VolatileSysTickCounter::new(), ticks_per_microsecond) };
    let mut pages = [[0; LCD_WIDTH]; 8];
    if boot_coordinator::coordinate_with_welcome(
        &mut io,
        &mut display_delay,
        &mut fm_delay,
        config,
        Some(env!("CARGO_PKG_VERSION")),
        &mut pages,
    )
    .is_err()
    {
        firmware_fail_closed();
    }
    firmware_fail_closed();
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn firmware_fail_closed() -> ! {
    // No application loop is implemented. Mask delivery and park after either
    // a completed welcome or the first startup failure.
    unsafe { core::arch::asm!("cpsid i", options(nomem, nostack, preserves_flags)) }
    loop {
        unsafe { core::arch::asm!("wfi", options(nomem, nostack, preserves_flags)) }
    }
}
