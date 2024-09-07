use alloc::collections::VecDeque;
use alloc::vec::Vec;

use embassy_time::{Timer, Duration};
use embedded_hal_async::spi::SpiDevice;

use num_traits::float::Float;

use defmt::*;

//const PREV_VALUES_LENGTH: usize = 1000;
const PREV_VALUES_LENGTH: usize = 20;
const THRESHOLD: i64 = 300;
const MAX_OVERSHOOT_COUNTER: i32 = 500;

#[derive(Debug)]
struct MS5611CalibrationData {
    pressure_sensitivity: u16,
    pressure_offset: u16,
    temp_coef_pressure_sensitivity: u16,
    temp_coef_pressure_offset: u16,
    reference_temperature: u16,
    temp_coef_temperature: u16,
}

impl MS5611CalibrationData {
    pub fn valid(&self) -> bool {
        // We assume that every value needs to be non-zero and non-0xffff.
        self.pressure_sensitivity != 0x0000 &&
            self.pressure_offset != 0x0000 &&
            self.temp_coef_pressure_sensitivity != 0x0000 &&
            self.temp_coef_pressure_offset != 0x0000 &&
            self.reference_temperature != 0x0000 &&
            self.temp_coef_temperature != 0x0000 &&
            self.pressure_sensitivity != 0xffff &&
            self.pressure_offset != 0xffff &&
            self.temp_coef_pressure_sensitivity != 0xffff &&
            self.temp_coef_pressure_offset != 0xffff &&
            self.reference_temperature != 0xffff &&
            self.temp_coef_temperature != 0xffff
    }
}

pub struct MS5611<SPI: SpiDevice<u8>> {
    spi: SPI,
    calibration_data: Option<MS5611CalibrationData>,
    read_temp: bool,
    dt: Option<i32>,
    temp: Option<i32>,
    raw_pressure: Option<i32>,
    pressure: Option<i32>,
    baro_filter: BaroFilter,
}

impl<SPI: SpiDevice<u8>> MS5611<SPI> {
    pub async fn init(spi: SPI) -> Result<Self, SPI::Error> {
        let mut baro = Self {
            spi,
            calibration_data: None,
            read_temp: true,
            dt: None,
            temp: None,
            raw_pressure: None,
            pressure: None,
            baro_filter: BaroFilter::new(),
        };

        'outer: for _i in 0..3 { // did you know that rust has loop labels?
            baro.reset().await?;

            for _j in 0..50 {
                Timer::after(Duration::from_micros(10)).await;

                baro.read_calibration_values().await?;
                if baro.calibration_data.as_ref().map(|d| d.valid()).unwrap_or(false) {
                    break 'outer;
                }
            }
        }

        if baro.calibration_data.as_ref().map(|d| d.valid()).unwrap_or(false) {
            info!("MS5611 initialized");
        } else {
            error!("Failed to initialize MS5611");
        }

        Ok(baro)
    }

    async fn command(&mut self, command: MS5611Command, response_len: usize) -> Result<Vec<u8>, SPI::Error> {
        let mut payload = [alloc::vec![command.into()], [0x00].repeat(response_len)].concat();
        self.spi.transfer_in_place(&mut payload).await?;
        Ok(payload[1..].to_vec())
    }

    async fn reset(&mut self) -> Result<(), SPI::Error> {
        self.command(MS5611Command::Reset, 0).await?;
        Ok(())
    }

    async fn read_calibration_values(&mut self) -> Result<(), SPI::Error> {
        let c1 = self.command(MS5611Command::ReadProm(1), 2).await?;
        let c2 = self.command(MS5611Command::ReadProm(2), 2).await?;
        let c3 = self.command(MS5611Command::ReadProm(3), 2).await?;
        let c4 = self.command(MS5611Command::ReadProm(4), 2).await?;
        let c5 = self.command(MS5611Command::ReadProm(5), 2).await?;
        let c6 = self.command(MS5611Command::ReadProm(6), 2).await?;

        self.calibration_data = Some(MS5611CalibrationData {
            pressure_sensitivity: ((c1[0] as u16) << 8) + (c1[1] as u16),
            pressure_offset: ((c2[0] as u16) << 8) + (c2[1] as u16),
            temp_coef_pressure_sensitivity: ((c3[0] as u16) << 8) + (c3[1] as u16),
            temp_coef_pressure_offset: ((c4[0] as u16) << 8) + (c4[1] as u16),
            reference_temperature: ((c5[0] as u16) << 8) + (c5[1] as u16),
            temp_coef_temperature: ((c6[0] as u16) << 8) + (c6[1] as u16),
        });

        Ok(())
    }

    async fn read_sensor_data(&mut self, time: u32) -> Result<(), SPI::Error> {
        let response = self.command(MS5611Command::ReadAdc, 3).await?;
        let mut value = ((response[0] as i32) << 16) + ((response[1] as i32) << 8) + (response[2] as i32);
        let cal = self.calibration_data.as_ref().unwrap();

        if self.read_temp {
            //if time % 23 == 0 || time % 482 < 24 {
            //    value += 1000000;
            //}
            if time % 13 == 0 {
                value += 1000000;
            }

            let mut dt = (value as i32) - ((cal.reference_temperature as i32) << 8);
            //info!("Baro read_sensor_data(). dt: {:?}", dt);

            //if time % 2_000 < 1000 {
                dt = self.baro_filter.filter(dt, time);
            //}

            self.dt = Some(dt);
        } else {
            self.raw_pressure = Some(value);
        }

        if let Some((dt, raw_pressure)) = self.dt.zip(self.raw_pressure) {
            let mut temp = 2000 + (((dt as i64) * (cal.temp_coef_temperature as i64)) >> 23);

            let mut offset =
                ((cal.pressure_offset as i64) << 16) + ((cal.temp_coef_pressure_offset as i64 * dt as i64) >> 7);
            let mut sens = ((cal.pressure_sensitivity as i64) << 15)
                + (((cal.temp_coef_pressure_sensitivity as i64) * (dt as i64)) >> 8);

            // second order temp compensation
            if temp < 2000 {
                let t2 = ((dt as i64) * (dt as i64)) >> 31;
                let temp_offset = temp - 2000;
                let mut off2 = (5 * temp_offset * temp_offset) >> 1;
                let mut sens2 = off2 >> 1;

                if temp < -1500 { // brrrr
                    let temp_offset = temp + 1500;
                    off2 += 7 * temp_offset * temp_offset;
                    sens2 += (11 * temp_offset * temp_offset) >> 1;
                }

                temp -= t2;
                offset -= off2;
                sens -= sens2;
            }

            self.temp = Some(temp as i32);
            let p = (((raw_pressure as i64 * sens) >> 21) - offset) >> 15;
            self.pressure = Some(p as i32);
        }

        Ok(())
    }

    async fn start_next_conversion(&mut self) -> Result<(), SPI::Error> {
        let osr = MS5611OSR::OSR256;
        if self.read_temp {
            self.command(MS5611Command::StartTempConversion(osr), 0).await?;
        } else {
            self.command(MS5611Command::StartPressureConversion(osr), 0).await?;
        }
        Ok(())
    }

    pub async fn tick(&mut self, time: u32) {
        //info!("Baro tick.");
        if let Err(_) = self.read_sensor_data(time).await {
            self.dt = None;
            self.temp = None;
            self.raw_pressure = None;
            self.pressure = None;
            self.read_temp = true;
        } else {
            self.read_temp = !self.read_temp;
        }

        if let Err(_) = self.start_next_conversion().await {
            self.dt = None;
            self.temp = None;
            self.raw_pressure = None;
            self.pressure = None;
            self.read_temp = true;
        }
    }

    pub fn temperature(&self) -> Option<f32> {
        self.temp.map(|t| (t as f32) / 100.0)
    }

    pub fn pressure(&self) -> Option<f32> {
        self.pressure.map(|p| (p as f32) / 100.0)
    }

    pub fn altitude(&self) -> Option<f32> {
        self.pressure()
            .map(|p| 44330.769 * (1.0 - (p / 1012.5).powf(0.190223)))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum MS5611Command {
    Reset,
    StartPressureConversion(MS5611OSR),
    StartTempConversion(MS5611OSR),
    ReadAdc,
    ReadProm(u8),
}

impl Into<u8> for MS5611Command {
    fn into(self: Self) -> u8 {
        match self {
            Self::Reset => 0x1e,
            Self::StartPressureConversion(osr) => 0x40 + ((osr as u8) << 1),
            Self::StartTempConversion(osr) => 0x50 + ((osr as u8) << 1),
            Self::ReadAdc => 0x00,
            Self::ReadProm(adr) => 0xa0 + (adr << 1),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum MS5611OSR {
    OSR256 = 0b000,
    OSR512 = 0b001,
    OSR1024 = 0b010,
    OSR2048 = 0b011,
    OSR4096 = 0b100,
}

pub struct BaroFilter{
    previous_raw_values: VecDeque<i32>,
    last_filtered_value: Option<i32>,
    overshoot_counter: i32
}

impl BaroFilter {
    pub fn new() -> Self{
        info!("BaroFilter new");
        Self{
            previous_raw_values: VecDeque::with_capacity(PREV_VALUES_LENGTH),
            last_filtered_value: None,
            overshoot_counter: 0,
        }
    }


    //first filter (logical spike filter)
    //fn logical_spike_filter(&mut self, input: i64) -> i64 {
    //    //handle first value
    //    if self.prev_values.is_empty() {
    //        return input;
    //    }

    //    //handle normal case
    //    //overshoot detected
    //    let previous = self.prev_values.front().unwrap();
    //    //println!("input: {:?}, previous: {:?}", input, previous);
    //    if i64::abs(input - previous) > THRESHOLD {
    //        let mut sorted: Vec<_> = self.prev_values.iter().collect();
    //        sorted.sort();
    //        let median = *sorted[sorted.len() / 2];

    //        if i64::abs(median - input) > THRESHOLD {
    //            return *previous;
    //        }

    //        //it's still no drift from the real new value
    //        //if self.overshoot_counter < MAX_OVERSHOOT_COUNTER {
    //        //    //println!("inc");
    //        //    self.overshoot_counter += 1;
    //        //    return *previous;
    //        //} else {
    //        //    println!("overshoot_counter: {}", self.overshoot_counter);
    //        //}
    //    }

    //    ////either no overshoot detected or we drifted
    //    //If self.overshoot_counter > 0 {
    //    //    println!("resetting overshoot_counter: {}", self.overshoot_counter);
    //    //}
    //    //Self.overshoot_counter = 0;
    //    input
    //}

    //fn causal_median_filter(&mut self, input: i64) -> i64 {
    //    const SKIP_FACTOR: usize = 5;

    //    if self.prev_values.is_empty() {
    //        return input;
    //    }

    //    let mut sorted: Vec<_> = self.prev_values.iter().collect();
    //    sorted.sort();
    //    *sorted[sorted.len() / 2]

    //    //if self.prev_values[PREV_VALUES_LENGTH-1] != -1 {
    //    //    let mut median_array: [i64; PREV_VALUES_LENGTH/SKIP_FACTOR + 1] = [-1; PREV_VALUES_LENGTH/SKIP_FACTOR + 1];
    //    //    let mut j = 0;
    //    //    for i in 0..PREV_VALUES_LENGTH {
    //    //        if i % SKIP_FACTOR == 0 {
    //    //            median_array[j] = self.prev_values[i];
    //    //            j += 1;
    //    //        }
    //    //    }
    //    //    median_array[PREV_VALUES_LENGTH/SKIP_FACTOR] = input;

    //    //    Self::median(&mut median_array)
    //    //}
    //    //else{
    //    //    input
    //    //}
    //}


    pub fn filter(&mut self, input_value: i32, time: u32) -> i32 {
        let previous = self.last_filtered_value.unwrap_or(input_value);

        let mut sorted: Vec<_> = self.previous_raw_values.iter().collect();
        sorted.sort();
        let median = if sorted.len() > 0 {
            *sorted[sorted.len() / 2]
        } else {
            input_value
        };

        //let mean = if self.previous_raw_values.is_empty() {
        //    input_value
        //} else {
        //    self.previous_raw_values.iter().sum::<i32>() / (self.previous_raw_values.len() as i32)
        //};

        //info!("running filter with input = {:?}", value);
        //handle normal case
        //overshoot detected
        //println!("input: {:?}, previous: {:?}", input, previous);
        //let filtered = if i64::abs(input_value - median) > THRESHOLD {
        //    //if i64::abs(median - input_value) > THRESHOLD {
        //    //    previous
        //    //} else {
        //    //    input_value
        //    //}

        //    //it's still no drift from the real new value
        //    //if self.overshoot_counter < MAX_OVERSHOOT_COUNTER {
        //    //    //println!("inc");
        //    //    self.overshoot_counter += 1;
        //    //    return *previous;
        //    //} else {
        //    //    println!("overshoot_counter: {}", self.overshoot_counter);
        //    //}
        //    previous
        //} else {
        //    input_value
        //};
        let filtered = median;

        //const ALPHA: f32 = 0.99;
        //let filtered = ((previous as f32) * ALPHA + (input_value as f32) * (1.0 - ALPHA)) as i32;

        //if time % 10 == 0 {
            self.previous_raw_values.truncate(PREV_VALUES_LENGTH - 1);
            self.previous_raw_values.push_front(input_value);
        //}
        self.last_filtered_value = Some(filtered);

        //info!("spike filter result = {:?}", value);
        filtered
    }

    //fn median_of_medians(array: &mut [i64], k: usize) -> i64 {
    //    if array.len() == 1 {
    //        return array[0];
    //    }

    //    let pivot = array[array.len() / 2];
    //    let (lows, highs): (Vec<i64>, Vec<i64>) = array.iter().partition(|&&x| x < pivot);
    //    let num_lows = lows.len();

    //    if k < num_lows {
    //        Self::median_of_medians(&mut lows.into_iter().collect::<Vec<_>>(), k)
    //    } else if k > num_lows {
    //        Self::median_of_medians(&mut highs.into_iter().collect::<Vec<_>>(), k - num_lows - 1)
    //    } else {
    //        pivot
    //    }
    //}

    //fn median(array: &mut [i64]) -> i64 {
    //    let length = array.len();
    //    if length % 2 == 0{
    //        (Self::median_of_medians(array, length / 2 - 1) + Self::median_of_medians(array, length / 2)) / 2
    //    } else {
    //        Self::median_of_medians(array, length / 2)
    //    }
    //}
}
