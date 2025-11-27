use reqwest::{Url, header::ACCEPT};
use semver::Version;
use serde::Deserialize;

use crate::{VERSION, ZeroError};

const LATEST_URL: &'static str = "https://api.github.com/repos/lily-and-doll/zerosplitter/releases/latest";

pub fn check_for_updates() -> Result<Option<Url>, ZeroError> {
	let response = reqwest::blocking::Client::builder()
		.user_agent("ZeroSplitter")
		.build()?
		.get(LATEST_URL)
		.header(ACCEPT, "application/vnd.github+json")
		.send()?;

	let json = response.json::<LatestResponse>()?;

	if Version::parse(&json.tag_name).expect("Malformed tag on release!")
		> VERSION.parse().expect("No package version found!")
	{
		let download_url = json
			.assets
			.zero
			.browser_download_url
			.parse::<Url>()
			.map_err(|_| ZeroError::ParseError)?;

		Ok(Some(download_url))
	} else {
		Ok(None)
	}
}

#[derive(Deserialize)]
struct LatestResponse {
	tag_name: String,
	assets: Assets,
}

#[derive(Deserialize)]
struct Assets {
	#[serde(rename = "0")]
	zero: Asset,
}

#[derive(Deserialize)]
struct Asset {
	browser_download_url: String,
}
