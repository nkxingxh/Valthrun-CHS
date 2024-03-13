use std::fs;

use anyhow::Context;
use clap::Parser;
use cs2::{
    offsets_runtime,
    CS2Handle,
    CS2HandleState,
};
use radar_client::{
    CS2RadarGenerator,
    WebRadarPublisher,
};
use url::Url;
use utils_state::StateRegistry;

/// Standalone Valthrun CS2 radar
#[derive(Parser, Debug)]
#[command(long_about = None)]
struct Args {
    /// Target server address used to publish the web radar.
    /// Use ws://127.0.0.1:7229/publish for local development.
    #[arg(short, long, default_value = "wss://radar.valth.run/publish")]
    publish_url: String,

    // 一个 bool 型参数用来指示是否要将 sessionid 保存到本地
    #[arg(short, long)]
    session_id_write_to_file: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let url = Url::parse(&args.publish_url).context("invalid target server address")?;

    let radar_generator = {
        let cs2 = CS2Handle::create(true)?;
        offsets_runtime::setup_provider(&cs2)?;

        let mut states = StateRegistry::new(1024 * 8);
        states.set(CS2HandleState::new(cs2), ())?;

        Box::new(CS2RadarGenerator::new(states)?)
    };
    let radar_client = WebRadarPublisher::connect(radar_generator, &url).await?;

    let mut radar_url = url.clone();
    radar_url.set_path(&format!("/session/{}", radar_client.session_id));
    if radar_url.scheme() == "wss" {
        let _ = radar_url.set_scheme("https");
    } else {
        let _ = radar_url.set_scheme("http");
    }

    log::info!("Radar session {}", radar_client.session_id);
    log::info!("Available at {}", radar_url);

    if (args.session_id_write_to_file) {
        fs::write("session.txt", radar_url.as_str()).unwrap();
    }

    if let Some(err) = radar_client.await {
        log::error!("Radar error: {:#}", err);
    }
    Ok(())
}
