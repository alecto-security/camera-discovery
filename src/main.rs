//! `camera-discovery`: report ONVIF cameras and NVRs on a local network.

use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;

use alecto_discovery::{discover, render_json, render_text, DiscoveryError};

/// Report ONVIF cameras and NVRs that answer WS-Discovery.
#[derive(Debug, Parser)]
#[command(name = "camera-discovery", version, about, long_about = None)]
struct Cli {
    /// Keep only devices whose XAddr host is inside this IPv4 CIDR.
    ///
    /// The probe is still multicast; the filter is applied to responses.
    #[arg(long, value_name = "CIDR")]
    network: Option<String>,

    /// How long to wait for responses, in seconds.
    #[arg(long, default_value_t = 5, value_name = "SECONDS")]
    timeout: u64,

    /// Print a JSON array instead of the human-readable report.
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("camera-discovery: {err}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<(), DiscoveryError> {
    let devices = discover(cli.network.as_deref(), Duration::from_secs(cli.timeout)).await?;
    let output = if cli.json {
        render_json(&devices).map_err(|err| DiscoveryError::fail(format!("render json: {err}")))?
    } else {
        render_text(&devices)
    };
    println!("{output}");
    Ok(())
}
