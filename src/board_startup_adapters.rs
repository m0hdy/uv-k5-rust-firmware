#![cfg_attr(not(test), no_std)]

#[path = "adc_startup_adapter.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod adc_startup_adapter;
#[path = "backlight_startup_executor.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
pub(crate) mod backlight_startup_executor;
#[path = "board_sequence.rs"]
#[allow(unused_attributes)]
mod board_sequence;
#[path = "device_clock_gate_startup.rs"]
#[allow(dead_code, unused_attributes)]
mod device_clock_gate_startup;
#[path = "gpio_startup_adapter.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod gpio_startup_adapter;
#[path = "portcon_startup_adapter.rs"]
#[allow(dead_code, unused_attributes)]
mod portcon_startup_adapter;
#[path = "systick_startup_adapter.rs"]
#[allow(dead_code, unused_attributes)]
mod systick_startup_adapter;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use adc_startup_adapter::apply_to_hardware as initialize_adc_hardware;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use backlight_startup_executor::initialize_backlight_hardware;
pub use board_sequence::{Call, Defines};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use gpio_startup_adapter::{Features as GpioFeatures, GpioPlan};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use portcon_startup_adapter::Features as PortconFeatures;
use systick_startup_adapter::SYS_TICK_PERIOD_TICKS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupError;

pub trait StartupIo {
    fn device_clock_gates(&mut self);
    fn systick(&mut self, period_ticks: u32) -> Result<(), StartupError>;
    fn portcon(&mut self, enable_swd: bool);
    fn gpio(&mut self, enable_fmradio: bool);
    fn backlight_init_hardware(&mut self);
    fn adc_init(&mut self, exttrig_sel: u16);
    fn placeholder(&mut self, call: Call);
}

struct SequencerBackend<'a, IO> {
    io: &'a mut IO,
    enable_swd: bool,
    enable_fmradio: bool,
    exttrig_sel: u16,
    stopped: bool,
}

impl<IO: StartupIo> board_sequence::Backend for SequencerBackend<'_, IO> {
    fn call(&mut self, call: Call) {
        if self.stopped {
            return;
        }

        match call {
            Call::DeviceClockGates => self.io.device_clock_gates(),
            Call::SystickInit => {
                if self.io.systick(SYS_TICK_PERIOD_TICKS).is_err() {
                    self.stopped = true;
                }
            }
            Call::PortconInit => self.io.portcon(self.enable_swd),
            Call::GpioInit => self.io.gpio(self.enable_fmradio),
            Call::BacklightInitHardware => self.io.backlight_init_hardware(),
            Call::AdcInit => self.io.adc_init(self.exttrig_sel),
            other => self.io.placeholder(other),
        }
    }
}

pub fn run(
    defines: Defines,
    enable_swd: bool,
    exttrig_sel: u16,
    io: &mut impl StartupIo,
) -> Result<(), StartupError> {
    let mut backend = SequencerBackend {
        io,
        enable_swd,
        enable_fmradio: defines.enable_fmradio,
        exttrig_sel,
        stopped: false,
    };
    board_sequence::run(defines, &mut backend);
    if backend.stopped {
        Err(StartupError)
    } else {
        Ok(())
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct HardwareIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl StartupIo for HardwareIo {
    fn device_clock_gates(&mut self) {
        // SAFETY: This witness is only linked, never executed; target integration must uphold
        // the adapter's documented DP32G030 mapping and exclusive startup ownership.
        unsafe { device_clock_gate_startup::apply_device_clock_gates() }
    }

    fn systick(&mut self, period_ticks: u32) -> Result<(), StartupError> {
        // SAFETY: The witness vector installs SysTick_Handler before this enable path.
        unsafe { systick_startup_adapter::apply_to_hardware(period_ticks) }
            .map_err(|_| StartupError)
    }

    fn portcon(&mut self, enable_swd: bool) {
        // SAFETY: This witness is only linked, never executed; target integration must uphold
        // the adapter's documented DP32G030 mapping and exclusive startup ownership.
        unsafe {
            portcon_startup_adapter::apply_portcon_startup_plan(PortconFeatures { enable_swd })
        }
    }

    fn gpio(&mut self, enable_fmradio: bool) {
        let plan = GpioPlan::for_features(GpioFeatures { enable_fmradio });
        // SAFETY: This witness is only linked, never executed; target integration must uphold
        // the adapter's documented DP32G030 mapping and exclusive startup ownership.
        unsafe { gpio_startup_adapter::apply_gpio_startup_plan(&plan) }
    }

    fn backlight_init_hardware(&mut self) {
        // SAFETY: Startup has enabled the PWM clock; the caller must reserve B6 and
        // provide the documented register mapping and exclusive ownership.
        unsafe { initialize_backlight_hardware() }
    }

    fn adc_init(&mut self, exttrig_sel: u16) {
        // SAFETY: The caller must uphold the SARADC mapping, IRQ-handler/vector,
        // and whole-register/interrupt serialization requirements documented below.
        unsafe { initialize_adc_hardware(exttrig_sel) }
    }

    fn placeholder(&mut self, _call: Call) {}
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
/// Runs the target adapters in sequencer order for the link-only witness.
///
/// # Safety
///
/// The caller must satisfy the mapping and exclusive-ownership preconditions
/// documented by the target adapters, reserve B6 for the backlight, and
/// install valid SysTick and SARADC vectors and handlers before this enables
/// their interrupts. Serialize the complete SARADC operation against every
/// writer to its touched SYSCON/SARADC registers and IRQ line. The caller must
/// explicitly select `exttrig_sel`; no production value is inferred here.
/// The link-only witness is deliberately not configured for hardware execution.
pub(super) unsafe fn run_hardware(
    defines: Defines,
    enable_swd: bool,
    exttrig_sel: u16,
) -> Result<(), StartupError> {
    run(defines, enable_swd, exttrig_sel, &mut HardwareIo)
}
