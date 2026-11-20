#![no_std]

pub mod bk1080_startup_adapter;
#[allow(unused_attributes)]
pub mod boot_coordinator;
pub mod channel_codec;
pub mod channel_validity;
#[allow(unused_attributes)]
pub mod display_boot_defaults;
#[allow(unused_attributes)]
pub mod eeprom_i2c_bridge;
#[allow(unused_attributes)]
pub mod eeprom_read_transport;
#[allow(unused_attributes)]
pub mod eeprom_transaction;
pub mod eeprom_welcome_startup;
pub mod firmware_container_checksum;
#[allow(unused_attributes)]
pub mod i2c_bitbang_flow;
pub mod scan_decisions;
pub mod scheduler;
#[allow(unused_attributes)]
pub mod settings_boot_composition;
#[allow(unused_attributes)]
pub mod settings_boot_defaults;
#[allow(unused_attributes)]
pub mod st7565_display;
#[allow(unused_attributes)]
pub mod st7565_line_transport;
#[allow(unused_attributes)]
pub mod st7565_mmio_transport;
#[allow(unused_attributes)]
pub mod st7565_startup;
#[allow(unused_attributes)]
pub mod st7565_startup_executor;
#[allow(unused_attributes)]
pub mod startup_version_display;
#[allow(unused_attributes)]
pub mod startup_version_row_plan;
pub mod tuning_signalling;
#[allow(unused_attributes)]
pub mod welcome_startup;

pub struct ByteQueue<const CAPACITY: usize> {
    bytes: [u8; CAPACITY],
    head: usize,
    len: usize,
}

impl<const CAPACITY: usize> ByteQueue<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            head: 0,
            len: 0,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_full(&self) -> bool {
        self.len == CAPACITY
    }

    pub fn push(&mut self, byte: u8) -> Result<(), u8> {
        if self.is_full() {
            return Err(byte);
        }

        let tail = (self.head + self.len) % CAPACITY;
        self.bytes[tail] = byte;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }

        let byte = self.bytes[self.head];
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        Some(byte)
    }
}

impl<const CAPACITY: usize> Default for ByteQueue<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ByteQueue;

    #[test]
    fn empty_queue_returns_no_byte() {
        let mut queue = ByteQueue::<2>::new();
        assert!(queue.is_empty());
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn full_queue_preserves_existing_bytes() {
        let mut queue = ByteQueue::<2>::new();
        assert_eq!(queue.push(10), Ok(()));
        assert_eq!(queue.push(20), Ok(()));
        assert!(queue.is_full());
        assert_eq!(queue.push(30), Err(30));
        assert_eq!(queue.pop(), Some(10));
        assert_eq!(queue.pop(), Some(20));
    }

    #[test]
    fn queue_preserves_fifo_order_after_wrapping() {
        let mut queue = ByteQueue::<2>::new();
        queue.push(10).unwrap();
        queue.push(20).unwrap();
        assert_eq!(queue.pop(), Some(10));
        queue.push(30).unwrap();
        assert_eq!(queue.pop(), Some(20));
        assert_eq!(queue.pop(), Some(30));
    }

    #[test]
    fn zero_capacity_queue_stays_empty_and_rejects_bytes() {
        let mut queue = ByteQueue::<0>::new();
        assert!(queue.is_empty());
        assert!(queue.is_full());
        assert_eq!(queue.push(10), Err(10));
        assert_eq!(queue.pop(), None);
    }
}
