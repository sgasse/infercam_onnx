//! Utility functions
//!
use std::{fs::File, io::Cursor};

use anyhow::Result;
use reqwest::Client;

/// Download a file from a URL to a given filepath.
pub async fn download_file(
    client: &Client,
    url: &str,
    filepath: impl AsRef<std::path::Path>,
) -> Result<()> {
    let resp = client.get(url).send().await?;

    let mut file = File::create(filepath)?;
    let mut content = Cursor::new(resp.bytes().await?);
    std::io::copy(&mut content, &mut file)?;

    Ok(())
}
