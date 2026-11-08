#![allow(non_snake_case)]

//! Sleep Sound Full Length
//!
//! In the base game every fighter's `sound_sleepstart` script plays
//! `se_common_sleep` with PLAY_STATUS, which the game stops as soon as the
//! short "falling asleep" status ends. This plugin replaces that script with a
//! PLAY_SE of the same sound, which is not tied to the status, so the whole
//! clip plays. It covers every source of sleep (Sing, Snooze, Peach/Daisy
//! Blossom, Darkrai, etc.) because it hooks the victim's sleep status.

use std::sync::atomic::{AtomicBool, Ordering};

use smashline::skyline_smash::app::{
    enSEType, lua_bind::{SoundModule, WorkModule}, sv_animcmd, sv_system, utility,
    BattleObjectModuleAccessor,
};
use smashline::{Agent, ExecStop, Hash40, L2CAgentBase, L2CFighterCommon, L2CValue, Main, Priority};

/// Fighters whose vanilla `sound_sleepstart` is only
/// `PLAY_STATUS(se_common_sleep)` (checked against WuBoytH/SSBU-Dumped-Scripts).
/// Steve (`pickel`) has an extra sound and is handled separately below.
const FIGHTERS: &[&str] = &[
    "bayonetta", "brave", "buddy", "captain", "chrom", "cloud", "daisy", "dedede",
    "demon", "diddy", "dolly", "donkey", "duckhunt", "edge", "eflame", "elight",
    "falco", "fox", "gamewatch", "ganon", "gaogaen", "gekkouga", "ike", "inkling",
    "jack", "kamui", "ken", "kirby", "koopa", "koopajr", "krool", "link",
    "littlemac", "lucario", "lucas", "lucina", "luigi", "mario", "mariod", "marth",
    "master", "metaknight", "mewtwo", "miienemyf", "miienemyg", "miienemys",
    "miifighter", "miigunner", "miiswordsman", "murabito", "nana", "ness",
    "packun", "pacman", "palutena", "peach", "pfushigisou", "pichu", "pikachu",
    "pikmin", "pit", "pitb", "plizardon", "popo", "purin", "pzenigame", "reflet",
    "richter", "ridley", "robot", "rockman", "rosetta", "roy", "ryu", "samus",
    "samusd", "sheik", "shizue", "shulk", "simon", "snake", "sonic", "szerosuit",
    "tantan", "toonlink", "trail", "wario", "wiifit", "wolf", "yoshi",
    "younglink", "zelda",
];

/// Same as smash_script's `is_excute`.
///
/// The game's animcmd functions read their arguments from the agent's lua
/// stack, and `pop_lua_stack(1)` reads the value in the first slot. If
/// anything is left on the stack, that slot holds a leftover instead of the
/// result, and `get_bool()` panics ("L2CValue: ... not a bool"), which froze
/// the game on sleep in the volume builds. So, like smash_script, clear the
/// stack before asking and after reading, and never panic on a bad value.
unsafe fn is_excute(agent: &mut L2CAgentBase) -> bool {
    agent.clear_lua_stack();
    sv_animcmd::is_excute(agent.lua_state_agent);
    let ret = agent.pop_lua_stack(1).try_get_bool().unwrap_or(true);
    agent.clear_lua_stack();
    ret
}

/// Same as smash_script's `PLAY_SE` macro: push the hash, call the animcmd.
unsafe fn play_se(agent: &mut L2CAgentBase, name: &str) {
    agent.clear_lua_stack();
    let mut hash = L2CValue::new_int(Hash40::new(name).hash);
    agent.push_lua_stack(&mut hash);
    sv_animcmd::PLAY_SE(agent.lua_state_agent);
    agent.clear_lua_stack();
}

/// Playback volume for the sleep sound. 1.0 = normal. Other mods use 1.3-1.75;
/// raise this to make it louder, lower it if it distorts.
/// 1.75 tested stable on 2026-11-08 but too quiet; 2.5 is about +8 dB, 3.5 about +11 dB.
const VOLUME: f32 = 3.5;

/// Plays se_common_sleep and boosts its volume.
///
/// The fighter's module accessor comes from the game's own lookup (not from a
/// struct field, whose layout can differ between game versions). If anything
/// looks wrong we fall back to the plain PLAY_SE that is known to work.
unsafe fn play_sleep(agent: &mut L2CAgentBase) {
    let module_accessor: *mut _ = sv_system::battle_object_module_accessor(agent.lua_state_agent);
    if module_accessor.is_null() {
        play_se(agent, "se_common_sleep");
        return;
    }
    let handle = SoundModule::play_se(
        module_accessor,
        Hash40::new("se_common_sleep"),
        true,
        false,
        false,
        false,
        enSEType(0),
    );
    if handle != 0 {
        SoundModule::set_se_vol(module_accessor, handle as i32, VOLUME, 0);
    }
}

unsafe extern "C" fn sound_sleepstart(agent: &mut L2CAgentBase) {
    if is_excute(agent) {
        play_sleep(agent);
    }
    // Leave the lua stack empty for whatever runs next on this agent.
    agent.clear_lua_stack();
}

/// Steve's vanilla script also plays a wood sound on frame 12; keep it.
unsafe extern "C" fn sound_sleepstart_pickel(agent: &mut L2CAgentBase) {
    if is_excute(agent) {
        play_sleep(agent);
    }
    sv_animcmd::frame(agent.lua_state_agent, 12.0);
    if is_excute(agent) {
        play_se(agent, "se_pickel_special_n03_wood");
    }
    agent.clear_lua_stack();
}

// ---------------------------------------------------------------------------
// Louder shield break and final-stock hit
//
// se_common_guardbreak (shield breaks) and se_common_finishhit (the red-screen
// final hit) are started by the game's own code, not by a fighter script, so
// they can't be replaced like the sleep sound. Instead, every frame we look at
// each fighter: the moment one of these sounds starts playing on that fighter,
// we stop it and immediately start it again at a higher volume.
// ---------------------------------------------------------------------------

/// Sounds to boost and their volume (1.0 = normal).
const BOOSTED: [(&str, f32); 2] = [
    ("se_common_guardbreak", 2.5),
    ("se_common_finishhit", 2.5),
];

const ENTRY_ID: i32 = 0x10000000; // FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID
const KIND_NANA: i32 = 0x4C; // FIGHTER_KIND_NANA (shares Popo's entry ID)

/// Whether each boosted sound was already playing last frame, per fighter
/// (8 entry IDs, plus 8 more for Nana).
#[allow(clippy::declare_interior_mutable_const)]
const NOT_PLAYING: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];
static WAS_PLAYING: [[AtomicBool; 2]; 16] = [NOT_PLAYING; 16];

unsafe extern "C" fn boost_common_sounds(fighter: &mut L2CFighterCommon) {
    let boma: *mut BattleObjectModuleAccessor =
        sv_system::battle_object_module_accessor(fighter.lua_state_agent);
    if boma.is_null() {
        return;
    }
    let entry = WorkModule::get_int(boma, ENTRY_ID);
    if !(0..8).contains(&entry) {
        return;
    }
    let slot = entry as usize + if utility::get_kind(&mut *boma) == KIND_NANA { 8 } else { 0 };
    let Some(state) = WAS_PLAYING.get(slot) else { return };

    for (i, (name, volume)) in BOOSTED.iter().enumerate() {
        let hash = Hash40::new(name);
        let playing = SoundModule::is_playing(boma, hash);
        let was = state[i].load(Ordering::Relaxed);
        if playing && !was {
            SoundModule::stop_se(boma, hash, 0);
            let handle = SoundModule::play_se(boma, hash, true, false, false, false, enSEType(0));
            if handle != 0 {
                SoundModule::set_se_vol(boma, handle as i32, *volume, 0);
            }
            state[i].store(true, Ordering::Relaxed);
        } else if !playing && was {
            state[i].store(false, Ordering::Relaxed);
        }
    }
}

#[skyline::main(name = "sleep_sound_full")]
pub fn main() {
    for name in FIGHTERS {
        Agent::new(*name)
            .acmd("sound_sleepstart", sound_sleepstart, Priority::Low)
            // Main runs on normal frames, ExecStop during hitlag (the final
            // hit starts its sound while everyone is frozen).
            .on_line(Main, boost_common_sounds)
            .on_line(ExecStop, boost_common_sounds)
            .install();
    }
    Agent::new("pickel")
        .acmd("sound_sleepstart", sound_sleepstart_pickel, Priority::Low)
        .on_line(Main, boost_common_sounds)
        .on_line(ExecStop, boost_common_sounds)
        .install();
}
