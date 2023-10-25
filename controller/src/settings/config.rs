use std::{
    collections::BTreeMap,
    fs::File,
    io::{
        BufReader,
        BufWriter,
    },
    path::PathBuf,
};

use anyhow::Context;
use imgui::Key;
use serde::{
    Deserialize,
    Serialize,
};

use super::{
    EspConfig,
    HotKey,
};

fn bool_true() -> bool {
    true
}
fn bool_false() -> bool {
    false
}
fn default_u32<const V: u32>() -> u32 {
    V
}
fn default_i32<const V: i32>() -> i32 {
    V
}

fn default_key_settings() -> HotKey {
    Key::Pause.into()
}
fn default_key_trigger_bot() -> Option<HotKey> {
    Some(Key::MouseMiddle.into())
}
fn default_key_none() -> Option<HotKey> {
    None
}

fn default_esp_mode() -> KeyToggleMode {
    KeyToggleMode::AlwaysOn
}

fn default_trigger_bot_mode() -> KeyToggleMode {
    KeyToggleMode::Trigger
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
pub enum KeyToggleMode {
    AlwaysOn,
    Toggle,
    Trigger,
    TriggerInverted,
    Off,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AppSettings {
    #[serde(default = "default_key_settings")]
    pub key_settings: HotKey,

    #[serde(default = "default_esp_mode")]
    pub esp_mode: KeyToggleMode,

    #[serde(default = "default_key_none")]
    pub esp_toogle: Option<HotKey>,

    #[serde(default = "Default::default")]
    pub esp_settings: BTreeMap<String, EspConfig>,

    #[serde(default = "Default::default")]
    pub esp_settings_enabled: BTreeMap<String, bool>,

    #[serde(default = "bool_true")]
    pub bomb_timer: bool,

    #[serde(default = "bool_false")]
    pub spectators_list: bool,

    #[serde(default = "bool_true")]
    pub valthrun_watermark: bool,

    #[serde(default = "default_i32::<16364>")]
    pub mouse_x_360: i32,

    #[serde(default = "default_trigger_bot_mode")]
    pub trigger_bot_mode: KeyToggleMode,

    #[serde(default = "default_key_trigger_bot")]
    pub key_trigger_bot: Option<HotKey>,

    #[serde(default = "bool_true")]
    pub trigger_bot_team_check: bool,

    #[serde(default = "default_u32::<10>")]
    pub trigger_bot_delay_min: u32,

    #[serde(default = "default_u32::<20>")]
    pub trigger_bot_delay_max: u32,

    #[serde(default = "bool_false")]
    pub trigger_bot_check_target_after_delay: bool,

    #[serde(default = "bool_false")]
    pub aim_assist_recoil: bool,

    #[serde(default = "bool_true")]
    pub hide_overlay_from_screen_capture: bool,

    #[serde(default = "bool_false")]
    pub render_debug_window: bool,

    #[serde(default = "default_u32::<0>")]
    pub overlay_fps_limit: u32,

    #[serde(default = "bool_true")]
    pub metrics: bool,

    #[serde(default)]
    pub imgui: Option<String>,
}

pub fn get_settings_path() -> anyhow::Result<PathBuf> {
    let exe_file = std::env::current_exe().context("missing current exe path")?;
    let base_dir = exe_file.parent().context("could not get exe directory")?;

    Ok(base_dir.join("config.yaml"))
}

pub fn load_app_settings() -> anyhow::Result<AppSettings> {
    let config_path = get_settings_path()?;
    if !config_path.is_file() {
        log::info!(
            "应用程序配置文件 {} 不存在。",
            config_path.to_string_lossy()
        );
        log::info!("使用默认配置。");
        let config: AppSettings =
            serde_yaml::from_str("").context("failed to parse empty config")?;

        return Ok(config);
    }

    let config = File::open(&config_path).with_context(|| {
        format!(
            "failed to open app config at {}",
            config_path.to_string_lossy()
        )
    })?;
    let mut config = BufReader::new(config);

    let config: AppSettings =
        serde_yaml::from_reader(&mut config).context("failed to parse app config")?;

    log::info!("从 {} 加载应用程序配置", config_path.to_string_lossy());
    Ok(config)
}

pub fn save_app_settings(settings: &AppSettings) -> anyhow::Result<()> {
    let config_path = get_settings_path()?;
    let config = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&config_path)
        .with_context(|| {
            format!(
                "failed to open app config at {}",
                config_path.to_string_lossy()
            )
        })?;
    let mut config = BufWriter::new(config);

    serde_yaml::to_writer(&mut config, settings).context("failed to serialize config")?;

    log::debug!("保存应用配置。");
    Ok(())
}
