//! Sensors module.
//!
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures_core::{
    task::{self, Poll},
    Stream,
};
use rscam::{Camera, Config, Frame, IntervalInfo, ResolutionInfo};

pub type CaptureFn = Box<dyn Fn() -> Option<Frame> + Send + Sync>;

const DEFAULT_CAM_DEVICE: &str = "/dev/video0";

/// Get a capture function to a video device on a Linux machine with maximum resolution in MJPG format.
pub fn get_max_res_mjpg_capture_fn() -> Result<CameraWrapper<Camera>> {
    let mut cam = Camera::new(DEFAULT_CAM_DEVICE)?;

    let format = &cam
        .formats()
        .filter_map(|format_res| format_res.ok().map(|x| x.format))
        .find(|x| x == "MJPG".as_bytes())
        .context("failed to find required format MJPG")?;

    let resolution = match cam.resolutions(format)? {
        ResolutionInfo::Discretes(resolutions) => {
            resolutions.iter().max_by(|a, b| a.0.cmp(&b.0)).cloned()
        }
        ResolutionInfo::Stepwise {
            min: _,
            max,
            step: _,
        } => Some(max),
    }
    .context("failed to get maximum resolution")?;

    let interval = match cam.intervals(format, resolution)? {
        IntervalInfo::Discretes(intervals) => {
            intervals.iter().max_by(|a, b| a.0.cmp(&b.0)).cloned()
        }
        IntervalInfo::Stepwise {
            min: _,
            max,
            step: _,
        } => Some(max),
    }
    .context("failed to get maximum interval")?;

    log::info!(
        "Starting camera {} with format {}, resolution {}x{} and interval {}/{}",
        DEFAULT_CAM_DEVICE,
        String::from_utf8_lossy(format),
        resolution.0,
        resolution.1,
        interval.1,
        interval.0,
    );
    cam.start(&Config {
        interval,
        resolution,
        format,
        ..Default::default()
    })?;

    // Ok(Box::new(move || cam.capture().ok()))
    Ok(CameraWrapper { inner: cam })
}

pub trait Capturable {
    fn get_frame(&self) -> Option<Frame>;
}

impl Capturable for Camera {
    fn get_frame(&self) -> Option<Frame> {
        self.capture().ok()
    }
}

pub struct CameraWrapper<T>
where
    T: Capturable,
{
    inner: T,
}

impl<T> CameraWrapper<T>
where
    T: Capturable,
{
    pub fn get_frame(&self) -> Option<Frame> {
        self.inner.get_frame()
    }
}

impl<T> Stream for CameraWrapper<T>
where
    T: Capturable,
{
    type Item = std::result::Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.get_frame() {
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

            if let Ok(resolutions) = cam.resolutions(b"MJPG") {
                dbg!(resolutions);
            }

            if let Ok(interval) = cam.intervals("MJPG".as_bytes(), (1280, 720)) {
                println!("Supported interval:");
                dbg!(interval);
            }

            Ok(())
        }
    }
}
