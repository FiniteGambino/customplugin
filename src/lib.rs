#![allow(non_snake_case)]

//! Sleep Sound Full Length
//!
//! In the base game every fighter's `sound_sleepstart` script plays
//! `se_common_sleep` with PLAY_STATUS, which the game stops as soon as the
//! short "falling asleep" status ends. This plugin replaces that script with a
//! PLAY_SE of the same sound, which is not tied to the status, so the whole
//! clip plays. It covers every source of sleep (Sing, Snooze, Peach/Daisy
//! Blossom, Darkrai, etc.) because it hooks the victim's sleep status.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Debug recorder: appends a line to sd:/sleep_sound_debug.txt (first 120 lines only).
static TRACES: AtomicUsize = AtomicUsize::new(0);
fn trace(msg: &str) {
    if TRACES.fetch_add(1, Ordering::Relaxed) > 120 {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("sd:/sleep_sound_debug.txt")
    {
        let _ = writeln!(f, "{}", msg);
    }
}


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
    trace("play_sleep: start");
    let module_accessor: *mut _ = sv_system::battle_object_module_accessor(agent.lua_state_agent);
    trace(&format!("play_sleep: module_accessor = {:p}", module_accessor));
    if module_accessor.is_null() {
        trace("play_sleep: null accessor, falling back to PLAY_SE");
        play_se(agent, "se_common_sleep");
        return;
    }
    trace("play_sleep: calling SoundModule::play_se");
    let handle = SoundModule::play_se(
        module_accessor,
        Hash40::new("se_common_sleep"),
        true,
        false,
        false,
        false,
        enSEType(0),
    );
    trace(&format!("play_sleep: play_se returned handle = {:#x}", handle));
    if handle != 0 {
        trace("play_sleep: calling set_se_vol");
        SoundModule::set_se_vol(module_accessor, handle as i32, VOLUME, 0);
        trace("play_sleep: set_se_vol done");
    }
}

unsafe extern "C" fn sound_sleepstart(agent: &mut L2CAgentBase) {
    trace("sound_sleepstart: called");
    let go = is_excute(agent);
    trace(&format!("sound_sleepstart: is_excute = {}", go));
    if go {
        play_sleep(agent);
    }
    trace("sound_sleepstart: finished");
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
    // Write any panic message to the debug file (the default handler's message is lost).
    std::panic::set_hook(Box::new(|info| {
        trace(&format!("PANIC: {}", info));
    }));
    trace("plugin loaded");
    for name in FIGHTERS {
        Agent::new(*name)
            .acmd("sound_sleepstart", sound_sleepstart, Priority::Low)
            .install();
    }
    Agent::new("pickel")
        .acmd("sound_sleepstart", sound_sleepstart_pickel, Priority::Low)
        .install();
}
