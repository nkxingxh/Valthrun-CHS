use std::{
    borrow::Cow,
    collections::{
        btree_map::Entry,
        HashMap,
    },
    fs::File,
    io::{
        BufReader,
        Write,
    },
    path::PathBuf,
    sync::{
        atomic::Ordering,
        Arc,
        Mutex,
    },
    thread,
    time::Instant,
};

use anyhow::Context;
use cs2::{
    BuildInfo,
    CS2Handle,
    StateCurrentMap,
};
use imgui::{
    Condition,
    ImColor32,
    SelectableFlags,
    StyleColor,
    StyleVar,
    TableColumnFlags,
    TableColumnSetup,
    TableFlags,
    TreeNodeFlags,
};
use obfstr::obfstr;
use overlay::UnicodeTextRenderer;
use url::Url;
use utils_state::StateRegistry;

use super::{
    Color,
    EspColor,
    EspColorType,
    EspConfig,
    EspSelector,
    GrenadeSettings,
    GrenadeSpotInfo,
    GrenadeType,
    KeyToggleMode,
};
use crate::{
    enhancements::StateGrenadeHelperPlayerLocation,
    radar::{
        self,
        WebRadar,
        WebRadarState,
    },
    settings::{
        AppSettings,
        EspBoxType,
        EspHeadDot,
        EspHealthBar,
        EspPlayerSettings,
        EspTracePosition,
    },
    utils::{
        self,
        ImGuiKey,
        ImguiComboEnum,
    },
    Application,
};

enum EspPlayerActiveHeader {
    Features,
    Style,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum GrenadeSettingsTarget {
    None,
    General,
    MapType(String),
    Map {
        map_name: String,
        display_name: String,
    },
}

impl GrenadeSettingsTarget {
    pub fn display_name(&self) -> &str {
        match self {
            Self::None => &"None",
            Self::General => &"Settings",
            Self::MapType(value) => value,
            Self::Map { display_name, .. } => display_name,
        }
    }

    pub fn ui_token(&self) -> Cow<'static, str> {
        match self {
            Self::None => "_none".into(),
            Self::General => "_settings".into(),
            Self::MapType(value) => format!("map_type_{}", value).into(),
            Self::Map { map_name: name, .. } => format!("map_{}", name).into(),
        }
    }

    pub fn ident_level(&self) -> usize {
        match self {
            Self::None => 0,
            Self::General => 0,
            Self::MapType(_) => 0,
            Self::Map { .. } => 1,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum GrenadeHelperTransferDirection {
    Export,
    Import,
}

enum GrenadeHelperTransferState {
    /// Currently no transfer in progress
    Idle,
    /// A new transfer should be initiated.
    Pending {
        direction: GrenadeHelperTransferDirection,
    },
    /// A transfer has been initiated.
    /// This might be ether an export or import.
    Active {
        direction: GrenadeHelperTransferDirection,
    },
    /// The current transfer failed.
    Failed {
        direction: GrenadeHelperTransferDirection,
        message: String,
    },
    /// The source file has been loaded.
    /// Prompting the user, if he wants to replace or add the new items.
    ImportPending {
        elements: HashMap<String, Vec<GrenadeSpotInfo>>,
    },
    ImportSuccess {
        count: usize,
        replacing: bool,
    },
    ExportSuccess {
        target_path: PathBuf,
    },
}

pub struct SettingsUI {
    discord_link_copied: Option<Instant>,
    radar_session_copied: Option<Instant>,

    esp_selected_target: EspSelector,
    esp_pending_target: Option<EspSelector>,
    esp_player_active_header: EspPlayerActiveHeader,

    grenade_helper_target: GrenadeSettingsTarget,
    grenade_helper_selected_id: usize,
    grenade_helper_skip_confirmation_dialog: bool,
    grenade_helper_new_item: Option<GrenadeSpotInfo>,
    grenade_helper_transfer_state: Arc<Mutex<GrenadeHelperTransferState>>,

    grenade_helper_pending_target: Option<GrenadeSettingsTarget>,
    grenade_helper_pending_selected_id: Option<usize>,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
impl SettingsUI {
    pub fn new() -> Self {
        Self {
            discord_link_copied: None,
            radar_session_copied: None,

            esp_selected_target: EspSelector::None,
            esp_pending_target: None,
            esp_player_active_header: EspPlayerActiveHeader::Features,

            grenade_helper_target: GrenadeSettingsTarget::General,
            grenade_helper_selected_id: 0,
            grenade_helper_new_item: None,
            grenade_helper_skip_confirmation_dialog: false,
            grenade_helper_transfer_state: Arc::new(Mutex::new(GrenadeHelperTransferState::Idle)),

            grenade_helper_pending_target: None,
            grenade_helper_pending_selected_id: None,
        }
    }

    pub fn render(
        &mut self,
        app: &Application,
        ui: &imgui::Ui,
        unicode_text: &UnicodeTextRenderer,
    ) {
        let content_font = ui.current_font().id();
        let _title_font = if let Some(font_id) = app.fonts.valthrun.font_id() {
            ui.push_font(font_id)
        } else {
            return;
        };

        ui.window(obfstr!("Valthrun-CHS"))
            .size([650.0, 300.0], Condition::FirstUseEver)
            .title_bar(false)
            .build(|| {
                {
                    for (text, color) in [
                        ("V", [0.81, 0.69, 0.06, 1.0]),
                        ("a", [0.84, 0.61, 0.15, 1.0]),
                        ("l", [0.86, 0.52, 0.24, 1.0]),
                        ("t", [0.89, 0.44, 0.33, 1.0]),
                        ("h", [0.92, 0.36, 0.41, 1.0]),
                        ("r", [0.95, 0.27, 0.50, 1.0]),
                        ("u", [0.97, 0.19, 0.59, 1.0]),
                        ("n", [1.00, 0.11, 0.68, 1.0]),
                        ("-", [0.79, 0.26, 0.78, 0.0]),
                        ("C", [0.79, 0.26, 0.78, 1.0]),
                        ("H", [0.53, 0.37, 0.85, 1.0]),
                        ("S", [0.28, 0.40, 0.90, 1.0]),
                    ] {
                        ui.text_colored(color, text);
                        ui.same_line();
                    }

                    ui.new_line();
                    ui.dummy([0.0, 5.0]);
                }

                let _content_font = ui.push_font(content_font);
                let mut settings = app.settings_mut();

                if let Some(_tab_bar) = ui.tab_bar("main") {
                    if let Some(_tab) = ui.tab_item("信息") {
                        let build_info = app.app_state.resolve::<BuildInfo>(()).ok();

                        ui.text(obfstr!(
                            "Valthrun-CHS 是一个开源的 CS2 外部只读内核游戏增强器。"
                        ));
                        ui.text(&format!(
                            "{} 版本 {} ({})",
                            obfstr!("Valthrun-CHS"),
                            VERSION,
                            env!("BUILD_TIME")
                        ));
                        ui.text(&format!(
                            "{} 版本 {} ({})",
                            obfstr!("CS2"),
                            build_info.as_ref().map_or("error", |info| &info.revision),
                            build_info
                                .as_ref()
                                .map_or("error", |info| &info.build_datetime)
                        ));
                        ui.text(" ");
                        ui.text(obfstr!("由 NKXingXh 汉化"));
                        ui.text(&format!(
                            "https://github.com/{}/{}",
                            obfstr!("nkxingxh"),
                            obfstr!("Valthrun-CHS")
                        ));

                        let ydummy = ui.window_size()[1]
                            - ui.cursor_pos()[1]
                            - ui.text_line_height_with_spacing() * 2.0
                            - 12.0;
                        ui.dummy([0.0, ydummy]);
                        ui.separator();

                        ui.text(obfstr!("加入 discord (English):"));
                        ui.text_colored(
                            [0.18, 0.51, 0.97, 1.0],
                            obfstr!("https://discord.gg/ecKbpAPW5T"),
                        );
                        if ui.is_item_hovered() {
                            ui.set_mouse_cursor(Some(imgui::MouseCursor::Hand));
                        }

                        if ui.is_item_clicked() {
                            self.discord_link_copied = Some(Instant::now());
                            ui.set_clipboard_text(obfstr!("https://discord.gg/ecKbpAPW5T"));
                        }

                        let show_copied = self
                            .discord_link_copied
                            .as_ref()
                            .map(|time| time.elapsed().as_millis() < 3_000)
                            .unwrap_or(false);

                        if show_copied {
                            ui.same_line();
                            ui.text("(已复制)");
                        }
                    }

                    if let Some(_) = ui.tab_item("热键") {
                        ui.button_key(
                            obfstr!("调出菜单"),
                            &mut settings.key_settings,
                            [150.0, 0.0],
                        );

                        {
                            let _enabled = ui.begin_enabled(matches!(
                                settings.esp_mode,
                                KeyToggleMode::Toggle | KeyToggleMode::Trigger
                            ));
                            ui.button_key_optional(
                                obfstr!("ESP 切换/触发"),
                                &mut settings.esp_toogle,
                                [150.0, 0.0],
                            );
                        }
                    }

                    if let Some(_tab) = ui.tab_item(obfstr!("视觉")) {
                        ui.set_next_item_width(150.0);
                        ui.combo_enum(
                            obfstr!("ESP"),
                            &[
                                (KeyToggleMode::Off, "始终关闭"),
                                (KeyToggleMode::Trigger, "按住键触发"),
                                (KeyToggleMode::TriggerInverted, "反向触发"),
                                (KeyToggleMode::Toggle, "按键切换"),
                                (KeyToggleMode::AlwaysOn, "保持启用"),
                            ],
                            &mut settings.esp_mode,
                        );

                        ui.checkbox(obfstr!("炸弹计时器"), &mut settings.bomb_timer);
                        ui.checkbox(obfstr!("旁观者名单"), &mut settings.spectators_list);
                        ui.checkbox(obfstr!("投掷物助手"), &mut settings.grenade_helper.active);
                    }

                    if let Some(_tab) = ui.tab_item(obfstr!("ESP")) {
                        if settings.esp_mode == KeyToggleMode::Off {
                            let _style = ui.push_style_color(StyleColor::Text, [ 1.0, 0.76, 0.03, 1.0 ]);
                            ui.text(obfstr!("ESP 已经关闭。"));
                            ui.text(obfstr!("请在 \"视觉\" 菜单中启用 \"ESP\""));
                        } else {
                            self.render_esp_settings(&mut *settings, ui);
                        }
                    }

                    if let Some(_tab) = ui.tab_item(obfstr!("投掷物助手")) {
                        if settings.grenade_helper.active {
                            self.render_grenade_helper(&app.app_state, &mut settings.grenade_helper, ui, unicode_text);
                        } else {
                            let _style = ui.push_style_color(StyleColor::Text, [ 1.0, 0.76, 0.03, 1.0 ]);
                            ui.text(obfstr!("投掷物助手已经关闭。"));
                            ui.text(obfstr!("请在 \"视觉\" 菜单中启用 \"投掷物助手\""));
                        }

                        self.render_grenade_helper_transfer(&mut settings.grenade_helper, ui);
                    }

                    if let Some(_) = ui.tab_item(obfstr!("辅助瞄准")) {
                        ui.set_next_item_width(150.0);
                        ui.combo_enum(
                            obfstr!("自动开火"),
                            &[
                                (KeyToggleMode::Off, "始终关闭"),
                                (KeyToggleMode::Trigger, "按住键触发"),
                                (KeyToggleMode::TriggerInverted, "反向触发"),
                                (KeyToggleMode::Toggle, "按键切换"),
                                (KeyToggleMode::AlwaysOn, "保持启用"),
                            ],
                            &mut settings.trigger_bot_mode,
                        );

                        if !matches!(
                            settings.trigger_bot_mode,
                            KeyToggleMode::Off | KeyToggleMode::AlwaysOn
                        ) {
                            ui.button_key_optional(
                                obfstr!("自动开火热键"),
                                &mut settings.key_trigger_bot,
                                [150.0, 0.0],
                            );
                        }
                        if !matches!(settings.trigger_bot_mode, KeyToggleMode::Off) {
                            let mut values_updated = false;
                            let slider_width = (ui.current_column_width() / 2.0 - 80.0).min(300.0).max(50.0);
                            let slider_width_1 = (ui.current_column_width() / 2.0 - 20.0).min(300.0).max(50.0);

                            ui.text(obfstr!("开火延迟 最小: ")); ui.same_line();
                            ui.set_next_item_width(slider_width);
                            values_updated |= ui.slider_config("##delay_min", 0, 300).display_format("%dms").build(&mut settings.trigger_bot_delay_min); ui.same_line();

                            ui.text(" 最大: "); ui.same_line();
                            ui.set_next_item_width(slider_width);
                            values_updated |= ui.slider_config("##delay_max", 0, 300).display_format("%dms").build(&mut settings.trigger_bot_delay_max); 

                            ui.text(obfstr!("Shoot duration: ")); ui.same_line();
                            ui.set_next_item_width(slider_width_1);
                            values_updated |= ui.slider_config("##shoot_duration", 0, 1000).display_format("%dms").build(&mut settings.trigger_bot_shot_duration); 

                            if values_updated {
                                /* fixup min/max */
                                let delay_min = settings
                                    .trigger_bot_delay_min
                                    .min(settings.trigger_bot_delay_max);
                                let delay_max = settings
                                    .trigger_bot_delay_min
                                    .max(settings.trigger_bot_delay_max);

                                settings.trigger_bot_delay_min = delay_min;
                                settings.trigger_bot_delay_max = delay_max;
                            }

                            ui.checkbox(
                                obfstr!("延迟后重新测试触发目标"),
                                &mut settings.trigger_bot_check_target_after_delay,
                            );
                            ui.checkbox(obfstr!("不打友军"), &mut settings.trigger_bot_team_check);
                            ui.separator();
                        }

                        ui.checkbox("Simle Recoil Helper", &mut settings.aim_assist_recoil);
                    }

                    if let Some(_) = ui.tab_item("雷达") {
                        let mut web_radar = app.web_radar.borrow_mut();
                        self.render_web_radar(&mut settings, &mut web_radar, &app.cs2, ui);
                    }

                    if let Some(_) = ui.tab_item("杂项") {
                        ui.checkbox(obfstr!("Valthrun 水印"), &mut settings.valthrun_watermark);

                        if ui.checkbox(
                            obfstr!("截图时隐藏叠加层"),
                            &mut settings.hide_overlay_from_screen_capture,
                        ) {
                            app.settings_screen_capture_changed
                                .store(true, Ordering::Relaxed);
                        }

                        if ui.checkbox(
                            obfstr!("显示渲染调试叠加层"),
                            &mut settings.render_debug_window,
                        ) {
                            app.settings_render_debug_window_changed
                                .store(true, Ordering::Relaxed);
                        }

                        // FPS Limit
                        ui.slider_config("叠加层 FPS 限制", 0, 960)
                            .build(&mut settings.overlay_fps_limit);
                    }
                }
            });
    }

    fn render_web_radar(
        &mut self,
        settings: &mut AppSettings,
        web_radar: &mut Option<Arc<Mutex<WebRadar>>>,
        cs2: &Arc<CS2Handle>,
        ui: &imgui::Ui,
    ) {
        match web_radar {
            Some(radar) => {
                let mut radar = radar.lock().unwrap();
                match radar.connection_state() {
                    WebRadarState::Connecting => {
                        ui.text(format!("正在连接到 {}", radar.endpoint()));
                        ui.text("请稍候...");
                    }
                    WebRadarState::Connected { session_id } => {
                        let mut radar_url = radar.endpoint().clone();
                        radar_url.set_path(&format!("/session/{}", session_id));
                        if radar_url.scheme() == "wss" {
                            let _ = radar_url.set_scheme("https");
                        } else {
                            let _ = radar_url.set_scheme("http");
                        }

                        ui.text(format!("正在分享当前游戏。"));
                        {
                            let mut session_id = session_id.clone();
                            ui.text("会话 ID");

                            ui.same_line_with_pos(100.0);
                            ui.set_next_item_width(300.0);
                            ui.input_text("##session_id", &mut session_id)
                                .read_only(true)
                                .build();

                            let show_copied = self
                                .radar_session_copied
                                .as_ref()
                                .map(|time| time.elapsed().as_millis() < 3_000)
                                .unwrap_or(false);

                            let copy_session_text = if show_copied {
                                "会话 ID 已复制"
                            } else {
                                "复制会话 id"
                            };

                            ui.same_line();
                            if ui.button(copy_session_text) {
                                ui.set_clipboard_text(format!("{}", session_id));
                                self.radar_session_copied = Some(Instant::now());
                            }
                        }
                        {
                            let mut radar_url = format!("{}", radar_url);
                            ui.set_next_item_width(100.0);
                            ui.text("URL");

                            ui.same_line_with_pos(100.0);
                            ui.set_next_item_width(300.0);
                            ui.input_text("##url", &mut radar_url)
                                .read_only(true)
                                .build();

                            ui.same_line();
                            if ui.button("打开 URL") {
                                ui.set_clipboard_text(&radar_url);
                                utils::open_url(&radar_url);
                            }
                        }

                        ui.new_line();
                        if ui.button("停止共享") {
                            radar.close_connection();
                            drop(radar);
                            *web_radar = None;
                        }
                    }
                    WebRadarState::Disconnected { message } => {
                        ui.text_colored([1.0, 0.0, 0.0, 1.0], "共享当前游戏时发生错误:");
                        ui.text(message);

                        ui.new_line();
                        if ui.button("Close") {
                            radar.close_connection();
                            drop(radar);
                            *web_radar = None;
                        }
                    }
                }
            }
            None => {
                let mut current_url = if let Some(value) = settings.web_radar_url.as_ref() {
                    value.to_string()
                } else {
                    "wss://radar.valth.run/publish".to_string()
                };

                let url = Url::parse(&current_url);
                ui.disabled(url.is_err(), || {
                    if ui.button("启用 Web 雷达") {
                        let url = url.as_ref().unwrap();
                        *web_radar = Some(radar::create_web_radar(url.clone(), cs2.clone()));
                    }
                });

                ui.same_line();
                ui.text(obfstr!("开始分享当前游戏"));
                {
                    let button_text = if settings.web_radar_advanced_settings {
                        "基础设置"
                    } else {
                        "高级设置"
                    };
                    let button_text_width = ui.calc_text_size(button_text)[0];

                    let total_width = ui.content_region_avail()[0] + 2.0;
                    ui.same_line_with_pos(total_width - button_text_width);
                    if ui.button(button_text) {
                        settings.web_radar_advanced_settings =
                            !settings.web_radar_advanced_settings;
                    }
                }

                ui.text("Web 雷达是一个全面详细的雷达，可以从任何地方进行访问。");
                ui.text("这意味着您还可以将包含所有敌人信息的雷达显示给您的队友。");

                if settings.web_radar_advanced_settings {
                    ui.new_line();
                    ui.text("高级设置");
                    ui.text("雷达服务器:");
                    ui.same_line();
                    let _style_red_boarder =
                        ui.push_style_color(StyleColor::Border, [1.0, 0.0, 0.0, 1.0]);
                    ui.set_next_item_width(ui.content_region_avail()[0]);
                    if ui.input_text("##url", &mut current_url).build() {
                        settings.web_radar_url = Some(current_url);
                    }
                }
            }
        }
    }

    fn render_esp_target(
        &mut self,
        settings: &mut AppSettings,
        ui: &imgui::Ui,
        target: &EspSelector,
    ) {
        let config_key = target.config_key();
        let target_enabled = settings
            .esp_settings_enabled
            .get(&config_key)
            .cloned()
            .unwrap_or_default();

        let parent_enabled = target_enabled || {
            let mut current = target.parent();
            while let Some(parent) = current.take() {
                let enabled = settings
                    .esp_settings_enabled
                    .get(&parent.config_key())
                    .cloned()
                    .unwrap_or_default();

                if enabled {
                    current = Some(parent);
                    break;
                }

                current = parent.parent();
            }

            current.is_some()
        };

        {
            let pos_begin = ui.cursor_screen_pos();
            let clicked = ui
                .selectable_config(format!(
                    "{} ##{}",
                    target.config_display(),
                    target.config_key()
                ))
                .selected(target == &self.esp_selected_target)
                .flags(SelectableFlags::SPAN_ALL_COLUMNS)
                .build();

            let indicator_color = if target_enabled {
                ImColor32::from_rgb(0x4C, 0xAF, 0x50)
            } else if parent_enabled {
                ImColor32::from_rgb(0xFF, 0xC1, 0x07)
            } else {
                ImColor32::from_rgb(0xF4, 0x43, 0x36)
            };
            let pos_end = ui.cursor_screen_pos();
            let indicator_radius = ui.current_font_size() * 0.25;

            ui.get_window_draw_list()
                .add_circle(
                    [
                        pos_begin[0] - indicator_radius - 5.0,
                        pos_begin[1] + (pos_end[1] - pos_begin[1]) / 2.0 - indicator_radius / 2.0,
                    ],
                    indicator_radius,
                    indicator_color,
                )
                .filled(true)
                .build();

            if clicked {
                self.esp_pending_target = Some(target.clone());
            }
        }

        let children = target.children();
        if children.len() > 0 {
            ui.indent();
            for child in children.iter() {
                self.render_esp_target(settings, ui, child);
            }
            ui.unindent();
        }
    }

    fn render_esp_settings_player(
        &mut self,
        settings: &mut AppSettings,
        ui: &imgui::Ui,
        target: EspSelector,
    ) {
        let config_key = target.config_key();
        let config_enabled = settings
            .esp_settings_enabled
            .get(&config_key)
            .cloned()
            .unwrap_or_default();

        let config = match settings.esp_settings.entry(config_key.clone()) {
            Entry::Occupied(entry) => {
                let value = entry.into_mut();
                if let EspConfig::Player(value) = value {
                    value
                } else {
                    log::warn!("Detected invalid player config for {}", config_key);
                    *value = EspConfig::Player(EspPlayerSettings::new(&target));
                    if let EspConfig::Player(value) = value {
                        value
                    } else {
                        unreachable!()
                    }
                }
            }
            Entry::Vacant(entry) => {
                if let EspConfig::Player(value) =
                    entry.insert(EspConfig::Player(EspPlayerSettings::new(&target)))
                {
                    value
                } else {
                    unreachable!()
                }
            }
        };
        let _ui_enable_token = ui.begin_enabled(config_enabled);

        let content_height =
            ui.content_region_avail()[1] - ui.text_line_height_with_spacing() * 2.0 - 16.0;
        unsafe {
            imgui::sys::igSetNextItemOpen(
                matches!(
                    self.esp_player_active_header,
                    EspPlayerActiveHeader::Features
                ),
                0,
            );
        };
        if ui.collapsing_header("功能", TreeNodeFlags::empty()) {
            self.esp_player_active_header = EspPlayerActiveHeader::Features;
            if let Some(_token) = {
                ui.child_window("features")
                    .size([0.0, content_height])
                    .begin()
            } {
                ui.indent_by(5.0);
                ui.dummy([0.0, 5.0]);

                const COMBO_WIDTH: f32 = 150.0;
                {
                    const ESP_BOX_TYPES: [(EspBoxType, &'static str); 3] = [
                        (EspBoxType::None, "关闭"),
                        (EspBoxType::Box2D, "2D 平面"),
                        (EspBoxType::Box3D, "3D 立体"),
                    ];

                    ui.set_next_item_width(COMBO_WIDTH);
                    ui.combo_enum(obfstr!("显示方框"), &ESP_BOX_TYPES, &mut config.box_type);
                }

                {
                    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
                    enum PlayerSkeletonType {
                        None,
                        Skeleton,
                    }

                    const PLAYER_SKELETON_TYPES: [(PlayerSkeletonType, &'static str); 2] = [
                        (PlayerSkeletonType::None, "关闭"),
                        (PlayerSkeletonType::Skeleton, "启用"),
                    ];

                    let mut skeleton_type = if config.skeleton {
                        PlayerSkeletonType::Skeleton
                    } else {
                        PlayerSkeletonType::None
                    };

                    ui.set_next_item_width(COMBO_WIDTH);
                    let value_changed = ui.combo_enum(
                        obfstr!("显示骨架"),
                        &PLAYER_SKELETON_TYPES,
                        &mut skeleton_type,
                    );

                    if value_changed {
                        config.skeleton = matches!(skeleton_type, PlayerSkeletonType::Skeleton);
                    }
                }

                {
                    const HEAD_DOT_TYPES: [(EspHeadDot, &'static str); 3] = [
                        (EspHeadDot::None, "No"),
                        (EspHeadDot::Filled, "Filled"),
                        (EspHeadDot::NotFilled, "Not Filled"),
                    ];

                    ui.set_next_item_width(COMBO_WIDTH);
                    ui.combo_enum(obfstr!("head dot"), &HEAD_DOT_TYPES, &mut config.head_dot);
                }

                {
                    const TRACER_LINE_TYPES: [(EspTracePosition, &'static str); 7] = [
                        (EspTracePosition::None, "无"),
                        (EspTracePosition::TopLeft, "左上"),
                        (EspTracePosition::TopCenter, "正上"),
                        (EspTracePosition::TopRight, "右上"),
                        (EspTracePosition::BottomLeft, "左下"),
                        (EspTracePosition::BottomCenter, "正下"),
                        (EspTracePosition::BottomRight, "右下"),
                    ];

                    ui.set_next_item_width(COMBO_WIDTH);
                    ui.combo_enum(
                        obfstr!("追踪线"),
                        &TRACER_LINE_TYPES,
                        &mut config.tracer_lines,
                    );
                }

                {
                    const HEALTH_BAR_TYPES: [(EspHealthBar, &'static str); 5] = [
                        (EspHealthBar::None, "无"),
                        (EspHealthBar::Top, "顶部"),
                        (EspHealthBar::Left, "左侧"),
                        (EspHealthBar::Bottom, "底部"),
                        (EspHealthBar::Right, "右侧"),
                    ];

                    ui.set_next_item_width(COMBO_WIDTH);
                    ui.combo_enum(obfstr!("血量条"), &HEALTH_BAR_TYPES, &mut config.health_bar);
                }
                ui.dummy([0.0, 10.0]);

                ui.text("显示玩家信息");
                ui.checkbox(obfstr!("名称"), &mut config.info_name);
                ui.checkbox(obfstr!("武器"), &mut config.info_weapon);
                ui.checkbox(obfstr!("距离"), &mut config.info_distance);
                ui.checkbox(obfstr!("生命值"), &mut config.info_hp_text);
                ui.checkbox(obfstr!("工具包"), &mut config.info_flag_kit);
                ui.checkbox(obfstr!("被闪了"), &mut config.info_flag_flashed);
                ui.checkbox(obfstr!("仅显示附近玩家"), &mut config.near_players);
                if config.near_players {
                    ui.same_line();
                    ui.slider_config("最大距离", 0.0, 50.0)
                        .build(&mut config.near_players_distance);
                }
            }
        }

        unsafe {
            imgui::sys::igSetNextItemOpen(
                matches!(self.esp_player_active_header, EspPlayerActiveHeader::Style),
                0,
            );
        };
        if ui.collapsing_header("外观", TreeNodeFlags::empty()) {
            self.esp_player_active_header = EspPlayerActiveHeader::Style;
            if let Some(_token) = {
                ui.child_window("styles")
                    .size([0.0, content_height])
                    .begin()
            } {
                ui.indent_by(5.0);
                ui.dummy([0.0, 5.0]);

                if let Some(_token) = {
                    let mut column_type = TableColumnSetup::new("类型");
                    column_type.init_width_or_weight = 130.0;
                    column_type.flags = TableColumnFlags::WIDTH_FIXED;

                    let mut column_value = TableColumnSetup::new("值");
                    column_value.init_width_or_weight = 160.0;
                    column_value.flags = TableColumnFlags::WIDTH_FIXED;

                    ui.begin_table_header_with_flags(
                        "styles_table",
                        [TableColumnSetup::new("项目名称"), column_type, column_value],
                        TableFlags::ROW_BG
                            | TableFlags::BORDERS
                            | TableFlags::SIZING_STRETCH_PROP
                            | TableFlags::SCROLL_Y,
                    )
                } {
                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("ESP 方框颜色"),
                        &mut config.box_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("ESP 方框线宽"),
                        1.0,
                        10.0,
                        &mut config.box_width,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("玩家骨架颜色"),
                        &mut config.skeleton_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("玩家骨架线宽"),
                        1.0,
                        10.0,
                        &mut config.skeleton_width,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("Head dot color"),
                        &mut config.head_dot_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("Head dot thickness"),
                        1.0,
                        5.0,
                        &mut config.head_dot_thickness,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("Head dot radius"),
                        0.0,
                        10.0,
                        &mut config.head_dot_base_radius,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("Head dot z offset"),
                        0.0,
                        10.0,
                        &mut config.head_dot_z,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("血量条宽度"),
                        5.0,
                        30.0,
                        &mut config.health_bar_width,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("追踪线颜色"),
                        &mut config.tracer_lines_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_width(
                        ui,
                        obfstr!("追踪线宽度"),
                        1.0,
                        10.0,
                        &mut config.tracer_lines_width,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("名字文本颜色"),
                        &mut config.info_name_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("距离文本颜色"),
                        &mut config.info_distance_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("武器文本颜色"),
                        &mut config.info_weapon_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("生命值文本颜色"),
                        &mut config.info_hp_text_color,
                    );

                    ui.table_next_row();
                    Self::render_esp_settings_player_style_color(
                        ui,
                        obfstr!("玩家标志文本颜色"),
                        &mut config.info_flags_color,
                    );
                }
            }
        }

        drop(_ui_enable_token);
    }

    fn render_esp_settings_player_style_width(
        ui: &imgui::Ui,
        label: &str,
        min: f32,
        max: f32,
        value: &mut f32,
    ) -> bool {
        ui.table_next_column();
        ui.text(label);

        ui.table_next_column();
        ui.text(&format!("{:.2} - {:.2}", min, max));

        ui.table_next_column();
        if {
            ui.input_float(&format!("##{}_style_width", ui.table_row_index()), value)
                .build()
        } {
            *value = value.clamp(min, max);
            true
        } else {
            false
        }
    }

    fn render_esp_settings_player_style_color(ui: &imgui::Ui, label: &str, color: &mut EspColor) {
        ui.table_next_column();
        ui.text(label);

        ui.table_next_column();
        {
            let mut color_type = EspColorType::from_esp_color(color);
            ui.set_next_item_width(ui.content_region_avail()[0]);
            let color_type_changed = ui.combo_enum(
                &format!("##{}_color_type", ui.table_row_index()),
                &[
                    (EspColorType::Static, "静态"),
                    (EspColorType::HealthBased, "基于生命值"),
                    (EspColorType::HealthBasedRainbow, "花里胡哨"),
                    (EspColorType::DistanceBased, "基于距离"),
                ],
                &mut color_type,
            );

            if color_type_changed {
                *color = match color_type {
                    EspColorType::Static => EspColor::Static {
                        value: Color::from_f32([1.0, 1.0, 1.0, 1.0]),
                    },
                    EspColorType::HealthBased => EspColor::HealthBased {
                        max: Color::from_f32([0.0, 1.0, 0.0, 1.0]),
                        mid: Color::from_f32([1.0, 1.0, 0.0, 1.0]),
                        min: Color::from_f32([1.0, 0.0, 0.0, 1.0]),
                    },
                    EspColorType::HealthBasedRainbow => EspColor::HealthBasedRainbow { alpha: 1.0 },
                    EspColorType::DistanceBased => EspColor::DistanceBased {
                        near: Color::from_f32([1.0, 0.0, 0.0, 1.0]),
                        mid: Color::from_f32([1.0, 1.0, 0.0, 1.0]),
                        far: Color::from_f32([0.0, 1.0, 0.0, 1.0]),
                    },
                }
            }
        }

        ui.table_next_column();
        {
            match color {
                EspColor::HealthBasedRainbow { alpha } => {
                    ui.text("Alpha:");
                    ui.same_line();
                    ui.set_next_item_width(100.0);
                    ui.slider_config(
                        &format!("##{}_rainbow_alpha", ui.table_row_index()),
                        0.1,
                        1.0,
                    )
                    .display_format("%.2f")
                    .build(alpha);
                }
                EspColor::Static { value } => {
                    let mut color_value = value.as_f32();

                    if {
                        ui.color_edit4_config(
                            &format!("##{}_static_value", ui.table_row_index()),
                            &mut color_value,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    } {
                        *value = Color::from_f32(color_value);
                    }
                }
                EspColor::HealthBased { max, mid, min } => {
                    let mut max_value = max.as_f32();
                    if {
                        ui.color_edit4_config(
                            &format!("##{}_health_max", ui.table_row_index()),
                            &mut max_value,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    } {
                        *max = Color::from_f32(max_value);
                    }
                    ui.same_line();
                    ui.text(" => ");
                    ui.same_line();

                    let mut mid_value = mid.as_f32();
                    if {
                        ui.color_edit4_config(
                            &format!("##{}_health_mid", ui.table_row_index()),
                            &mut mid_value,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    } {
                        *mid = Color::from_f32(mid_value);
                    }
                    ui.same_line();
                    ui.text(" => ");
                    ui.same_line();

                    let mut min_value = min.as_f32();
                    if {
                        ui.color_edit4_config(
                            &format!("##{}_health_min", ui.table_row_index()),
                            &mut min_value,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    } {
                        *min = Color::from_f32(min_value);
                    }
                }
                EspColor::DistanceBased { near, mid, far } => {
                    let mut near_color = near.as_f32();
                    if ui
                        .color_edit4_config(
                            &format!("##{}_near", ui.table_row_index()),
                            &mut near_color,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    {
                        *near = Color::from_f32(near_color);
                    }

                    ui.same_line();
                    ui.text(" => ");
                    ui.same_line();
                    let mut mid_color = mid.as_f32();
                    if ui
                        .color_edit4_config(
                            &format!("##{}_mid", ui.table_row_index()),
                            &mut mid_color,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    {
                        *mid = Color::from_f32(mid_color);
                    }

                    ui.same_line();
                    ui.text(" => ");
                    ui.same_line();
                    let mut far_color = far.as_f32();
                    if ui
                        .color_edit4_config(
                            &format!("##{}_far", ui.table_row_index()),
                            &mut far_color,
                        )
                        .alpha_bar(true)
                        .inputs(false)
                        .label(false)
                        .build()
                    {
                        *far = Color::from_f32(far_color);
                    }
                }
            }
        }
    }

    fn render_esp_settings_chicken(
        &mut self,
        _settings: &mut AppSettings,
        ui: &imgui::Ui,
        _target: EspSelector,
    ) {
        ui.text("Chicken!");
    }

    fn render_esp_settings_weapon(
        &mut self,
        _settings: &mut AppSettings,
        ui: &imgui::Ui,
        _target: EspSelector,
    ) {
        ui.text("Weapon!");
    }

    fn render_esp_settings(&mut self, settings: &mut AppSettings, ui: &imgui::Ui) {
        if let Some(target) = self.esp_pending_target.take() {
            self.esp_selected_target = target;
        }

        /* the left tree */
        let content_region = ui.content_region_avail();
        let original_style = ui.clone_style();
        let tree_width = (content_region[0] * 0.25).max(150.0);
        let content_width = (content_region[0] - tree_width - 5.0).max(300.0);

        ui.text("ESP 目标");
        ui.same_line_with_pos(
            original_style.window_padding[0] * 2.0 + tree_width + original_style.window_border_size,
        );
        if !matches!(self.esp_selected_target, EspSelector::None) {
            let target_key = self.esp_selected_target.config_key();
            let target_enabled = settings
                .esp_settings_enabled
                .entry(target_key.to_string())
                .or_insert(false);

            ui.checkbox(self.esp_selected_target.config_title(), target_enabled);

            let reset_text = "重置配置";
            let reset_text_width = ui.calc_text_size(&reset_text)[0];

            let total_width = ui.content_region_avail()[0] + 2.0;
            ui.same_line_with_pos(total_width - reset_text_width);

            let _enabled = ui.begin_enabled(*target_enabled);
            if ui.button(reset_text) {
                /* just removing the key will work as a default config will be emplaced later */
                settings.esp_settings.remove(&target_key);
            }
        } else {
            ui.text("目标配置");
        };

        //ui.dummy([0.0, 10.0]);

        if let (Some(_token), _padding) = {
            let padding = ui.push_style_var(StyleVar::WindowPadding([
                0.0,
                original_style.window_padding[1],
            ]));
            let window = ui
                .child_window("ESP Target")
                .size([tree_width, 0.0])
                .border(true)
                .draw_background(true)
                .scroll_bar(true)
                .begin();

            (window, padding)
        } {
            ui.indent_by(
                original_style.window_padding[0] +
                /* for the indicator */
                ui.current_font_size() * 0.5 + 4.0,
            );

            self.render_esp_target(settings, ui, &EspSelector::Player);
            // self.render_esp_target(settings, ui, &EspSelector::Chicken);
            // self.render_esp_target(settings, ui, &EspSelector::Weapon)
        }
        ui.same_line();
        if let Some(_token) = {
            ui.child_window("Content")
                .size([content_width, 0.0])
                .scroll_bar(true)
                .begin()
        } {
            match &self.esp_selected_target {
                EspSelector::None => {}
                EspSelector::Player
                | EspSelector::PlayerTeam { .. }
                | EspSelector::PlayerTeamVisibility { .. } => {
                    self.render_esp_settings_player(settings, ui, self.esp_selected_target.clone())
                }
                EspSelector::Chicken => {
                    self.render_esp_settings_chicken(settings, ui, self.esp_selected_target.clone())
                }
                EspSelector::Weapon
                | EspSelector::WeaponGroup { .. }
                | EspSelector::WeaponSingle { .. } => {
                    self.render_esp_settings_weapon(settings, ui, self.esp_selected_target.clone())
                }
            }
        }
    }

    fn render_grenade_target(
        &mut self,
        settings: &mut GrenadeSettings,
        ui: &imgui::Ui,
        target: &GrenadeSettingsTarget,
    ) {
        let ident = ui.clone_style().indent_spacing * target.ident_level() as f32;
        if ident > 0.0 {
            ui.indent_by(ident);
        }

        let item_text = match target {
            GrenadeSettingsTarget::None => "None".to_string(),
            GrenadeSettingsTarget::General => "Settings".to_string(),
            GrenadeSettingsTarget::MapType(value) => value.clone(),
            GrenadeSettingsTarget::Map {
                map_name,
                display_name,
            } => {
                let location_count = settings.map_spots.get(map_name).map(Vec::len).unwrap_or(0);
                format!(
                    "{} ({}) ##{}",
                    display_name,
                    location_count,
                    target.ui_token()
                )
            }
        };

        let clicked = ui
            .selectable_config(item_text)
            .selected(target == &self.grenade_helper_target)
            .flags(SelectableFlags::SPAN_ALL_COLUMNS)
            .build();

        if clicked && !matches!(target, GrenadeSettingsTarget::MapType(_)) {
            self.grenade_helper_pending_target = Some(target.clone());
        }

        if ident > 0.0 {
            ui.unindent_by(ident);
        }
    }

    fn render_grenade_helper(
        &mut self,
        states: &StateRegistry,
        settings: &mut GrenadeSettings,
        ui: &imgui::Ui,
        unicode_text: &UnicodeTextRenderer,
    ) {
        if let Some(target) = self.grenade_helper_pending_target.take() {
            self.grenade_helper_target = target;
            self.grenade_helper_selected_id = 0;
            self.grenade_helper_new_item = None;
        }

        if let Some(target) = self.grenade_helper_pending_selected_id.take() {
            self.grenade_helper_selected_id = target;
            self.grenade_helper_new_item = None;
        }

        /* the left tree */
        let content_region = ui.content_region_avail();
        let original_style = ui.clone_style();
        let tree_width = (content_region[0] * 0.25).max(150.0);
        let content_width = content_region[0] - tree_width - 5.0;

        ui.text("Grenade Helper");

        ui.same_line_with_pos(
            original_style.window_padding[0] * 2.0 + tree_width + original_style.window_border_size,
        );
        ui.text("");

        {
            let text_import = "Import";
            let text_import_width = ui.calc_text_size(&text_import)[0];

            let text_export = "Export";
            let text_export_width = ui.calc_text_size(&text_export)[0];

            let total_width = ui.content_region_avail()[0] + 2.0;

            let mut grenade_helper_transfer_state =
                self.grenade_helper_transfer_state.lock().unwrap();
            let _buttons_disabled = ui.begin_disabled(!matches!(
                &*grenade_helper_transfer_state,
                GrenadeHelperTransferState::Idle
            ));
            ui.same_line_with_pos(
                total_width
                    - text_export_width
                    - original_style.frame_padding[0] * 2.0
                    - text_import_width
                    - original_style.frame_padding[0] * 2.0,
            );
            if ui.button(text_export) {
                *grenade_helper_transfer_state = GrenadeHelperTransferState::Pending {
                    direction: GrenadeHelperTransferDirection::Export,
                };
            }

            ui.same_line_with_pos(total_width - text_import_width);
            if ui.button(text_import) {
                *grenade_helper_transfer_state = GrenadeHelperTransferState::Pending {
                    direction: GrenadeHelperTransferDirection::Import,
                };
            }
        }

        //ui.dummy([0.0, 10.0]);

        if let (Some(_token), _padding) = {
            let padding = ui.push_style_var(StyleVar::WindowPadding([
                0.0,
                original_style.window_padding[1],
            ]));
            let window = ui
                .child_window("Helper Target")
                .size([tree_width, 0.0])
                .border(true)
                .draw_background(true)
                .scroll_bar(true)
                .begin();

            (window, padding)
        } {
            ui.indent_by(original_style.window_padding[0] + 4.0);

            for target in [
                GrenadeSettingsTarget::General,
                GrenadeSettingsTarget::MapType("Competitive Maps".to_owned()),
                GrenadeSettingsTarget::Map {
                    map_name: "de_ancient".to_owned(),
                    display_name: "Ancient".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_anubis".to_owned(),
                    display_name: "Anubis".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_dust2".to_owned(),
                    display_name: "Dust 2".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_inferno".to_owned(),
                    display_name: "Inferno".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_mills".to_owned(),
                    display_name: "Mills".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_mirage".to_owned(),
                    display_name: "Mirage".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_nuke".to_owned(),
                    display_name: "Nuke".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "cs_office".to_owned(),
                    display_name: "Office".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_overpass".to_owned(),
                    display_name: "Overpass".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "de_thera".to_owned(),
                    display_name: "Thera".to_owned(),
                },
                GrenadeSettingsTarget::Map {
                    map_name: "cs_vertigo".to_owned(),
                    display_name: "Vertigo".to_owned(),
                },
            ] {
                self.render_grenade_target(settings, ui, &target);
            }
        }
        ui.same_line();
        if let Some(_token) = {
            ui.child_window("Content")
                .size([content_width, 0.0])
                .scroll_bar(true)
                .begin()
        } {
            match &self.grenade_helper_target {
                GrenadeSettingsTarget::General => {
                    self.render_grenade_helper_target_settings(states, settings, ui);
                }
                GrenadeSettingsTarget::None | GrenadeSettingsTarget::MapType(_) => {
                    /* Nothing to render */
                }
                GrenadeSettingsTarget::Map { map_name, .. } => {
                    self.render_grenade_helper_target_map(
                        states,
                        settings,
                        ui,
                        &map_name.clone(),
                        unicode_text,
                    );
                }
            }
        }
    }

    fn render_grenade_helper_target_map(
        &mut self,
        states: &StateRegistry,
        settings: &mut GrenadeSettings,
        ui: &imgui::Ui,
        map_name: &str,
        unicode_text: &UnicodeTextRenderer,
    ) {
        /* the left tree */
        let content_region = ui.content_region_avail();
        let original_style = ui.clone_style();
        let tree_width = (content_region[0] * 0.25).max(150.0);
        let content_width = content_region[0] - tree_width - original_style.item_spacing[0];

        /* The list itself */
        {
            ui.text("Available spots");
            if let (Some(_token), _padding) = {
                let padding = ui.push_style_var(StyleVar::WindowPadding([
                    0.0,
                    original_style.window_padding[1],
                ]));
                let window = ui
                    .child_window("Map Target")
                    .size([
                        tree_width,
                        content_region[1]
                            - ui.text_line_height_with_spacing() * 2.0
                            - original_style.frame_padding[1] * 2.0,
                    ])
                    .border(true)
                    .draw_background(true)
                    .scroll_bar(true)
                    .begin();

                (window, padding)
            } {
                ui.indent_by(original_style.window_padding[0]);

                if let Some(grenades) = settings.map_spots.get(map_name) {
                    for grenade in grenades {
                        let grenade_types = grenade
                            .grenade_types
                            .iter()
                            .map(GrenadeType::display_name)
                            .collect::<Vec<_>>()
                            .join(", ");

                        let clicked = ui
                            .selectable_config(format!(
                                "{} ({}) ##{}",
                                grenade.name, grenade_types, grenade.id
                            ))
                            .selected(grenade.id == self.grenade_helper_selected_id)
                            .flags(SelectableFlags::SPAN_ALL_COLUMNS)
                            .build();
                        unicode_text.register_unicode_text(&grenade.name);

                        if clicked {
                            self.grenade_helper_pending_selected_id = Some(grenade.id);
                        }
                    }
                }
            }

            /* Add / delete buttons */
            {
                let mut delete_current_grenade = false;
                let current_grenade_position = settings
                    .map_spots
                    .get(map_name)
                    .map(|spots| {
                        spots
                            .iter()
                            .position(|spot| spot.id == self.grenade_helper_selected_id)
                    })
                    .flatten();

                let button_width = (tree_width - original_style.item_spacing[0]) / 2.0;
                ui.set_cursor_pos([
                    0.0,
                    content_region[1]
                        - ui.text_line_height()
                        - original_style.frame_padding[1] * 2.0,
                ]);
                if ui.button_with_size("New", [button_width, 0.0]) {
                    self.grenade_helper_new_item = Some(Default::default());
                    self.grenade_helper_selected_id = 0;
                }

                let _button_disabled = ui.begin_disabled(current_grenade_position.is_none());
                ui.same_line();
                if ui.button_with_size("Delete", [button_width, 0.0]) {
                    if self.grenade_helper_skip_confirmation_dialog {
                        delete_current_grenade = true;
                    } else {
                        ui.open_popup("Delete item? ##delete_grenade_helper_spot");
                    }
                }

                if let Some(_token) = ui
                    .modal_popup_config("Delete item? ##delete_grenade_helper_spot")
                    .resizable(false)
                    .movable(false)
                    .always_auto_resize(true)
                    .begin_popup()
                {
                    ui.text("Are you sure you want to delete this item?");
                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                    ui.checkbox(
                        "do not ask again",
                        &mut self.grenade_helper_skip_confirmation_dialog,
                    );

                    let button_width =
                        (ui.content_region_avail()[0] - original_style.item_spacing[0]) / 2.0;
                    if ui.button_with_size("Yes", [button_width, 0.0]) {
                        ui.close_current_popup();
                        delete_current_grenade = true;
                    }

                    ui.same_line();
                    if ui.button_with_size("No", [button_width, 0.0]) {
                        ui.close_current_popup();
                    }
                }

                if delete_current_grenade {
                    if let Some(grenades) = settings.map_spots.get_mut(map_name) {
                        grenades.remove(current_grenade_position.unwrap());
                    }
                }
            }
        }

        /* grenade info */
        ui.set_cursor_pos([tree_width + original_style.item_spacing[0], 0.0]);
        if let Some(_token) = {
            ui.child_window("Content")
                .size([content_width, 0.0])
                .scroll_bar(true)
                .begin()
        } {
            if let Some(current_grenade) = {
                settings
                    .map_spots
                    .get_mut(map_name)
                    .map(|spots| {
                        spots
                            .iter_mut()
                            .find(|spot| spot.id == self.grenade_helper_selected_id)
                    })
                    .flatten()
                    .or(self.grenade_helper_new_item.as_mut())
            } {
                let mut assign_current_position = false;
                let _full_width = ui.push_item_width(-1.0);

                if current_grenade.id > 0 {
                    ui.text("Grenade Info");
                } else {
                    ui.text("Add a new grenade spot");
                }

                ui.text("Name");
                ui.input_text("##grenade_helper_spot_name", &mut current_grenade.name)
                    .build();
                unicode_text.register_unicode_text(&current_grenade.name);

                ui.text("Description");
                ui.input_text_multiline(
                    "##grenade_helper_spot_description",
                    &mut current_grenade.description,
                    [0.0, 100.0],
                )
                .build();
                unicode_text.register_unicode_text(&current_grenade.description);

                ui.text("Eye position");
                ui.input_float3(
                    "##grenade_helper_spot_eye_position",
                    &mut current_grenade.eye_position,
                )
                .display_format("%.3f")
                .build();

                ui.text("Pitch/Yaw");
                ui.input_float2(
                    "##grenade_helper_spot_ptch_yaw",
                    &mut current_grenade.eye_direction,
                )
                .display_format("%.3f")
                .build();

                let current_map = states
                    .get::<StateCurrentMap>(())
                    .map(|value| value.current_map.clone())
                    .flatten();

                let current_player_position = states
                    .resolve::<StateGrenadeHelperPlayerLocation>(())
                    .map(|value| {
                        if let StateGrenadeHelperPlayerLocation::Valid {
                            eye_position,
                            eye_direction,
                        } = *value
                        {
                            Some((eye_position, eye_direction))
                        } else {
                            None
                        }
                    });

                {
                    let button_enabled =
                        current_player_position.as_ref().unwrap_or(&None).is_some();
                    let _enabled_token = ui.begin_enabled(button_enabled);
                    if ui.button("Use current") {
                        if current_map
                            .as_ref()
                            .map(|current_map| current_map == map_name)
                            .unwrap_or(false)
                        {
                            assign_current_position = true;
                        } else {
                            /* Map differs */
                            ui.open_popup(
                                "Are you sure?##grenade_helper_use_current_map_different",
                            );
                        }
                    }

                    if ui.is_item_hovered() {
                        match &current_player_position {
                            Ok(Some(_)) => {
                                ui.tooltip_text("Copy your current location and direction")
                            }
                            Ok(None) => ui.tooltip_text("You don't have a valid player position"),
                            Err(err) => ui.tooltip_text(format!("Error: {:#}", err)),
                        }
                    }
                }

                if let Some(_token) = ui
                    .modal_popup_config("Are you sure?##grenade_helper_use_current_map_different")
                    .resizable(false)
                    .always_auto_resize(true)
                    .begin_popup()
                {
                    ui.text("The current map does not match the selected map.");
                    ui.text(format!("Selected map: {}", map_name));
                    ui.text(format!(
                        "Current map: {}",
                        current_map
                            .as_ref()
                            .map(String::as_str)
                            .unwrap_or("unknown")
                    ));
                    ui.new_line();
                    ui.text("Do you want to copy the location anyways?");

                    ui.spacing();
                    ui.separator();
                    ui.spacing();

                    let button_width =
                        (ui.content_region_avail()[0] - original_style.item_spacing[0]) / 2.0;
                    if ui.button_with_size("Yes", [button_width, 0.0]) {
                        ui.close_current_popup();
                        assign_current_position = true;
                    }

                    ui.same_line();
                    if ui.button_with_size("No", [button_width, 0.0]) {
                        ui.close_current_popup();
                    }
                }

                if assign_current_position {
                    if let Some((eye_position, eye_direction)) =
                        current_player_position.ok().flatten()
                    {
                        current_grenade.eye_position = eye_position.as_slice().try_into().unwrap();
                        current_grenade.eye_direction =
                            eye_direction.as_slice().try_into().unwrap();
                    }
                }

                for grenade_type in [
                    GrenadeType::Smoke,
                    GrenadeType::Flashbang,
                    GrenadeType::Explosive,
                    GrenadeType::Molotov,
                ] {
                    let current_position = current_grenade
                        .grenade_types
                        .iter()
                        .position(|value| *value == grenade_type);

                    let mut enabled = current_position.is_some();
                    if ui.checkbox(grenade_type.display_name(), &mut enabled) {
                        if let Some(current_position) = current_position {
                            current_grenade.grenade_types.remove(current_position);
                        } else {
                            current_grenade.grenade_types.push(grenade_type);
                        }
                    }
                }

                if current_grenade.id == 0 {
                    let region_avail = ui.content_region_max();
                    ui.set_cursor_pos([region_avail[0] - 100.0, ui.cursor_pos()[1]]);
                    if ui.button_with_size("Create", [100.0, 0.0]) {
                        if let Some(mut grenade) = self.grenade_helper_new_item.take() {
                            let grenades =
                                settings.map_spots.entry(map_name.to_string()).or_default();

                            grenade.id = GrenadeSpotInfo::new_id();
                            self.grenade_helper_pending_selected_id = Some(grenade.id);

                            grenades.push(grenade);
                        }
                    }
                }
            } else {
                let text = "Please select an item";
                let text_bounds = ui.calc_text_size(text);
                let region_avail = ui.content_region_avail();

                ui.set_cursor_pos([
                    (region_avail[0] - text_bounds[0]) / 2.0,
                    (region_avail[1] - text_bounds[1]) / 2.0,
                ]);

                ui.text_colored(
                    ui.style_color(StyleColor::TextDisabled),
                    "Please select a grenade",
                );
            }
        }
    }

    fn render_grenade_helper_target_settings(
        &mut self,
        _states: &StateRegistry,
        settings: &mut GrenadeSettings,
        ui: &imgui::Ui,
    ) {
        fn render_color(ui: &imgui::Ui, label: &str, value: &mut Color) {
            let mut color_value = value.as_f32();

            if {
                ui.color_edit4_config(label, &mut color_value)
                    .alpha_bar(true)
                    .inputs(false)
                    .label(true)
                    .build()
            } {
                *value = Color::from_f32(color_value);
            }
        }

        ui.text("UI Settings");
        ui.spacing();

        ui.input_float("Circle distance", &mut settings.circle_distance)
            .build();
        ui.input_float("Circle radius", &mut settings.circle_radius)
            .build();
        ui.input_scalar("Circle segments", &mut settings.circle_segments)
            .build();

        ui.input_float("Angle threshold yar", &mut settings.angle_threshold_yaw)
            .build();
        ui.input_float("Angle threshold pitch", &mut settings.angle_threshold_pitch)
            .build();

        render_color(ui, "Color position", &mut settings.color_position);
        render_color(
            ui,
            "Color position (active)",
            &mut settings.color_position_active,
        );
        render_color(ui, "Color angle", &mut settings.color_angle);
        render_color(
            ui,
            "Color angle  (active)",
            &mut settings.color_angle_active,
        );
    }

    fn render_grenade_helper_transfer(&mut self, settings: &mut GrenadeSettings, ui: &imgui::Ui) {
        let mut transfer_state = self.grenade_helper_transfer_state.lock().unwrap();
        match &*transfer_state {
            GrenadeHelperTransferState::Idle => return,

            GrenadeHelperTransferState::Pending { direction } => {
                let executor: Box<
                    dyn FnOnce() -> anyhow::Result<GrenadeHelperTransferState> + Send,
                > = match direction {
                    GrenadeHelperTransferDirection::Export => {
                        let items = settings.map_spots.clone();
                        Box::new(move || {
                            // GrenadeHelperTransferState
                            let Some(target_path) = rfd::FileDialog::new()
                                .add_filter("Valthrun Grenade Spots", &["vgs"])
                                .save_file()
                            else {
                                return Ok(GrenadeHelperTransferState::Idle);
                            };

                            let items = serde_json::to_string(&items)?;
                            let mut output = File::options()
                                .create(true)
                                .truncate(true)
                                .write(true)
                                .open(&target_path)
                                .context("open destination")?;
                            output.write_all(items.as_bytes()).context("write")?;

                            Ok(GrenadeHelperTransferState::ExportSuccess { target_path })
                        })
                    }
                    GrenadeHelperTransferDirection::Import => {
                        Box::new(move || {
                            // GrenadeHelperTransferState
                            let Some(target_path) = rfd::FileDialog::new()
                                .add_filter("Valthrun Grenade Spots", &["vgs"])
                                .pick_file()
                            else {
                                return Ok(GrenadeHelperTransferState::Idle);
                            };

                            let input = File::options()
                                .read(true)
                                .open(target_path)
                                .context("open file")?;

                            let elements = serde_json::from_reader(&mut BufReader::new(input))
                                .context("parse file")?;

                            Ok(GrenadeHelperTransferState::ImportPending { elements })
                        })
                    }
                };

                thread::spawn({
                    let direction = direction.clone();
                    let grenade_helper_transfer_state = self.grenade_helper_transfer_state.clone();
                    move || {
                        let result = executor();
                        let mut transfer_state = grenade_helper_transfer_state.lock().unwrap();
                        match result {
                            Ok(new_state) => {
                                *transfer_state = new_state;
                            }
                            Err(err) => {
                                *transfer_state = GrenadeHelperTransferState::Failed {
                                    direction,
                                    message: format!("{:#}", err),
                                };
                            }
                        }
                    }
                });
                *transfer_state = GrenadeHelperTransferState::Active {
                    direction: direction.clone(),
                };
            }
            GrenadeHelperTransferState::Active { .. } => {
                /* Just waiting for the file picker to finish. */
            }

            GrenadeHelperTransferState::ImportPending { elements } => {
                let mut popup_open = true;
                if let Some(_popup) = ui
                    .modal_popup_config("Data Import")
                    .always_auto_resize(true)
                    .opened(&mut popup_open)
                    .begin_popup()
                {
                    let total_count = elements.values().map(|spots| spots.len()).sum::<usize>();

                    ui.text(format!(
                        "The following locations have been loaded ({})",
                        total_count
                    ));
                    for (map_name, spots) in elements.iter() {
                        ui.text(format!("- {} ({} spots)", map_name, spots.len()));
                    }

                    ui.new_line();
                    ui.text("Would you like to replace the current configuration?");

                    ui.spacing();
                    ui.separator();
                    ui.spacing();

                    let button_width =
                        (ui.content_region_avail()[0] - ui.clone_style().item_spacing[0]) / 2.0;

                    if ui.button_with_size("Cancel", [button_width, 0.0]) {
                        *transfer_state = GrenadeHelperTransferState::Idle;
                        return;
                    }

                    ui.same_line();
                    if ui.button_with_size("Yes", [button_width, 0.0]) {
                        settings.map_spots = elements.clone();
                        *transfer_state = GrenadeHelperTransferState::ImportSuccess {
                            count: total_count,
                            replacing: false,
                        };
                    }
                } else {
                    ui.open_popup("Data Import");
                }
            }

            GrenadeHelperTransferState::Failed { direction, message } => {
                let mut popup_open = true;
                let popup_name = format!(
                    "{} failed",
                    match direction {
                        GrenadeHelperTransferDirection::Export => "Export",
                        GrenadeHelperTransferDirection::Import => "Import",
                    }
                );
                if let Some(_popup) = ui
                    .modal_popup_config(&popup_name)
                    .opened(&mut popup_open)
                    .always_auto_resize(true)
                    .begin_popup()
                {
                    ui.text("A fatal error occurred:");
                    ui.spacing();

                    ui.text(message);

                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                    if ui.button_with_size("Close", [100.0, 0.0]) {
                        popup_open = false;
                    }
                } else {
                    ui.open_popup(&popup_name);
                }

                if !popup_open {
                    *transfer_state = GrenadeHelperTransferState::Idle;
                }
            }
            GrenadeHelperTransferState::ExportSuccess { target_path } => {
                let mut popup_open = true;
                if let Some(_popup) = ui
                    .modal_popup_config("Export successfull")
                    .opened(&mut popup_open)
                    .always_auto_resize(true)
                    .begin_popup()
                {
                    ui.text("All grenade helper spots have been exported to");
                    ui.text(format!("{}", target_path.display()));

                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                    if ui.button_with_size("Close", [100.0, 0.0]) {
                        popup_open = false;
                    }
                } else {
                    ui.open_popup("Export successfull");
                }

                if !popup_open {
                    *transfer_state = GrenadeHelperTransferState::Idle;
                }
            }
            GrenadeHelperTransferState::ImportSuccess { count, .. } => {
                let mut popup_open = true;
                if let Some(_popup) = ui
                    .modal_popup_config("Import successfull")
                    .opened(&mut popup_open)
                    .always_auto_resize(true)
                    .begin_popup()
                {
                    ui.text(format!("{} grenade helper spots have been imported", count));

                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                    if ui.button_with_size("Close", [100.0, 0.0]) {
                        popup_open = false;
                    }
                } else {
                    ui.open_popup("Import successfull");
                }

                if !popup_open {
                    *transfer_state = GrenadeHelperTransferState::Idle;
                }
            }
        }
    }
}