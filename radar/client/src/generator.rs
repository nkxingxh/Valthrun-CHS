use std::ffi::CStr;

use anyhow::Context;
use cs2::{
    CEntityIdentityEx,
    ClassNameCache,
    EntitySystem,
    Globals,
    PlayerPawnInfo,
    StateCurrentMap,
};
use cs2_schema_generated::{
    cs2::client::{
        C_CSPlayerPawn,
        C_PlantedC4,
        C_C4,
    },
    EntityHandle,
};
use obfstr::obfstr;
use radar_shared::{
    BombDefuser,
    PlantedC4State,
    RadarC4,
    RadarPlantedC4,
    RadarPlayerPawn,
    RadarState,
};
use utils_state::StateRegistry;

pub trait RadarGenerator: Send {
    fn generate_state(&mut self) -> anyhow::Result<RadarState>;
}

fn planted_c4_to_radar_state(
    generator: &CS2RadarGenerator,
    planted_c4: &C_PlantedC4,
) -> anyhow::Result<PlantedC4State> {
    if planted_c4.m_bBombDefused()? {
        return Ok(PlantedC4State::Defused {});
    }

    let globals = generator.states.resolve::<Globals>(())?;
    let time_fuse = planted_c4.m_flC4Blow()?.m_Value()?;
    if time_fuse <= globals.time_2()? {
        return Ok(PlantedC4State::Detonated {});
    }

    let entities = generator.states.resolve::<EntitySystem>(())?;
    let time_total = planted_c4.m_flTimerLength()?;

    let defuser = if planted_c4.m_bBeingDefused()? {
        let time_defuse = planted_c4.m_flDefuseCountDown()?.m_Value()?;
        let time_total = planted_c4.m_flDefuseLength()?;

        let handle_defuser = planted_c4.m_hBombDefuser()?;
        let defuser = entities
            .get_by_handle(&handle_defuser)?
            .with_context(|| obfstr!("missing bomb defuser player pawn").to_string())?
            .entity()?
            .reference_schema()?;

        let defuser_controller = defuser.m_hController()?;
        let defuser_controller = entities
            .get_by_handle(&defuser_controller)?
            .with_context(|| obfstr!("missing bomb defuser controller").to_string())?
            .entity()?
            .reference_schema()?;

        let defuser_name = CStr::from_bytes_until_nul(&defuser_controller.m_iszPlayerName()?)
            .ok()
            .map(CStr::to_string_lossy)
            .unwrap_or("Name Error".into())
            .to_string();

        Some(BombDefuser {
            time_remaining: time_defuse - globals.time_2()?,
            time_total: time_total,

            player_name: defuser_name,
        })
    } else {
        None
    };

    Ok(PlantedC4State::Active {
        time_detonation: time_fuse - globals.time_2()?,
        time_total,
        defuser,
    })
}

pub struct CS2RadarGenerator {
    states: StateRegistry,
}

impl CS2RadarGenerator {
    pub fn new(states: StateRegistry) -> anyhow::Result<Self> {
        Ok(Self { states })
    }

    fn generate_pawn_info(
        &self,
        player_pawn_handle: EntityHandle<C_CSPlayerPawn>,
    ) -> anyhow::Result<RadarPlayerPawn> {
        let pawn_info = self.states.resolve::<PlayerPawnInfo>(player_pawn_handle)?;

        Ok(RadarPlayerPawn {
            controller_entity_id: pawn_info.controller_entity_id,
            pawn_entity_id: pawn_info.pawn_entity_id,

            player_name: pawn_info.player_name.clone().unwrap_or_default(),
            player_flashtime: pawn_info.player_flashtime,
            player_has_defuser: pawn_info.player_has_defuser,
            player_health: pawn_info.player_health,

            position: [
                pawn_info.position.x,
                pawn_info.position.y,
                pawn_info.position.z,
            ],
            rotation: pawn_info.rotation,

            team_id: pawn_info.team_id,
            weapon: pawn_info.weapon.id(),
        })
    }
}

impl RadarGenerator for CS2RadarGenerator {
    fn generate_state(&mut self) -> anyhow::Result<RadarState> {
        self.states.invalidate_states();

        let current_map = self.states.resolve::<StateCurrentMap>(())?;
        let mut radar_state = RadarState {
            player_pawns: Vec::with_capacity(16),
            world_name: current_map
                .current_map
                .as_ref()
                .map(|v| v.as_str())
                .unwrap_or("<empty>")
                .to_string(),

            planted_c4: None,
            c4_entities: Default::default(),

            local_controller_entity_id: None,
        };

        let entities = self.states.resolve::<EntitySystem>(())?;
        let class_name_cache = self.states.resolve::<ClassNameCache>(())?;

        let local_controller = entities.get_local_player_controller()?;
        if !local_controller.is_null()? {
            let local_controller_id = local_controller
                .reference_schema()?
                .m_pEntity()?
                .reference_schema()?
                .handle::<()>()?
                .get_entity_index();

            radar_state.local_controller_entity_id = Some(local_controller_id);
        }

        for entity_identity in entities.all_identities() {
            let entity_class =
                match class_name_cache.lookup(&entity_identity.entity_class_info()?)? {
                    Some(entity_class) => entity_class,
                    None => {
                        log::warn!(
                            "Failed to get entity class info {:X}",
                            entity_identity.memory.address,
                        );
                        continue;
                    }
                };

            match entity_class.as_str() {
                "C_CSPlayerPawn" => match self.generate_pawn_info(entity_identity.handle()?) {
                    Ok(info) => radar_state.player_pawns.push(info),
                    Err(error) => {
                        log::warn!(
                            "Failed to generate player pawn ESP info for {}: {:#}",
                            entity_identity.handle::<()>()?.get_entity_index(),
                            error
                        );
                    }
                },
                "C_PlantedC4" => {
                    let planted_c4 = entity_identity.entity_ptr::<C_PlantedC4>()?.read_schema()?;
                    if !planted_c4.m_bC4Activated()? {
                        /* skip this C4 */
                        continue;
                    }

                    let position = planted_c4
                        .m_pGameSceneNode()?
                        .read_schema()?
                        .m_vecAbsOrigin()?;
                    let bomb_site = planted_c4.m_nBombSite()? as u8;

                    match planted_c4_to_radar_state(self, &planted_c4) {
                        Ok(state) => {
                            radar_state.planted_c4 = Some(RadarPlantedC4 {
                                position,
                                bomb_site,
                                state,
                            })
                        }
                        Err(err) => {
                            log::warn!("Failed to generate planted C4 state: {}", err);
                        }
                    }
                }
                "C_C4" => {
                    let c4 = entity_identity.entity_ptr::<C_C4>()?.read_schema()?;
                    if c4.m_bBombPlanted()? {
                        /* this bomb has been planted already */
                        continue;
                    }

                    let owner = c4.m_hOwnerEntity()?;
                    let position = c4.m_pGameSceneNode()?.read_schema()?.m_vecAbsOrigin()?;

                    radar_state.c4_entities.push(RadarC4 {
                        entity_id: entity_identity.handle::<()>()?.get_entity_index(),
                        position,
                        owner_entity_id: if owner.is_valid() {
                            Some(owner.get_entity_index())
                        } else {
                            None
                        },
                    });
                }
                _ => {}
            }
        }

        Ok(radar_state)
    }
}