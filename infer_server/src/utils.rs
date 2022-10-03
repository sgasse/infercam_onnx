use crate::Error;
use reqwest::Client;
use std::{fs::File, io::Cursor};

pub async fn download_file(
    client: &Client,
    url: &str,
    filepath: impl AsRef<std::path::Path>,
) -> Result<(), Error> {
    // Setup download with reqwest
    println!("Downloading {}", url);
    let resp = client.get(url).send().await?;

    let mut file = File::create(filepath)?;
    let mut content = Cursor::new(resp.bytes().await?);
    std::io::copy(&mut content, &mut file)?;

    Ok(())
}
