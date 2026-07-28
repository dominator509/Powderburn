#![allow(clippy::unwrap_used)]

use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::geom::Facing;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_core::ids::Ap;
use pb_sim::shot::resolve_shot;
use pb_sim::state::{ActorState, SimError, SimState, Stance};

fn make_actor(id: ActorId, pos: TileXY) -> ActorState {
    ActorState {
        faction_id: String::new(),
        is_companion: false,
        attributes: pb_core::Attributes::BALANCED,
        ap: Ap(10),
        position: pos,
        facing: Facing::South,
        sequence: 5,
        hit_points: 20,
        max_hp: 20,
        name: format!("Actor{}", id.0),
        alive: true,
        routed: false,
        wounds: vec![],
        sand: 10,
        max_sand: 10,
        stance: Stance::Standing,
        progression: pb_sim::progression::ActorProgression::new(),
        weapon: "colt_army_1860".into(),
        weapon_profile: Default::default(),
        loaded_rounds: 6,
        weapon_capacity: 6,
        fouling: 0,
        jammed: false,
    }
}

/// Test that a called shot to GunArm emits:
///   ShotHit(hit=true), HitLocation(GunArm), DamageApplied,
///   WoundApplied(wound=Broken), WeaponDropped in order.
#[test]
fn called_shot_to_gunarm_full_pipeline() {
    // Seed 5 gives a legal hit. Give this pipeline fixture an explicit
    // high-damage authored profile so the wound and drop stages must execute.
    let mut state = SimState::new(5, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    state
        .actors
        .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state
        .actors
        .get_mut(&shooter)
        .unwrap()
        .weapon_profile
        .damage_bonus = 20;
    state
        .actors
        .insert(target, make_actor(target, TileXY::new(5, 0)));

    let result = resolve_shot(
        &state,
        shooter,
        target,
        Some(HitLocationType::GunArm),
        true,
        0,
    );
    assert!(result.is_ok(), "Shot should resolve without error");

    let events = result.unwrap();

    // Expected event sequence:
    // 0: Fired
    // 1: ShotHit(hit=true)
    // 2: HitLocation(location=GunArm)
    // 3: DamageApplied
    // 4: WoundApplied(wound=Broken)
    // 5: WeaponDropped
    // 6: SandLost
    // 7-9: muzzle density 3, then density 1 on two forward tiles

    let event_types: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::Fired { .. } => "Fired",
            Event::ShotHit { .. } => "ShotHit",
            Event::HitLocation { .. } => "HitLocation",
            Event::DamageApplied { .. } => "DamageApplied",
            Event::WoundApplied { .. } => "WoundApplied",
            Event::WeaponDropped { .. } => "WeaponDropped",
            Event::SandLost { .. } => "SandLost",
            Event::SmokeDeposited { .. } => "SmokeDeposited",
            _ => "Other",
        })
        .collect();

    assert_eq!(
        event_types,
        vec![
            "Fired",
            "ShotHit",
            "HitLocation",
            "DamageApplied",
            "WoundApplied",
            "WeaponDropped",
            "SandLost",
            "SmokeDeposited",
            "SmokeDeposited",
            "SmokeDeposited",
        ],
        "Event sequence mismatch: {:?}",
        event_types
    );

    // Event 0: Fired
    assert_eq!(
        events[0],
        Event::Fired {
            actor: shooter,
            target,
        }
    );

    // Event 1: ShotHit (hit=true)
    assert_eq!(
        events[1],
        Event::ShotHit {
            actor: shooter,
            target,
            hit: true,
        }
    );

    // Event 2: HitLocation(GunArm)
    assert_eq!(
        events[2],
        Event::HitLocation {
            actor: target,
            location: HitLocationType::GunArm,
        }
    );

    // Event 3: DamageApplied (must be positive)
    if let Event::DamageApplied { actor, damage } = &events[3] {
        assert_eq!(*actor, target);
        assert!(*damage > 0, "Damage should be positive, got {}", damage);
    } else {
        panic!("Event[3] should be DamageApplied");
    }

    // Event 4: WoundApplied(Broken)
    assert_eq!(
        events[4],
        Event::WoundApplied {
            actor: target,
            wound: WoundType::Broken,
        }
    );

    // Event 5: WeaponDropped
    assert_eq!(
        events[5],
        Event::WeaponDropped {
            actor: target,
            item: "colt_army_1860".to_string(),
        }
    );

    // Event 6: the hit spends Sand.
    assert_eq!(
        events[6],
        Event::SandLost {
            actor: target,
            amount: 4,
        }
    );

    // Events 7-9: SmokeDeposited at the muzzle and two forward tiles.
    assert_eq!(
        events[7],
        Event::SmokeDeposited {
            tile: TileXY::new(0, 0),
            density: 3,
        }
    );
    assert_eq!(
        events[8],
        Event::SmokeDeposited {
            tile: TileXY::new(0, 1),
            density: 1,
        }
    );
    assert_eq!(
        events[9],
        Event::SmokeDeposited {
            tile: TileXY::new(0, 2),
            density: 1,
        }
    );
}

/// Stage-one legality refuses a shot outside the weapon's maximum range.
#[test]
fn shot_out_of_range_is_rejected() {
    let mut state = SimState::new(42, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    state
        .actors
        .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state
        .actors
        .insert(target, make_actor(target, TileXY::new(60, 0)));

    let result = resolve_shot(&state, shooter, target, None, false, 0);
    assert_eq!(result, Err(SimError::OutOfRange(target)));
}

#[test]
fn rain_doubles_cap_and_ball_misfire_after_luck_and_fouling() {
    let shooter = ActorId(1);
    let target = ActorId(2);
    let seed = (1..=10_000).find(|seed| {
        let roll = pb_rng::PbRng::draw(*seed, 1, 0, shooter.0, pb_rng::StreamTag::Misfire, 1, 100);
        (6..=10).contains(&roll)
    });
    assert!(
        seed.is_some(),
        "bounded seed corpus contains a rain-only misfire"
    );
    let seed = seed.unwrap();

    let make_state = |weather| {
        let mut state = SimState::new(seed, 1);
        state.weather = weather;
        let mut armed = make_actor(shooter, TileXY::new(0, 0));
        armed.fouling = 3; // clear 5%; rain 10% after LUCK
        state.actors.insert(shooter, armed);
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(2, 0)));
        state
    };

    let clear = resolve_shot(
        &make_state(pb_sim::environment::Weather::Clear),
        shooter,
        target,
        None,
        false,
        0,
    )
    .unwrap();
    let rain = resolve_shot(
        &make_state(pb_sim::environment::Weather::Rain),
        shooter,
        target,
        None,
        false,
        0,
    )
    .unwrap();
    assert!(!clear
        .iter()
        .any(|event| matches!(event, Event::Misfire { .. })));
    assert!(rain
        .iter()
        .any(|event| matches!(event, Event::Misfire { .. })));
}

#[test]
fn night_restricts_shots_to_authored_sight_radius() {
    let shooter = ActorId(1);
    let target = ActorId(2);
    let mut state = SimState::new(5, 1);
    state.light_level = pb_sim::environment::LightLevel::Night;
    state
        .actors
        .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state
        .actors
        .insert(target, make_actor(target, TileXY::new(5, 0)));
    assert_eq!(
        resolve_shot(&state, shooter, target, None, false, 0),
        Err(SimError::NoLineOfSight(shooter, target))
    );
}

/// Test that shooting a dead actor produces no events.
#[test]
fn shot_target_dead() {
    let mut state = SimState::new(42, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    let mut t = make_actor(target, TileXY::new(5, 0));
    t.alive = false;
    state
        .actors
        .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state.actors.insert(target, t);

    let result = resolve_shot(&state, shooter, target, None, false, 0).unwrap();
    assert!(
        result.is_empty(),
        "Shooting a dead target should produce no events"
    );
}

/// Test that the shot pipeline returns proper events.
#[test]
fn shot_returns_shot_hit_event() {
    let mut state = SimState::new(0, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    state
        .actors
        .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state
        .actors
        .insert(target, make_actor(target, TileXY::new(10, 0)));

    let result = resolve_shot(&state, shooter, target, None, false, 0).unwrap();
    assert!(!result.is_empty(), "Should not be empty");
    assert!(matches!(
        result.first(),
        Some(Event::Fired { actor, target: event_target })
            if *actor == shooter && *event_target == target
    ));
}

/// Test that unknown shooter returns error.
#[test]
fn shot_unknown_actor_returns_error() {
    let state = SimState::new(42, 1);
    let result = resolve_shot(&state, ActorId(1), ActorId(2), None, false, 0);
    assert!(result.is_err());
}
