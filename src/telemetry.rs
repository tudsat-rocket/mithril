#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::string::{String, ToString};
#[cfg(feature = "std")]
use std::vec::Vec;

use nalgebra::UnitQuaternion;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

// TODO: get this from some kind of parameter storage?
pub const LORA_MESSAGE_INTERVAL: u32 = 25;
pub const LORA_UPLINK_INTERVAL: u32 = 200;
pub const LORA_UPLINK_MODULO: u32 = 100;
pub const SIPHASHER_KEY: [u8; 16] = [0x64, 0xab, 0x31, 0x54, 0x02, 0x8e, 0x99, 0xc5, 0x29, 0x77, 0x2a, 0xf5, 0xba, 0x95, 0x07, 0x06];
#[allow(dead_code)]
pub const FLASH_SIZE: u32 = 32 * 1024 * 1024;
pub const FLASH_HEADER_SIZE: u32 = 4096; // needs to be multiple of 4096

pub use LogLevel::*;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct f8 {
    raw: u8,
}

#[test]
fn test_f8() {
    let values: Vec<f32> = alloc::vec![-0.00001, 0.008, -0.125, 0.5, 1.0, -10.0, 100.0, 1000.0];
    let compressed: Vec<f8> = values.iter().map(|x| (*x).into()).collect();
    let recovered: Vec<f32> = compressed.iter().map(|x| (*x).into()).collect();
    assert_eq!(
        recovered,
        alloc::vec![-0.01953125, 0.015625, -0.125, 0.5, 1.0, -10.0, 96.0, 960.0]
    );
}

impl From<f32> for f8 {
    fn from(x: f32) -> Self {
        let bits = x.to_bits();
        let sign = bits >> 31;
        let exponent = (bits >> 23) & 0xff;
        let fraction = bits & 0x7fffff;

        let expo_small = (((exponent as i32) - 0x80).clamp(-7, 8) + 7) as u32;
        let fraction_small = fraction >> 20;

        let raw = ((sign << 7) | (expo_small << 3) | fraction_small) as u8;
        Self { raw }
    }
}

impl Into<f32> for f8 {
    fn into(self) -> f32 {
        let sign = self.raw >> 7;
        let exponent = (self.raw & 0x7f) >> 3;
        let fraction = self.raw & 0x7;

        let expo_large = ((exponent as i32) - 7 + 0x80) as u32;
        let fraction_large = (fraction as u32) << 20;

        let bits = ((sign as u32) << 31) | (expo_large << 23) | fraction_large;
        f32::from_bits(bits)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum FlightMode {
    Idle = 0,
    HardwareArmed = 1,
    Armed = 2,
    Flight = 3,
    RecoveryDrogue = 4,
    RecoveryMain = 5,
    Landed = 6,
}

impl FlightMode {
    pub fn led_state(self, time: u32) -> (bool, bool, bool) {
        match self {
            FlightMode::Idle => (false, false, true),                       // ( ,  , G)
            FlightMode::HardwareArmed => (true, time % 500 < 250, false),   // (R, y,  )
            FlightMode::Armed => (true, true, false),                       // (R, Y,  )
            FlightMode::Flight => (false, true, false),                     // ( , Y,  )
            FlightMode::RecoveryDrogue => (false, true, true),              // ( , Y, G)
            FlightMode::RecoveryMain => (true, false, true),                // (R,  , G)
            FlightMode::Landed => (false, false, time % 1000 < 500),        // ( ,  , g)
        }
    }
}

impl Default for FlightMode {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum GPSFixType {
    NoFix = 0,
    AutonomousFix = 1,
    DifferentialFix = 2,
    RTKFix = 3,
    RTKFloat = 4,
    DeadReckoningFix = 5,
}

impl Default for GPSFixType {
    fn default() -> Self {
        Self::NoFix
    }
}

impl From<u8> for GPSFixType {
    fn from(x: u8) -> Self {
        match x {
            0 => Self::NoFix,
            1 => Self::AutonomousFix,
            2 => Self::DifferentialFix,
            3 => Self::RTKFix,
            4 => Self::RTKFloat,
            5 => Self::DeadReckoningFix,
            _ => Self::NoFix
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryMain {
    pub time: u32,
    pub mode: FlightMode,
    pub orientation: Option<UnitQuaternion<f32>>,
    pub vertical_speed: f32,
    pub vertical_accel: f32,
    pub vertical_accel_filtered: f32,
    pub altitude_baro: f32,
    pub altitude_max: f32,
    pub altitude: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryMainCompressed {
    pub time: u32, // TODO: combine these two?
    pub mode: FlightMode,
    pub orientation: (u8, u8, u8, u8),
    pub vertical_speed: f8,
    pub vertical_accel: f8,
    pub vertical_accel_filtered: f8,
    pub altitude_baro: u16,
    pub altitude_max: u16,
    pub altitude: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryRawSensors {
    pub time: u32,
    pub gyro: (f32, f32, f32),
    pub accelerometer1: (f32, f32, f32),
    pub accelerometer2: (f32, f32, f32),
    pub magnetometer: (f32, f32, f32),
    pub temperature_baro: f32,
    pub pressure_baro: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryRawSensorsCompressed {
    pub time: u32,
    pub gyro: (f8, f8, f8),
    pub accelerometer1: (f8, f8, f8),
    pub accelerometer2: (f8, f8, f8),
    pub magnetometer: (f8, f8, f8),
    pub temperature_baro: i8,
    pub pressure_baro: u16, // TODO: compress this further
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryDiagnostics {
    pub time: u32,
    pub cpu_utilization: u8,
    pub heap_utilization: u8,
    pub temperature_core: i8,
    pub cpu_voltage: u16,
    /// Battery voltage in mV
    pub battery_voltage: u16,
    /// Voltage behind arm switch in mV
    pub arm_voltage: u16,
    /// Current current draw (hah!) in mA
    pub current: u16,
    ///// Battery capacity consumed in mAh
    //pub consumed: u16,
    pub lora_rssi: u8,
    pub altitude_ground: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryGPS {
    // TODO: compress
    pub time: u32,
    pub fix_and_sats: u8,
    pub hdop: u16,
    pub latitude: [u8; 3],
    pub longitude: [u8; 3],
    pub altitude_asl: u16,
    pub flash_pointer: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryGCS {
    pub time: u32,
    pub lora_rssi: u8,
    pub lora_rssi_signal: u8,
    pub lora_snr: u8,
}

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DownlinkMessage {
    TelemetryMain(TelemetryMain),
    TelemetryMainCompressed(TelemetryMainCompressed),
    TelemetryRawSensors(TelemetryRawSensors),
    TelemetryRawSensorsCompressed(TelemetryRawSensorsCompressed),
    TelemetryDiagnostics(TelemetryDiagnostics),
    TelemetryGPS(TelemetryGPS),
    TelemetryGCS(TelemetryGCS),
    Log(u32, String, LogLevel, String),
    FlashContent(u32, Vec<u8>)
}

impl DownlinkMessage {
    pub fn time(&self) -> u32 {
        match self {
            DownlinkMessage::TelemetryMain(tm) => tm.time,
            DownlinkMessage::TelemetryMainCompressed(tm) => tm.time,
            DownlinkMessage::TelemetryRawSensors(tm) => tm.time,
            DownlinkMessage::TelemetryRawSensorsCompressed(tm) => tm.time,
            DownlinkMessage::TelemetryDiagnostics(tm) => tm.time,
            DownlinkMessage::TelemetryGPS(tm) => tm.time,
            DownlinkMessage::TelemetryGCS(tm) => tm.time,
            DownlinkMessage::Log(t, _, _, _) => *t,
            DownlinkMessage::FlashContent(_, _) => 0,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum UplinkMessage {
    Heartbeat,
    Reboot,
    RebootAuth(u64),
    RebootToBootloader,
    SetFlightMode(FlightMode),
    SetFlightModeAuth(FlightMode, u64),
    ReadFlash(u32, u32),
    EraseFlash,
    EraseFlashAuth(u64),
}

impl ToString for LogLevel {
    fn to_string(&self) -> String {
        match self {
            Debug => "DEBUG",
            Info => "INFO",
            Warning => "WARNING",
            Error => "ERROR",
            Critical => "CRITICAL",
        }
        .to_string()
    }
}

pub trait Transmit: Sized {
    fn wrap(&self) -> Vec<u8>;
    fn read_valid(buf: &[u8]) -> Option<Self>;
    fn pop_valid(buf: &mut Vec<u8>) -> Option<Self>;
}

impl<M: Serialize + DeserializeOwned> Transmit for M {
    fn wrap(&self) -> Vec<u8> {
        let mut buf = [0u8; 1024 + 8];
        let serialized = postcard::to_slice(self, &mut buf).unwrap();

        if serialized.len() > 127 {
            // For large packets (basically just for flash reading) we set the most
            // significant bit of the first length byte to indicate that we use a
            // 16-bit (or rather, 15-bit) length value
            let len = serialized.len();
            [
                &[0x42, 0x80 | ((len >> 8) as u8), len as u8],
                &*serialized
            ].concat()
        } else {
            [&[0x42, serialized.len() as u8], &*serialized].concat()
        }
    }

    // TODO: make this prettier

    fn read_valid(buf: &[u8]) -> Option<Self> {
        if buf.len() == 0 {
            return None;
        }

        if buf[0] != 0x42 {
            return None;
        }

        if buf.len() < 3 {
            return None;
        }

        let len = buf[1] as usize;
        if (len & 0x80) > 0 { // 15-bit length mode
            let len = ((len & 0x7f) << 8) + (buf[2] as usize);
            if buf.len() < 3 + len {
                return None;
            }

            postcard::from_bytes::<Self>(&buf[3..(len + 3)]).ok()
        } else { // 7-bit length mode
            if buf.len() < 2 + len {
                return None;
            }

            postcard::from_bytes::<Self>(&buf[2..(len + 2)]).ok()
        }
    }

    fn pop_valid(buf: &mut Vec<u8>) -> Option<Self> {
        while buf.len() > 0 {
            if buf[0] == 0x42 {
                if buf.len() < 3 {
                    return None;
                }

                let len = buf[1] as usize;
                if (len & 0x80) > 0 { // 15-bit length mode
                    let len = ((len & 0x7f) << 8) + (buf[2] as usize);
                    if buf.len() < 3 + len {
                        return None;
                    }
                } else { // 7-bit length mode
                    if buf.len() < 2 + len {
                        return None;
                    }
                }

                break;
            }

            buf.remove(0);
        }

        if buf.len() < 3 {
            return None;
        }

        let len = buf[1] as usize;
        if (len & 0x80) > 0 { // 15-bit length mode
            let len = ((len & 0x7f) << 8) + (buf[2] as usize);

            let head = buf[3..(len+3)].to_vec();
            for _i in 0..(len + 3) {
                buf.remove(0);
            }

            postcard::from_bytes::<Self>(&head).ok()
        } else { // 7-bit length mode
            let head = buf[2..(len+2)].to_vec();
            for _i in 0..(len + 2) {
                buf.remove(0);
            }

            postcard::from_bytes::<Self>(&head).ok()
        }
    }
}
