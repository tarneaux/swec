use clap::Parser;
use std::str::FromStr;
use swec_client::{Api, ReadApi, WriteApi};
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    info!("Starting checker: {}", args.name);
    let client = swec_client::ReadWrite::new(args.api_url.clone()).unwrap_or_else(|e| {
        error!("Failed to create API client: {e}");
        std::process::exit(1);
    });
    debug!("API client created. API URL: {}", args.api_url);
    debug!("Checking if checker exists");
    let spec = swec_core::Spec {
        description: args.description.clone(),
        url: match &args.checker {
            Checker::Http { url } => Some(url.to_string()),
        },
        group: args.group.clone(),
    };

    let api_info = client.get_info().await.unwrap_or_else(|e| {
        error!("Failed to get API info: {e}");
        error!("Is this a valid SWEC API? Exiting.");
        std::process::exit(1);
    });

    if !api_info.writable {
        error!("This API endpoint, while being a valid SWEC API, is not writable. Exiting.");
        std::process::exit(1);
    }

    if client.get_checker(&args.name).await.is_err() {
        info!("Checker does not exist. Sending POST request to create it");
        client
            .post_checker_spec(&args.name, spec)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to create checker: {e}");
                std::process::exit(1);
            });
    } else {
        info!("Checker already exists. Sending PUT request to update spec just in case");
        client
            .put_checker_spec(&args.name, spec)
            .await
            .expect("Failed to update checker");
    }

    info!("Starting main loop");

    loop {
        debug!("Checking {}", args.name);
        let status = args.checker.check(args.timeout).await;
        debug!("Status of {}: {status}", args.name);
        client
            .post_checker_status(&args.name, status)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to post status: {e}, ignoring.");
            });
        debug!("Sleeping for {} seconds", args.interval);
        tokio::time::sleep(tokio::time::Duration::from_secs(args.interval)).await;
    }
}

#[derive(Debug, Clone)]
enum Checker {
    Http { url: reqwest::Url },
}

impl Checker {
    async fn check(&self, timeout: u64) -> swec_core::Status {
        match self {
            Self::Http { url } => {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(timeout))
                    .build()
                    .expect("Failed to create HTTP client");
                match client.get(url.clone()).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            swec_core::Status {
                                is_up: true,
                                message: "Success".to_string(),
                            }
                        } else {
                            swec_core::Status {
                                is_up: false,
                                message: format!("HTTP error: {}", response.status()),
                            }
                        }
                    }
                    Err(e) => swec_core::Status {
                        is_up: false,
                        message: format!("Error: {e}"),
                    },
                }
            }
        }
    }
}

/// Create a `Checker` from a string.
/// The string should be in the format `http#<url>`.
impl FromStr for Checker {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, '#').collect();
        match parts.as_slice() {
            ["http", url] => {
                let url: reqwest::Url = url.parse().map_err(|e| format!("Invalid URL: {e}"))?;
                if !["http", "https"].contains(&url.scheme()) {
                    return Err(format!("Invalid scheme: {}", url.scheme()));
                }
                Ok(Self::Http { url })
            }
            _ => Err(format!("Invalid checker: {s}")),
        }
    }
}

#[derive(Clone, Parser, Debug)]
#[command(version, about, author, long_about)]
struct Args {
    name: String,
    description: String,
    checker: Checker,
    #[clap(short, long)]
    group: Option<String>,
    #[clap(short, long, default_value = "5")]
    interval: u64,
    #[clap(short, long, default_value = "10")]
    timeout: u64,
    #[clap(short, long, default_value = "http://localhost:8081/api/v1")]
    api_url: String,
}
