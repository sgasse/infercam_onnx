use rscam::Frame;
use rscam::{Camera, Config};
use std::fs::File;
use std::io::Write;

pub fn get_cam() -> Camera {
    Camera::new("/dev/video0").unwrap()
}

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

pub fn get_frame_fn() -> Box<dyn Fn() -> Frame> {
    let mut cam = get_cam();

    cam.start(&Config {
        interval: (1, 30),
        resolution: (1280, 720),
        format: b"MJPG",
        ..Default::default()
    })
    .unwrap();

    let callback = move || cam.capture().unwrap();
    Box::new(callback)
}

#[cfg(test)]
mod test {
    use super::save_n_frames;

    #[test]
    fn test_saving_frames() {
        save_n_frames(1);
    }
}
