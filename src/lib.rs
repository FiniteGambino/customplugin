#![allow(non_snake_case)]

//! Sleep Sound Full Length
//!
//! In the base game every fighter's `sound_sleepstart` script plays
//! `se_common_sleep` with PLAY_STATUS, which the game stops as soon as the
//! short "falling asleep" status ends. This plugin replaces that script with a
//! PLAY_SE of the same sound, which is not tied to the status, so the whole
//! clip plays. It covers every source of sleep (Sing, Snooze, Peach/Daisy
//! Blossom, Darkrai, etc.) because it hooks the victim's sleep status.

use smashline::skyline_smash::app::{enSEType, lua_bind::SoundModule, sv_animcmd, sv_system};
use smashline::{Agent, Hash40, L2CAgentBase, L2CValue, Priority};

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
unsafe fn is_excute(agent: &mut L2CAgentBase) -> bool {
    sv_animcmd::is_excute(agent.lua_state_agent);
    agent.pop_lua_stack(1).get_bool()
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
/// Starts at 1.75, the highest value other mods are known to use.
const VOLUME: f32 = 1.75;

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
}

#[skyline::main(name = "sleep_sound_full")]
pub fn main() {
    for name in FIGHTERS {
        Agent::new(*name)
            .acmd("sound_sleepstart", sound_sleepstart, Priority::Low)
            .install();
    }
    Agent::new("pickel")
        .acmd("sound_sleepstart", sound_sleepstart_pickel, Priority::Low)
        .install();
}
