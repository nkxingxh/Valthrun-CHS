use cs2::{
    PlantedC4,
    PlantedC4State,
};
use overlay::UnicodeTextRenderer;

use super::Enhancement;
use crate::{
    settings::AppSettings,
    utils::ImguiUiEx,
};
pub struct BombInfoIndicator {}

impl BombInfoIndicator {
    pub fn new() -> Self {
        Self {}
    }
}

/// % of the screens height
const PLAYER_AVATAR_TOP_OFFSET: f32 = 0.004;

/// % of the screens height
const PLAYER_AVATAR_SIZE: f32 = 0.05;

impl Enhancement for BombInfoIndicator {
    fn update(&mut self, _ctx: &crate::UpdateContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &self,
        states: &utils_state::StateRegistry,
        ui: &imgui::Ui,
        unicode_text: &UnicodeTextRenderer,
    ) -> anyhow::Result<()> {
        let settings = states.resolve::<AppSettings>(())?;
        if !settings.bomb_timer {
            return Ok(());
        }

        let bomb_state = states.resolve::<PlantedC4>(())?;
        if matches!(bomb_state.state, PlantedC4State::NotPlanted) {
            return Ok(());
        }

        let group = ui.begin_group();

        let line_count = match &bomb_state.state {
            PlantedC4State::Active { .. } => 3,
            PlantedC4State::Defused | PlantedC4State::Detonated => 2,
            PlantedC4State::NotPlanted => unreachable!(),
        };
        let text_height = ui.text_line_height_with_spacing() * line_count as f32;

        /* align to be on the right side after the players */
        let offset_x = ui.io().display_size[0] * 1730.0 / 2560.0;
        let offset_y = ui.io().display_size[1] * PLAYER_AVATAR_TOP_OFFSET;
        let offset_y = offset_y
            + 0_f32.max((ui.io().display_size[1] * PLAYER_AVATAR_SIZE - text_height) / 2.0);

        ui.set_cursor_pos([offset_x, offset_y]);
        ui.text(&format!(
            "炸弹安放在 {}",
            if bomb_state.bomb_site == 0 { "A" } else { "B" }
        ));

        match &bomb_state.state {
            PlantedC4State::Active { time_detonation } => {
                ui.set_cursor_pos_x(offset_x);
                ui.text(&format!("倒计时: {:.3}", time_detonation));
                if let Some(defuser) = &bomb_state.defuser {
                    let color = if defuser.time_remaining > *time_detonation {
                        [0.79, 0.11, 0.11, 1.0]
                    } else {
                        [0.11, 0.79, 0.26, 1.0]
                    };

                    ui.set_cursor_pos_x(offset_x);
                    unicode_text.text_colored(
                        color,
                        &format!(
                            "{} 正在拆除... 需要 {:.3} 秒",
                            defuser.player_name, defuser.time_remaining
                        ),
                    );
                } else {
                    ui.set_cursor_pos_x(offset_x);
                    ui.text("未拆除");
                }
            }
            PlantedC4State::Defused => {
                ui.set_cursor_pos_x(offset_x);
                ui.text("炸弹已拆除");
            }
            PlantedC4State::Detonated => {
                ui.set_cursor_pos_x(offset_x);
                ui.text("炸了");
            }
            PlantedC4State::NotPlanted => unreachable!(),
        }

        group.end();
        Ok(())
    }
}
