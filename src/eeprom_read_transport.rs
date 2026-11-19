#![cfg_attr(not(test), no_std)]

//! Allocation-free execution of the existing EEPROM read plan.

pub use crate::eeprom_transaction::{Command, ReadPlan, plan_read};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acknowledgement {
    Ack,
    Nack,
}

/// Fallible bus operations required by the read executor.
///
/// Methods must not panic. A failed initial `start` must leave no transaction
/// active; a failed `repeated_start` may leave the existing transaction active.
/// `read_buffer` fills the entire slice on success and may modify any part on
/// failure. It must accept an empty slice without reading a byte. For nonempty
/// reads, acknowledge all bytes except the final byte, which is not acknowledged.
/// Implementations must not retry transfers.
pub trait I2cTransport {
    type Error;

    fn start(&mut self) -> Result<(), Self::Error>;
    fn repeated_start(&mut self) -> Result<(), Self::Error>;
    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error>;
    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error>;
    fn stop(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadError<E> {
    DestinationLength { expected: usize, actual: usize },
    NotAcknowledged,
    Transport(E),
}

/// Executes `plan` directly, rejecting a mismatched output length before I/O.
///
/// After a successful initial START, an intermediate error causes exactly one
/// cleanup STOP; a cleanup error is discarded in favor of the original error.
/// A failed initial START is not stopped, and a failed final STOP is not retried.
/// No later planned operation is attempted after failure. The output is not
/// rolled back: on a read error it may be partly modified; on a final STOP error
/// it already contains the complete response. Zero-length plans still execute
/// the full sequence, including an empty buffer read.
pub fn execute_read<T: I2cTransport>(
    transport: &mut T,
    plan: ReadPlan,
    output: &mut [u8],
) -> Result<(), ReadError<T::Error>> {
    let expected = plan
        .commands()
        .iter()
        .map(|command| match command {
            Command::ReadBuffer(size) => usize::from(*size),
            _ => 0,
        })
        .sum();
    if output.len() != expected {
        return Err(ReadError::DestinationLength {
            expected,
            actual: output.len(),
        });
    }

    let mut started = false;
    for command in plan.commands() {
        let result = match *command {
            Command::Start => {
                let result = if started {
                    transport.repeated_start()
                } else {
                    transport.start()
                };
                if result.is_ok() {
                    started = true;
                }
                result.map_err(ReadError::Transport)
            }
            Command::Write(byte) => match transport.write_byte(byte) {
                Ok(Acknowledgement::Ack) => Ok(()),
                Ok(Acknowledgement::Nack) => Err(ReadError::NotAcknowledged),
                Err(error) => Err(ReadError::Transport(error)),
            },
            Command::ReadBuffer(_) => transport.read_buffer(output).map_err(ReadError::Transport),
            Command::Stop => {
                started = false;
                transport.stop().map_err(ReadError::Transport)
            }
            Command::DelayMs(_) => unreachable!("ReadPlan contains no delays"),
        };
        if let Err(error) = result {
            if started {
                let _ = transport.stop();
            }
            return Err(error);
        }
    }
    Ok(())
}
