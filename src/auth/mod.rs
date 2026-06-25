use crate::config::client::ClientConfig;
use anyhow::{Context, Result};
use rspotify::{prelude::*, scopes, AuthCodeSpotify, Config as RSpotifyConfig, Credentials, OAuth};
use std::path::PathBuf;

pub fn build_client(client_cfg: &ClientConfig, cache_path: PathBuf) -> AuthCodeSpotify {
  let creds = Credentials::new(&client_cfg.client_id, &client_cfg.client_secret);

  let oauth = OAuth {
    redirect_uri: client_cfg.redirect_uri(),
    scopes: scopes!(
      "playlist-read-collaborative",
      "playlist-read-private",
      "playlist-modify-private",
      "playlist-modify-public",
      "user-follow-modify",
      "user-follow-read",
      "user-library-modify",
      "user-library-read",
      "user-modify-playback-state",
      "user-read-currently-playing",
      "user-read-playback-position",
      "user-read-playback-state",
      "user-read-private",
      "user-read-recently-played"
    ),
    ..Default::default()
  };

  let config = RSpotifyConfig {
    cache_path,
    token_cached: true,
    token_refreshing: true,
    ..Default::default()
  };

  AuthCodeSpotify::with_config(creds, oauth, config)
}

pub async fn authenticate(spotify: &AuthCodeSpotify) -> Result<()> {
  let url = spotify
    .get_authorize_url(false)
    .context("building authorize URL")?;
  spotify
    .prompt_for_token(&url)
    .await
    .context("OAuth token negotiation failed")?;
  Ok(())
}
