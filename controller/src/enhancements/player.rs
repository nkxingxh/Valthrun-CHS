use std::{ffi::CStr, sync::Arc};

use anyhow::Context;
use cs2::{BoneFlags, CEntityIdentityEx, CS2Model};
use cs2_schema_declaration::{define_schema, Ptr};
use cs2_schema_generated::cs2::client::{
    CCSPlayerController, CModelState, CSkeletonInstance, C_CSPlayerPawn,
};
use obfstr::obfstr;

use crate::{settings::AppSettings, view::ViewController};

use super::Enhancement;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum TeamType {
    Local,
    Enemy,
    Friendly,
}

pub struct PlayerInfo {
    pub team_type: TeamType,

    pub player_health: i32,
    pub player_name: String,
    pub position: nalgebra::Vector3<f32>,

    pub model: Arc<CS2Model>,
    pub bone_states: Vec<BoneStateData>,
}

impl PlayerInfo {
    pub fn calculate_screen_height(&self, view: &ViewController) -> Option<f32> {
        let entry_lower = view.world_to_screen(&(self.model.vhull_min + self.position), true)?;
        let entry_upper = view.world_to_screen(&(self.model.vhull_max + self.position), true)?;

        Some((entry_lower.y - entry_upper.y).abs())
    }
}

pub struct BoneStateData {
    pub position: nalgebra::Vector3<f32>,
}

impl TryFrom<CBoneStateData> for BoneStateData {
    type Error = anyhow::Error;

    fn try_from(value: CBoneStateData) -> Result<Self, Self::Error> {
        Ok(Self {
            position: nalgebra::Vector3::from_row_slice(&value.position()?),
        })
    }
}

define_schema! {
    pub struct CBoneStateData[0x20] {
        pub position: [f32; 3] = 0x00,
        pub scale: f32 = 0x0C,
        pub rotation: [f32; 4] = 0x10,
    }
}

trait CModelStateEx {
    #[allow(non_snake_case)]
    fn m_hModel(&self) -> anyhow::Result<Ptr<Ptr<()>>>;
    fn bone_state_data(&self) -> anyhow::Result<Ptr<[CBoneStateData]>>;
}

impl CModelStateEx for CModelState {
    #[allow(non_snake_case)]
    fn m_hModel(&self) -> anyhow::Result<Ptr<Ptr<()>>> {
        self.memory.reference_schema(0xA0)
    }

    fn bone_state_data(&self) -> anyhow::Result<Ptr<[CBoneStateData]>> {
        self.memory.reference_schema(0x80)
    }
}

pub struct PlayerESP {
    players: Vec<PlayerInfo>,
}

impl PlayerESP {
    pub fn new() -> Self {
        PlayerESP {
            players: Default::default(),
        }
    }

    fn generate_player_info(
        &self,
        ctx: &crate::UpdateContext,
        local_team: u8,
        player_controller: &Ptr<CCSPlayerController>,
    ) -> anyhow::Result<Option<PlayerInfo>> {
        let player_controller = player_controller.read_schema()?;

        let player_pawn = player_controller.m_hPlayerPawn()?;
        if !player_pawn.is_valid() {
            return Ok(None);
        }

        let player_pawn = match { ctx.cs2_entities.get_by_handle(&player_pawn)? } {
            Some(pawn) => pawn.entity_ptr::<C_CSPlayerPawn>()?.read_schema()?,
            None => {
                /*
                 * I'm not sure in what exact occasions this happens, but I would guess when the player is spectating or something.
                 * May check with m_bPawnIsAlive?
                 */
                return Ok(None);
            }
        };

        let player_health = player_pawn.m_iHealth()?;
        if player_health <= 0 {
            return Ok(None);
        }

        /* Will be an instance of CSkeletonInstance */
        let game_screen_node = player_pawn
            .m_pGameSceneNode()?
            .cast::<CSkeletonInstance>()
            .read_schema()?;
        if game_screen_node.m_bDormant()? {
            return Ok(None);
        }

        let player_team = player_controller.m_iTeamNum()?;
        let player_name = CStr::from_bytes_until_nul(&player_controller.m_iszPlayerName()?)
            .context("player name missing nul terminator")?
            .to_str()
            .context("invalid player name")?
            .to_string();

        let position =
            nalgebra::Vector3::<f32>::from_column_slice(&game_screen_node.m_vecAbsOrigin()?);

        let model = game_screen_node
            .m_modelState()?
            .m_hModel()?
            .read_schema()?
            .address()?;

        let model = ctx.model_cache.lookup(model)?;
        let bone_states = game_screen_node
            .m_modelState()?
            .bone_state_data()?
            .read_entries(model.bones.len())?
            .into_iter()
            .map(|bone| bone.try_into())
            .try_collect()?;

        let team_type = if player_controller.m_bIsLocalPlayerController()? {
            TeamType::Local
        } else if local_team == player_team {
            TeamType::Friendly
        } else {
            TeamType::Enemy
        };

        Ok(Some(PlayerInfo {
            team_type,
            player_name,
            player_health,
            position,

            bone_states,
            model: model.clone(),
        }))
    }
}

impl Enhancement for PlayerESP {
    fn update_settings(
        &mut self,
        ui: &imgui::Ui,
        settings: &mut AppSettings,
    ) -> anyhow::Result<bool> {
        let mut updated = false;

        if let Some(hotkey) = &settings.esp_toogle {
            if ui.is_key_pressed_no_repeat(hotkey.0) {
                log::debug!("Toggle player ESP");
                settings.esp = !settings.esp;
                updated = true;
            }
        }

        Ok(updated)
    }

    fn update(&mut self, ctx: &crate::UpdateContext) -> anyhow::Result<()> {
        self.players.clear();

        if !ctx.settings.esp || !(ctx.settings.esp_boxes || ctx.settings.esp_skeleton) {
            return Ok(());
        }

        self.players.reserve(16);

        let local_player_controller = ctx.cs2_entities.get_local_player_controller()?;

        if local_player_controller.is_null()? {
            /* We're currently not connected */
            return Ok(());
        }

        let local_player_controller = local_player_controller
            .reference_schema()
            .with_context(|| obfstr!("failed to read local player controller").to_string())?;

        let local_team = local_player_controller.m_iPendingTeamNum()?;

        let player_controllers = ctx.cs2_entities.get_player_controllers()?;
        for player_controller in player_controllers {
            match self.generate_player_info(ctx, local_team, &player_controller) {
                Ok(Some(info)) => self.players.push(info),
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "无法为 {:X} 生成玩家 ESP 信息: {:#}",
                        player_controller.address()?,
                        error
                    );
                }
            }
        }

        Ok(())
    }

    fn render(&self, settings: &AppSettings, ui: &imgui::Ui, view: &ViewController) {
        let draw = ui.get_window_draw_list();
        for entry in self.players.iter() {
            let esp_color = match &entry.team_type {
                TeamType::Local => continue,
                TeamType::Enemy => {
                    if !settings.esp_enabled_enemy {
                        continue;
                    }

                    &settings.esp_color_enemy
                }
                TeamType::Friendly => {
                    if !settings.esp_enabled_team {
                        continue;
                    }

                    &settings.esp_color_team
                }
            };

            if settings.esp_skeleton && entry.team_type != TeamType::Local {
                let bones = entry.model.bones.iter().zip(entry.bone_states.iter());

                for (bone, state) in bones {
                    if (bone.flags & BoneFlags::FlagHitbox as u32) == 0 {
                        continue;
                    }

                    let parent_index = if let Some(parent) = bone.parent {
                        parent
                    } else {
                        continue;
                    };

                    let parent_position = match view
                        .world_to_screen(&entry.bone_states[parent_index].position, true)
                    {
                        Some(position) => position,
                        None => continue,
                    };
                    let bone_position = match view.world_to_screen(&state.position, true) {
                        Some(position) => position,
                        None => continue,
                    };

                    draw.add_line(parent_position, bone_position, *esp_color)
                        .thickness(settings.esp_skeleton_thickness)
                        .build();
                }
            }

            if settings.esp_boxes && entry.team_type != TeamType::Local {
                view.draw_box_3d(
                    &draw,
                    &(entry.model.vhull_min + entry.position),
                    &(entry.model.vhull_max + entry.position),
                    (*esp_color).into(),
                    settings.esp_boxes_thickness,
                );
            }

            if settings.esp_health {
                if let Some(mut pos) = view.world_to_screen(&entry.position, false) {
                    let entry_height = entry.calculate_screen_height(view).unwrap_or(100.0);
                    let target_scale = entry_height * 15.0 / view.screen_bounds.y;
                    let target_scale = target_scale.clamp(0.5, 1.25);
                    ui.set_window_font_scale(target_scale);

                    let text = format!("{} HP", entry.player_health);
                    let [text_width, _] = ui.calc_text_size(&text);
                    pos.x -= text_width / 2.0;
                    draw.add_text(
                        pos,
                        esp_color.clone(),
                        format!("{} HP", entry.player_health),
                    );
                    ui.set_window_font_scale(1.0);
                }
            }
        }
    }
}
