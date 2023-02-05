//! Sensors module.
//!
use std::pin::Pin;

use bytes::Bytes;
use futures_core::{
    task::{Context, Poll},
    Stream,
};
use rscam::{Camera, Config, Frame};

use crate::Error;

pub type CaptureFn = Box<dyn Fn() -> Option<Frame> + Send + Sync>;

/// Get a capture function to a video device on a Linux machine.
pub fn get_capture_fn_linux(
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

/// Initialized, streamable camera.
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

    /// Capture a frame.
    pub fn capture(&self) -> Option<Frame> {
        (*self.capture_fn)()
    }
}

impl Stream for StreamableCamera {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match (*self.capture_fn)() {
            Some(frame) => {
                // Append `\n\n` to mark the end of a frame
                let body = Bytes::copy_from_slice(&[&frame[..], "\n\n".as_bytes()].concat());

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

    #[cfg(webcam)]
    mod webcam_tests {

        use rscam::Camera;

        use crate::Error;

        #[test]
        fn get_cam_resolution() -> Result<(), Error> {
            let cam_name = "/dev/video0";
            let cam = Camera::new(cam_name)?;

            println!("Supported formats:");
            for format in cam.formats() {
                dbg!(format?);
            }

            if let Ok(interval) = cam.intervals("MJPG".as_bytes(), (1280, 720)) {
                println!("Supported interval:");
                dbg!(interval);
            }

            Ok(())
        }
    }
}
