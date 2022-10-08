//! Camera sensors, save functions and callbacks to capture methods.

use rscam::Frame;
use rscam::{Camera, Config};
use std::fs::File;
use std::io::Write;

/// Get camera at `/dev/video0` and log supported formats.
pub fn get_cam() -> Camera {
    let cam_name = "/dev/video0";
    let cam = Camera::new(cam_name).unwrap();

    log::info!("Using camera {}", cam_name);
    for format in cam.formats() {
        log::info!("Supported format: {:?}", format);
    }

    cam
}

/// Initialize camera and return a closure to the capture function.
pub fn get_capture_func(resolution: (u32, u32), format: &str) -> Box<dyn Fn() -> Frame> {
    let mut cam = get_cam();

    cam.start(&Config {
        interval: (1, 30),
        resolution,
        format: format.as_bytes(),
        ..Default::default()
    })
    .unwrap();

    let callback = move || cam.capture().unwrap();
    Box::new(callback)
}

/// Capture and save `n` frames on disk.
pub fn save_n_frames(n: u32) {
    let mut cam = get_cam();

    cam.start(&Config {
        interval: (1, 30),
        resolution: (1280, 720),
        format: b"MJPG",
        ..Default::default()
    })
    .unwrap();

    for i in 0..n {
        let frame = cam.capture().unwrap();
        let mut file = File::create(&format!("frame-{}.jpg", i)).unwrap();
        file.write_all(&frame[..]).unwrap();
    }
}
#[cfg(test)]
mod test {
    use super::save_n_frames;

    /// Test capturing and saving one frame on disk.
    #[test]
    fn test_saving_frames() {
        save_n_frames(1);
    }
}