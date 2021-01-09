//! A platform agnostic Rust driver for the Sensirion SDP8xx differential pressure sensor, based
//! on the [`embedded-hal`](https://github.com/japaric/embedded-hal) traits.
//! Heavily inspired by the [`sgp30 driver by Danilo Bergen`](https://github.com/dbrgn/sgp30-rs)
//!
//! ## The Device
//!
//! The Sensirion SDP8xx is a differential pressure sensor. It has an I2C interface.
//!
//! - [Datasheet](https://www.sensirion.com/fileadmin/user_upload/customers/sensirion/Dokumente/8_Differential_Pressure/Datasheets/Sensirion_Differential_Pressure_Sensors_SDP8xx_Digital_Datasheet.pdf)
//! - [Product Page](https://www.sensirion.com/en/flow-sensors/differential-pressure-sensors/sdp800-proven-and-improved/)
//!
//! ## Usage
//!
//! ### Instantiating
//!
//! Import this crate and an `embedded_hal` implementation, then instantiate
//! the device:
//!
//! ```no_run
//! use linux_embedded_hal as hal;
//!
//! use hal::{Delay, I2cdev};
//! use sdp8xx::Sdp8xx;
//!
//! # fn main() {
//! let dev = I2cdev::new("/dev/i2c-1").unwrap();
//! let address = 0x25;
//! let mut sdp = Sdp8xx::new(dev, address, Delay);
//! # }
//! ```
//!
//! ### Fetching Device Information
//!
//! You can fetch the product id of your sensor:
//!
//! ```no_run
//! use linux_embedded_hal as hal;
//! use hal::{Delay, I2cdev};
//! use sdp8xx::ProductIdentifier;
//! use sdp8xx::Sdp8xx;
//!
//! let dev = I2cdev::new("/dev/i2c-1").unwrap();
//! let mut sdp = Sdp8xx::new(dev, 0x25, Delay);
//! let product_id: ProductIdentifier = sdp.read_product_id().unwrap();
//!
//! ```

#![deny(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

use core::convert::TryFrom;

pub mod product_info;
pub use crate::product_info::*;

pub mod command;
use crate::command::Command;

pub mod sample;
pub use crate::sample::*;

use embedded_hal as hal;
use i2c::read_words_with_crc;

use crate::hal::blocking::delay::{DelayMs, DelayUs};
use crate::hal::blocking::i2c::{Read, Write, WriteRead};
use sensirion_i2c::{crc8, i2c};

/// All possible errors in this crate
#[derive(Debug)]
pub enum Error<E> {
    /// I2C bus error
    I2c(E),
    /// CRC checksum validation failed
    WrongCrc,
    /// Wrong Buffer Size
    WrongBufferSize,
    /// Invalid sensor variant
    InvalidVariant,
}

impl<E, I2cWrite, I2cRead> From<i2c::Error<I2cWrite, I2cRead>> for Error<E>
where
    I2cWrite: Write<Error = E>,
    I2cRead: Read<Error = E>,
{
    fn from(err: i2c::Error<I2cWrite, I2cRead>) -> Self {
        match err {
            i2c::Error::WrongCrc => Error::WrongCrc,
            i2c::Error::WrongBufferSize => Error::WrongBufferSize,
            i2c::Error::I2cWrite(e) => Error::I2c(e),
            i2c::Error::I2cRead(e) => Error::I2c(e),
        }
    }
}

/// Driver for the SDP8xx
#[derive(Debug, Default)]
pub struct Sdp8xx<I2C, D> {
    /// The concrete I2C device implementation.
    i2c: I2C,
    /// The I2C device address.
    address: u8,
    /// The concrete Delay implementation.
    delay: D,
}

impl<I2C, D, E> Sdp8xx<I2C, D>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
    D: DelayUs<u16> + DelayMs<u16>,
{
    /// Create a new instance of the SDP8xx driver.
    pub fn new(i2c: I2C, address: u8, delay: D) -> Self {
        // TODO try to communicate and get parameters from chip
        Sdp8xx {
            i2c,
            address,
            delay,
        }
    }

    /// Destroy driver instance, return I2C bus instance.
    pub fn destroy(self) -> I2C {
        self.i2c
    }

    /// Write an I2C command to the sensor.
    fn send_command(&mut self, command: Command) -> Result<(), Error<E>> {
        self.i2c
            .write(self.address, &command.as_bytes())
            .map_err(Error::I2c)
    }

    /// Write an I2C command and data to the sensor.
    ///
    /// The data slice must have a length of 2 or 4.
    ///
    /// CRC checksums will automatically be added to the data.
    fn send_command_and_data(&mut self, command: Command, data: &[u8]) -> Result<(), Error<E>> {
        if data.len() != 2 && data.len() != 4 {
            return Err(Error::WrongBufferSize);
        }
        let mut buf = [0; 2 /* command */ + 6 /* max length of data + crc */];
        buf[0..2].copy_from_slice(&command.as_bytes());
        buf[2..4].copy_from_slice(&data[0..2]);
        buf[4] = crc8::calculate(&data[0..2]);
        if data.len() > 2 {
            buf[5..7].copy_from_slice(&data[2..4]);
            buf[7] = crc8::calculate(&data[2..4]);
        }
        let payload = if data.len() > 2 {
            &buf[0..8]
        } else {
            &buf[0..5]
        };
        self.i2c.write(self.address, payload).map_err(Error::I2c)
    }

    /// Return the product id of the SDP8xx
    pub fn read_product_id(&mut self) -> Result<ProductIdentifier, Error<E>> {
        let mut buf = [0; 18];
        // Request product id
        self.send_command(Command::ReadProductId0)?;
        self.send_command(Command::ReadProductId1)?;

        self.i2c.read(self.address, &mut buf).map_err(Error::I2c)?;

        ProductIdentifier::try_from(buf).map_err(|_| Error::WrongCrc)
    }

    /// Trigger a differential pressure read without clock stretching.
    /// This function blocks for at least 60 milliseconds to await a result.
    pub fn read_sample_triggered(&mut self) -> Result<Sample, Error<E>> {
        let mut buffer = [0; 9];

        self.send_command(Command::TriggerDifferentialPressureRead)?;

        self.delay.delay_ms(60);

        read_words_with_crc(&mut self.i2c, self.address, &mut buffer)?;

        Sample::try_from(buffer).map_err(|_| Error::WrongCrc)
    }
}


#[cfg(test)]
mod tests {
    use embedded_hal_mock as hal;

    use self::hal::delay::MockNoop as DelayMock;
    use self::hal::i2c::{Mock as I2cMock, Transaction};
    use super::*;

    /// Test the `product_id` function
    #[test]
    fn test_product_id() {
        let data = vec![
            0x03, 0x02, 206, 0x02, 0x01, 105, 0x44, 0x55, 0x00, 0x66, 0x77, 225, 0x88, 0x99, 0x24,
            0xaa, 0xbb, 0xC5,
        ];
        let expectations = [
            Transaction::write(0x25, Command::ReadProductId0.as_bytes()[..].into()),
            Transaction::write(0x25, Command::ReadProductId1.as_bytes()[..].into()),
            Transaction::read(0x25, data.clone()),
        ];
        //println!("{:x}", crc8::calculate(&data[15..17]));
        let mock = I2cMock::new(&expectations);
        let mut sdp = Sdp8xx::new(mock, 0x25, DelayMock);
        let id = sdp.read_product_id().unwrap();
        assert_eq!(
            ProductVariant::Sdp800_125Pa { revision: 0x01 },
            id.product_number
        );
        assert_eq!(0x445566778899aabb, id.serial_number);
        sdp.destroy().done();
    }
}
