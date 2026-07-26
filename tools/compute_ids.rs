fn main() {
    let names = [
        "e_bandit_02",
        "e_shooter",
        "e_ally_01",
        "e_ally_02",
        "e_enemy_01",
        "e_enemy_02",
        "c_whitehorse",
    ];
    for name in &names {
        let h = pb_core::hash::hash_state(name.as_bytes());
        let id = u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
        println!("  {} -> ActorId({})", name, id);
    }
}
