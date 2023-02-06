//! Sensors module.
//!
use std::pin::Pin;

use bytes::Bytes;
use futures_core::{
    task::{Context, Poll},
    Stream,
};
use rscam::{Camera, Config, Frame};
use simple_error::simple_error;

use crate::Error;

pub type CaptureFn = Box<dyn Fn() -> Option<Frame> + Send + Sync>;

/// Get a capture function to a video device on a Linux machine.
pub fn get_capture_fn_linux(
    device_name: &str,
    format: &str,
    resolution: Option<(u32, u32)>,
    frame_rate: Option<(u32, u32)>,
) -> Result<CaptureFn, Error> {
    let mut cam = Camera::new(device_name)?;
    log_supported_formats(&cam, format);
    let format = format.as_bytes();

    log::info!("Using camera {}", device_name);

    let resolution = resolution
        .map(Ok)
        .unwrap_or_else(|| get_max_resolution(&cam, format))?;

    let frame_rate = frame_rate
        .map(Ok)
        .unwrap_or_else(|| get_max_frame_rate(&cam, format, resolution))?;

    cam.start(&Config {
        interval: frame_rate,
        resolution,
        format,
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

/// Get the maximum supported resolution for the given format.
fn get_max_resolution(cam: &Camera, format: &[u8]) -> Result<(u32, u32), Error> {
    let resolution_info = cam.resolutions(format)?;
    log::debug!("Found resolutions: {:?}", &resolution_info);
    match resolution_info {
        rscam::ResolutionInfo::Discretes(resolutions) => resolutions
            .iter()
            // Map to iterator over ((width, height) num_pixels)
            .map(|res| (res, res.0 * res.1))
            // Get the highest resolution in terms of number of pixels
            .max_by(|a, b| a.1.cmp(&b.1))
            // Extract width and height values
            .map(|res| *res.0),
        rscam::ResolutionInfo::Stepwise {
            min: _,
            max,
            step: _,
        } => Some(max),
    }
    .ok_or_else(|| simple_error!("No resolution found").into())
}

/// Get the maximum supported frame rate for the given format and resolution.
fn get_max_frame_rate(
    cam: &Camera,
    format: &[u8],
    resolution: (u32, u32),
) -> Result<(u32, u32), Error> {
    let interval_info = cam.intervals(format, resolution)?;
    log::debug!("Found frame rates: {:?}", &interval_info);
    match interval_info {
        rscam::IntervalInfo::Discretes(frame_rates) => frame_rates
            .iter()
            // Map discrete values to real frame rate
            .map(|(denominator, numerator)| ((denominator, numerator), numerator / denominator))
            // Get the highest frame rate
            .max_by(|a, b| a.1.cmp(&b.1))
            // Extract denominator and numerator
            .map(|((&d, &n), _)| (d, n)),
        rscam::IntervalInfo::Stepwise {
            min: _,
            max,
            step: _,
        } => Some(max),
    }
    .ok_or_else(|| simple_error!("No frame rate found").into())
}

fn log_supported_formats(cam: &Camera, format: &str) {
    let formats: Vec<_> = cam
        .formats()
        .map(|fmt| match fmt {
            Ok(fmt) => Some(fmt),
            Err(_) => None,
        })
        .collect();
    log::debug!(
        "Supported formats: {:?}, using format {:?}",
        formats,
        format
    );
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn get_cam_info_if_available() -> Result<(), Error> {
        let cam_name = "/dev/video0";
        let cam = Camera::new(cam_name);

        match cam {
            Err(err) => println!("Could not initialize camera (maybe non available): {err}"),
            Ok(cam) => {
                let formats: Vec<_> = cam.formats().collect();
                println!("Supported formats: {formats:?}");

                let format = b"MJPG";

                let resolutions = cam.resolutions(format)?;
                println!("Supported resolutions: {resolutions:?}");

                let selected_resolution = get_max_resolution(&cam, format)?;
                let frame_rates = cam.intervals(format, selected_resolution)?;
                println!("Supported frame rates: {frame_rates:?}");
            }
        }

        Ok(())
    }
}
