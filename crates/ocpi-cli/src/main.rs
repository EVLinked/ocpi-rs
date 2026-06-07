//! # ocpi-cli
//!
//! `ocpi` — small command-line tools for working with OCPI parties:
//! list the versions a remote party supports, and validate that a JSON file
//! parses as an OCPI response envelope.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ocpi_client::OcpiClient;
use ocpi_types::OcpiResponse;
use url::Url;

/// OCPI command-line tools.
#[derive(Parser)]
#[command(name = "ocpi", version, about = "OCPI command-line tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the OCPI versions a remote party supports (`GET /versions`).
    Versions {
        /// Base URL of the remote party (e.g. `https://host/ocpi/cpo/2.2.1/`).
        url: String,
        /// OCPI authorization token to present.
        #[arg(long)]
        token: String,
    },
    /// Validate that a JSON file parses as an OCPI response envelope.
    Validate {
        /// Path to the JSON file to validate.
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Versions { url, token } => {
            let client = OcpiClient::new(Url::parse(&url)?, token);
            for version in client.versions().await? {
                println!("{}\t{}", version.version.as_str(), version.url);
            }
        }
        Command::Validate { path } => {
            let bytes = std::fs::read(&path)?;
            let envelope: OcpiResponse<serde_json::Value> = serde_json::from_slice(&bytes)?;
            println!(
                "valid OCPI envelope: status_code={} success={}",
                envelope.status_code,
                envelope.is_success()
            );
        }
    }
    Ok(())
}
