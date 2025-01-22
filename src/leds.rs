use embassy_futures::select::{select, Either};
use embassy_stm32::time::{hz, khz, mhz, Hertz};
use embassy_stm32::timer::Channel;
use embassy_stm32::{peripherals::*, timer::simple_pwm::SimplePwm};
use embassy_time::{Duration, Instant, Timer};

use num_traits::real::Real;

use shared_types::FlightMode;

use crate::FLIGHT_MODE_SIGNAL;

enum LedMode {
    Constant(f32),
    Sine(Hertz, f32, f32),
}

pub struct Leds {
    origin: Instant,
    pwm1: SimplePwm<'static, TIM1>,
    pwm2: SimplePwm<'static, TIM2>,
}

impl Leds {
    pub fn init(
        mut pwm1: SimplePwm<'static, TIM1>,
        mut pwm2: SimplePwm<'static, TIM2>,
    ) -> Self {
        pwm1.set_frequency(khz(10));
        pwm1.set_duty(Channel::Ch1, 0);
        pwm1.set_duty(Channel::Ch3, 0);
        pwm1.enable(Channel::Ch1);
        pwm1.enable(Channel::Ch3);
        pwm2.set_frequency(khz(10));
        pwm2.set_duty(Channel::Ch1, 0);
        pwm2.enable(Channel::Ch1);

        Self {
            origin: Instant::now(),
            pwm1,
            pwm2,
        }
    }

    pub fn led_state(&self, fm: FlightMode) -> (LedMode, LedMode, LedMode) {
        use LedMode::*;
        match fm {
            FlightMode::Idle => (Constant(0.0), Constant(0.0), Sine(hz(1), 0.1, 0.3)),
            FlightMode::HardwareArmed => (Constant(1.0), Sine(hz(2), 0.0, 1.0), Constant(0.0)),
            FlightMode::Armed => (Constant(1.0), Constant(1.0), Constant(0.0)),
            FlightMode::ArmedLaunchImminent => (Sine(hz(20), 0.0, 1.0), Constant(1.0), Constant(0.0)),
            FlightMode::Burn => (Constant(0.0), Sine(hz(5), 0.0, 1.0), Constant(0.0)),
            FlightMode::Coast => (Constant(0.0), Constant(1.0), Constant(0.0)),
            FlightMode::RecoveryDrogue => (Constant(0.0), Constant(1.0), Constant(1.0)),
            FlightMode::RecoveryMain => (Constant(1.0), Constant(0.0), Constant(1.0)),
            FlightMode::Landed => (Constant(0.0), Constant(0.0), Sine(hz(1), 0.0, 1.0)),
        }
    }

    fn mode_to_duty_cycle_f(&self, led_mode: LedMode) -> f32 {
        let time = self.origin.elapsed().as_millis();
        match led_mode {
            LedMode::Constant(f) => f,
            LedMode::Sine(freq, min, max) => {
                let s = f32::sin((freq.0 as f32) * (time as f32) / 1000.0);
                min + ((s + 1.0) / 2.0) * (max - min)
            }
        }
    }

    pub fn render(&mut self, flight_mode: FlightMode, ) {
        let (r,y,g) = self.led_state(flight_mode);
        let r = 1.0 - self.mode_to_duty_cycle_f(r);
        let y = 1.0 - self.mode_to_duty_cycle_f(y);
        let g = 1.0 - self.mode_to_duty_cycle_f(g);

        self.pwm1.set_duty(Channel::Ch1, (r * self.pwm1.get_max_duty() as f32) as u16);
        self.pwm1.set_duty(Channel::Ch3, (y * self.pwm1.get_max_duty() as f32) as u16);
        self.pwm2.set_duty(Channel::Ch1, (g * self.pwm2.get_max_duty() as f32) as u16);
    }
}

#[embassy_executor::task]
pub async fn run(mut leds: Leds) -> ! {
    let mut flight_mode = FlightMode::default();
    loop {
        leds.render(flight_mode);

        match select(FLIGHT_MODE_SIGNAL.wait(), Timer::after(Duration::from_millis(5))).await {
            Either::First(fm) => {
                flight_mode = fm;
            },
            Either::Second(_) => {}
        }
    }
}
