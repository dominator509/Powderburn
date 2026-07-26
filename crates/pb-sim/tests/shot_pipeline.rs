use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_sim::shot::resolve_shot;
use pb_sim::state::{SimState, ActorState, Stance};
use pb_core::ids::Ap;
use pb_core::geom::Facing;

fn make_actor(id: ActorId, pos: TileXY) -> ActorState {
    ActorState {
        ap: Ap(10),
        position: pos,
        facing: Facing::South,
        sequence: 5,
        hit_points: 20,
        max_hp: 20,
        name: format!("Actor{}", id.0),
        alive: true,
        wounds: vec![],
        sand: 10,
        max_sand: 10,
        stance: Stance::Standing,
    }
}

/// Test that a called shot to GunArm emits:
///   ShotHit(hit=true), HitLocation(GunArm), DamageApplied,
///   WoundApplied(wound=Broken), WeaponDropped in order.
#[test]
fn called_shot_to_gunarm_full_pipeline() {
    // seed=5 gives ToHit=27 (<45 for GunArm called shot), misfire=38 (>=5),
    // damage=23, so we get a full hit with wound and weapon drop.
    let mut state = SimState::new(5, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    state.actors.insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state.actors.insert(target, make_actor(target, TileXY::new(5, 0)));

    let result = resolve_shot(&state, shooter, target, Some(HitLocationType::GunArm));
    assert!(result.is_ok(), "Shot should resolve without error");

    let events = result.unwrap();

    // Expected event sequence:
    // 0: ShotHit(hit=true)
    // 1: HitLocation(location=GunArm)
    // 2: DamageApplied
    // 3: WoundApplied(wound=Broken)
    // 4: WeaponDropped

    let event_types: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::ShotHit { .. } => "ShotHit",
            Event::HitLocation { .. } => "HitLocation",
            Event::DamageApplied { .. } => "DamageApplied",
            Event::WoundApplied { .. } => "WoundApplied",
            Event::WeaponDropped { .. } => "WeaponDropped",
            _ => "Other",
        })
        .collect();

    assert_eq!(
        event_types,
        vec![
            "ShotHit",
            "HitLocation",
            "DamageApplied",
            "WoundApplied",
            "WeaponDropped",
        ],
        "Event sequence mismatch: {:?}",
        event_types
    );

    // Event 0: ShotHit (hit=true)
    assert_eq!(
        events[0],
        Event::ShotHit {
            actor: shooter,
            target,
            hit: true,
        }
    );

    // Event 1: HitLocation(GunArm)
    assert_eq!(
        events[1],
        Event::HitLocation {
            actor: target,
            location: HitLocationType::GunArm,
        }
    );

    // Event 2: DamageApplied (must be positive)
    if let Event::DamageApplied { actor, damage } = &events[2] {
        assert_eq!(*actor, target);
        assert!(*damage > 0, "Damage should be positive, got {}", damage);
    } else {
        panic!("Event[2] should be DamageApplied");
    }

    // Event 3: WoundApplied(Broken)
    assert_eq!(
        events[3],
        Event::WoundApplied {
            actor: target,
            wound: WoundType::Broken,
        }
    );

    // Event 4: WeaponDropped
    assert_eq!(
        events[4],
        Event::WeaponDropped {
            actor: target,
            item: "colt_army_1860".to_string(),
        }
    );
}

/// Test that a shot resolves without crashing even with distant target.
#[test]
fn shot_out_of_range_still_resolves() {
    let mut state = SimState::new(42, 1);
    let shooter = ActorId(1);
    let target = ActorId(2);
    state.actors.insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state.actors.insert(target, make_actor(target, TileXY::new(60, 0)));

    let result = resolve_shot(&state, shooter, target, None);
    assert!(
        result.is_ok(),
        "Shot pipeline should handle range without crashing"
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
    state.actors.insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state.actors.insert(target, t);

    let result = resolve_shot(&state, shooter, target, None).unwrap();
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
    state.actors.insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
    state.actors.insert(target, make_actor(target, TileXY::new(10, 0)));

    let result = resolve_shot(&state, shooter, target, None).unwrap();
    assert!(!result.is_empty(), "Should not be empty");
    // First event should always be ShotHit or Misfire
    match &result[0] {
        Event::ShotHit { .. } => { /* ok */ }
        Event::Misfire { .. } => { /* ok */ }
        other => panic!("First event should be ShotHit or Misfire, got {:?}", other),
    }
}

/// Test that unknown shooter returns error.
#[test]
fn shot_unknown_actor_returns_error() {
    let state = SimState::new(42, 1);
    let result = resolve_shot(&state, ActorId(1), ActorId(2), None);
    assert!(result.is_err());
}
