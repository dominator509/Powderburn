#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod compute_ids {
    use pb_core::hash::hash_state;

    #[test]
    fn print_actor_ids() {
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
        let mut out = String::new();
        for id in &ids {
            let h = hash_state(id.as_bytes());
            let actor_id = u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
            out.push_str(&format!(
                "{:30} -> ActorId({}) (hex: {:08x})\n",
                id, actor_id, actor_id
            ));
        }
        std::fs::write("/tmp/actor_ids.txt", &out).expect("write failed");
    }
}
