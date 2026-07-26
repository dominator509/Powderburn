fn main() {
    let ids = [
        "e_ally_01",
        "e_ally_02",
        "e_enemy_01",
        "e_enemy_02",
        "e_shooter",
        "e_target",
        "e_bandit_02",
        "c_whitehorse",
    ];
    for id in &ids {
        let h = pb_core::hash::hash_state(id.as_bytes());
        let actor_id = u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
        println!("{:30} -> ActorId({}) (hex: {:08x})", id, actor_id, actor_id);
    }
}
