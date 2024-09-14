use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use tokio::{task::JoinHandle, time::interval};

pub static METER: Meter = Meter::new();

#[derive(Default)]
pub struct Meter {
    raw_frames: AtomicU64,
    infered_frames: AtomicU64,
}

impl Meter {
    pub const fn new() -> Meter {
        Meter {
            raw_frames: AtomicU64::new(0),
            infered_frames: AtomicU64::new(0),
        }
    }

    pub fn tick_raw(&self) {
        self.raw_frames.fetch_add(1, Ordering::Relaxed);
    }

    pub fn tick_infered(&self) {
        self.infered_frames.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_reset_raw(&self) -> u64 {
        self.raw_frames.swap(0, Ordering::Relaxed)
    }

    pub fn get_reset_infered(&self) -> u64 {
        self.infered_frames.swap(0, Ordering::Relaxed)
    }
}

pub fn spawn_meter_logger() -> JoinHandle<()> {
    tokio::spawn(async {
        let mut log_interval = interval(Duration::from_secs(2));
        log_interval.tick().await;

        loop {
            let start = Instant::now();
            log_interval.tick().await;

            let raw_frames = METER.get_reset_raw();
            let infered_frames = METER.get_reset_infered();
            let elapsed = start.elapsed().as_secs_f32();
            let fps_raw = raw_frames as f32 / elapsed;
            let fps_infered = infered_frames as f32 / elapsed;

            if raw_frames > 0 {
                log::info!("Raw frames per second: {fps_raw:.2}")
            }
            if infered_frames > 0 {
                log::info!("Infered frames per second: {fps_infered:.2}")
            }
        }
    })
}
