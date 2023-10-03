use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Instant};

use imgui::Condition;
use obfstr::obfstr;

use crate::{
    settings::{AppSettings, HotKey},
    Application,
};

pub trait ImGuiKey {
    fn button_key(&self, label: &str, key: &mut HotKey, size: [f32; 2]) -> bool;
    fn button_key_optional(&self, label: &str, key: &mut Option<HotKey>, size: [f32; 2]) -> bool;
}

mod hotkey {
    use imgui::Key;

    use crate::settings::HotKey;

    pub fn render_button_key(
        ui: &imgui::Ui,
        label: &str,
        key: &mut Option<HotKey>,
        size: [f32; 2],
        optional: bool,
    ) -> bool {
        let _container = ui.push_id(label);

        let button_label = if let Some(key) = &key {
            format!("{:?}", key.0)
        } else {
            "None".to_string()
        };

        if !label.starts_with("##") {
            ui.text(label);
            ui.same_line();
        }

        let mut updated = false;
        if optional {
            if ui.button_with_size(&button_label, [size[0] - 35.0, size[1]]) {
                ui.open_popup(label);
            }

            ui.same_line_with_spacing(0.0, 10.0);

            ui.disabled(key.is_none(), || {
                if ui.button_with_size("X", [25.0, 0.0]) {
                    updated = true;
                    *key = None;
                }
            });
        } else {
            if ui.button_with_size(&button_label, size) {
                ui.open_popup(label);
            }
        }

        ui.modal_popup_config(label)
            .inputs(true)
            .collapsible(true)
            .movable(false)
            .menu_bar(false)
            .resizable(false)
            .title_bar(false)
            .build(|| {
                ui.text("Press any key or ESC to exit");

                if ui.is_key_pressed(Key::Escape) {
                    ui.close_current_popup();
                } else {
                    for key_variant in Key::VARIANTS {
                        if ui.is_key_pressed(key_variant) {
                            *key = Some(HotKey(key_variant));
                            updated = true;
                            ui.close_current_popup();
                        }
                    }
                }
            });

        updated
    }
}

impl ImGuiKey for imgui::Ui {
    fn button_key(&self, label: &str, key: &mut HotKey, size: [f32; 2]) -> bool {
        let mut key_opt = Some(key.clone());
        if hotkey::render_button_key(self, label, &mut key_opt, size, false) {
            *key = key_opt.unwrap();
            true
        } else {
            false
        }
    }

    fn button_key_optional(&self, label: &str, key: &mut Option<HotKey>, size: [f32; 2]) -> bool {
        hotkey::render_button_key(self, label, key, size, true)
    }
}

pub struct SettingsUI {
    settings: Rc<RefCell<AppSettings>>,
    discord_link_copied: Option<Instant>,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
impl SettingsUI {
    pub fn new(settings: Rc<RefCell<AppSettings>>) -> Self {
        Self {
            settings,
            discord_link_copied: None,
        }
    }

    pub fn render(&mut self, app: &Application, ui: &imgui::Ui) {
        ui.window(obfstr!("Valthrun-CHS"))
            .size([600.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                let mut settings = self.settings.borrow_mut();
                if let Some(_tab_bar) = ui.tab_bar("main") {
                    if let Some(_tab) = ui.tab_item("信息") {
                        ui.text(obfstr!("Valthrun 是一个开源的 CS2 外部只读内核游戏增强器。"));
                        ui.text(&format!("{} 版本 {}", obfstr!("Valthrun"), VERSION));
                        ui.text(&format!("{} 版本 {} ({})", obfstr!("CS2"), app.cs2_build_info.revision, app.cs2_build_info.build_datetime));
                        ui.text(" ");
                        ui.text(obfstr!("由 NKXingXh 汉化"));
                        ui.text(&format!("https://github.com/{}/{}", obfstr!("nkxingxh"), obfstr!("Valthrun-CHS")));

                        let ydummy = ui.window_size()[1] - ui.cursor_pos()[1] - ui.text_line_height_with_spacing() * 2.5;
                        ui.dummy([ 0.0, ydummy ]);
                        ui.separator();

                        ui.text("加入 discord (English):");
                        ui.text_colored([ 0.18, 0.51, 0.97, 1.0 ], obfstr!("https://discord.gg/ecKbpAPW5T"));
                        if ui.is_item_hovered() {
                            ui.set_mouse_cursor(Some(imgui::MouseCursor::Hand));
                        }

                        if ui.is_item_clicked() {
                            self.discord_link_copied = Some(Instant::now());
                            ui.set_clipboard_text(obfstr!("https://discord.gg/ecKbpAPW5T"));
                        }

                        let show_copied = self.discord_link_copied.as_ref()
                            .map(|time| time.elapsed().as_millis() < 3_000)
                            .unwrap_or(false);

                        if show_copied {
                            ui.same_line();
                            ui.text("(已复制)");
                        }
                    }

                    if let Some(_) = ui.tab_item("热键") {
                        ui.button_key("调出菜单", &mut settings.key_settings, [150.0, 0.0]);
                        ui.button_key_optional("ESP 开关", &mut settings.esp_toogle, [ 150.0, 0.0 ]);
                    }

                    if let Some(_tab) = ui.tab_item("视觉") {
                        ui.checkbox(obfstr!("ESP"), &mut settings.esp);

                        if settings.esp {
                            ui.checkbox(obfstr!("ESP 方框"), &mut settings.esp_boxes);
                            ui.slider_config("方框线宽", 0.1, 10.0)
                                .build(&mut settings.esp_boxes_thickness);
                            ui.checkbox(obfstr!("ESP 骨架"), &mut settings.esp_skeleton);
                            ui.slider_config("骨架线宽", 0.1, 10.0)
                                .build(&mut settings.esp_skeleton_thickness);
                            ui.checkbox(obfstr!("显示玩家生命值"), &mut settings.esp_health);

                            ui.checkbox("ESP 显示我方", &mut settings.esp_enabled_team);
                            if settings.esp_enabled_team {
                                ui.same_line();
                                ui.color_edit4_config("Team Color", &mut settings.esp_color_team)
                                    .alpha_bar(true)
                                    .inputs(false)
                                    .label(false)
                                    .build();
                                ui.same_line();
                                ui.text("我方颜色");
                            }

                            ui.checkbox("ESP 显示敌方", &mut settings.esp_enabled_enemy);
                            if settings.esp_enabled_enemy {
                                ui.same_line();
                                ui.color_edit4_config("Enemy Color", &mut settings.esp_color_enemy)
                                    .alpha_bar(true)
                                    .inputs(false)
                                    .label(false)
                                    .build();
                                ui.same_line();
                                ui.text("敌方颜色");
                            }
                            ui.separator();
                        }

                        ui.checkbox(obfstr!("炸弹计时器"), &mut settings.bomb_timer);

                        if ui.checkbox("截图时隐藏叠加层", &mut settings.hide_overlay_from_screen_capture) {
                            app.settings_screen_capture_changed.store(true, Ordering::Relaxed);
                        }
                    }

                    if let Some(_) = ui.tab_item("辅助瞄准") {
                        ui.button_key_optional("自动开火", &mut settings.key_trigger_bot, [150.0, 0.0]);
                        if settings.key_trigger_bot.is_some() {
                            let mut values_updated = false;

                            ui.text("开火延迟: "); ui.same_line();

                            let slider_width = (ui.current_column_width() / 2.0 - 20.0).min(300.0).max(50.0);
                            ui.set_next_item_width(slider_width);
                            values_updated |= ui.slider_config("##delay_min", 0, 250).display_format("%dms").build(&mut settings.trigger_bot_delay_min); ui.same_line();
                            ui.text(" - "); ui.same_line();
                            ui.set_next_item_width(slider_width);
                            values_updated |= ui.slider_config("##delay_max", 0, 250).display_format("%dms").build(&mut settings.trigger_bot_delay_max); 

                            if values_updated {
                                /* fixup min/max */
                                let delay_min = settings.trigger_bot_delay_min.min(settings.trigger_bot_delay_max);
                                let delay_max = settings.trigger_bot_delay_min.max(settings.trigger_bot_delay_max);

                                settings.trigger_bot_delay_min = delay_min;
                                settings.trigger_bot_delay_max = delay_max;
                            }

                            ui.checkbox("延迟后重新测试触发目标", &mut settings.trigger_bot_check_target_after_delay);
                            ui.checkbox("不打友军", &mut settings.trigger_bot_team_check);
                            ui.separator();
                        }

                        // ui.checkbox("Simle Recoil Helper", &mut settings.aim_assist_recoil);
                    }
                }
            });
    }
}
