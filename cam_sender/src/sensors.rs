use crate::Error;
use bytes::Bytes;
use futures_core::{
    task::{Context, Poll},
    Stream,
};
use rscam::Frame;
use rscam::{Camera, Config};
use std::pin::Pin;

pub type CaptureFn = Box<dyn Fn() -> Option<Frame> + Send + Sync>;

pub fn get_capture_fn(
    device_name: &str,
    resolution: (u32, u32),
    format: &str,
    frame_rate: (u32, u32),
) -> Result<CaptureFn, Error> {
    let mut cam = Camera::new(device_name)?;

    log::info!("Using camera {}", device_name);
    for format in cam.formats() {
        log::info!("Supported format: {:?}", format);
    }

    cam.start(&Config {
        interval: frame_rate,
        resolution,
        format: format.as_bytes(),
        ..Default::default()
    })?;

    let callback = move || cam.capture().ok();
    Ok(Box::new(callback))
}

/// Keep a handle to the capture function of an initialized camera.
pub struct StreamableCamera {
    capture_fn: CaptureFn,
}

impl StreamableCamera {
    /// Create a new instance.
    pub fn new(capture: CaptureFn) -> StreamableCamera {
        StreamableCamera {
            capture_fn: capture,
        }
    }

    pub fn capture(&self) -> Option<Frame> {
        (*self.capture_fn)()
    }
}

impl Stream for StreamableCamera {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match (*self.capture_fn)() {
            Some(frame) => {
                // TODO Remove
                use std::time::Duration;
                std::thread::sleep(Duration::from_secs(2));

                // Append `\n\n` to mark the end of a frame
                let body = Bytes::copy_from_slice(&[&frame[..], "\n\n".as_bytes()].concat());
                // let body = Bytes::copy_from_slice(&["test".as_bytes(), "\n\n".as_bytes()].concat());

                log::debug!("Streaming... ({} bytes)", body.len());

                Poll::Ready(Some(Ok(body)))
            }
            None => {
                log::error!("Error capturing frame");
                Poll::Ready(None)
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::Error;
    use rscam::Camera;

    #[test]
    fn get_cam_resolution() -> Result<(), Error> {
        let cam_name = "/dev/video0";
        let cam = Camera::new(cam_name)?;

        println!("Supported formats:");
        for format in cam.formats() {
            dbg!(format?);
        }

        println!("Supported interval:");
        for interval in cam.intervals("MJPG".as_bytes(), (1280, 720)) {
            dbg!(interval);
        }

        Ok(())
    }
}