//! Utility functions
//!
use std::{fs::File, io::Cursor};

use reqwest::Client;

use crate::Error;

/// Download a file from a URL to a given filepath.
pub async fn download_file(
    client: &Client,
    url: &str,
    filepath: impl AsRef<std::path::Path>,
) -> Result<(), Error> {
    let resp = client.get(url).send().await?;

    let mut file = File::create(filepath)?;
    let mut content = Cursor::new(resp.bytes().await?);
    std::io::copy(&mut content, &mut file)?;

    Ok(())
}
