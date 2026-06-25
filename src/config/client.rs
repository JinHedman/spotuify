use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;

const DEFAULT_REDIRECT_PORT: u16 = 8888;
const DASHBOARD_URL: &str = "https://developer.spotify.com/dashboard";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
  pub client_id: String,
  pub client_secret: String,
  #[serde(default = "default_port")]
  pub redirect_port: u16,
}

fn default_port() -> u16 {
  DEFAULT_REDIRECT_PORT
}

impl ClientConfig {
  pub fn redirect_uri(&self) -> String {
    format!("http://127.0.0.1:{}/callback", self.redirect_port)
  }

  pub fn load_or_bootstrap(path: &Path) -> Result<Self> {
    if path.exists() {
      Self::load(path)
    } else {
      let cfg = Self::prompt()?;
      cfg.save(path)?;
      println!();
      println!("Saved credentials to {}", path.display());
      println!();
      Ok(cfg)
    }
  }

  fn load(path: &Path) -> Result<Self> {
    let raw =
      std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
  }

  fn save(&self, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml::to_string(self)?;
    std::fs::write(path, yaml).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
  }

  fn prompt() -> Result<Self> {
    println!();
    println!("No client.yml yet — let's set up Spotify API credentials.");
    println!();
    println!("  1. We'll open {DASHBOARD_URL} in your browser.");
    println!("  2. Log in. Click 'Create app'. Fill in any name + description.");
    println!("  3. For the redirect URI, paste exactly:");
    println!("       http://127.0.0.1:{DEFAULT_REDIRECT_PORT}/callback");
    println!("  4. Under 'Which API/SDKs are you planning to use?', tick Web API.");
    println!("  5. Save, then copy the Client ID and Client Secret from the app page.");
    println!();

    if webbrowser::open(DASHBOARD_URL).is_err() {
      println!("(Could not open browser automatically — visit the URL above manually.)");
      println!();
    }

    let client_id = read_line("Client ID: ")?;
    let client_secret = read_line("Client Secret: ")?;
    let port_raw = read_line(&format!("Redirect port [{DEFAULT_REDIRECT_PORT}]: "))?;
    let redirect_port = if port_raw.trim().is_empty() {
      DEFAULT_REDIRECT_PORT
    } else {
      port_raw
        .trim()
        .parse()
        .with_context(|| format!("invalid port: {port_raw:?}"))?
    };

    Ok(Self {
      client_id: client_id.trim().to_string(),
      client_secret: client_secret.trim().to_string(),
      redirect_port,
    })
  }
}

fn read_line(prompt: &str) -> Result<String> {
  print!("{prompt}");
  io::stdout().flush()?;
  let mut buf = String::new();
  io::stdin().read_line(&mut buf).context("reading stdin")?;
  Ok(buf)
}
