//! Authored audio cue metadata.

/// A sound effect identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    PistolShot,
    RifleShot,
    Hit,
    Miss,
    Click,
    Select,
    Victory,
    Death,
    Move,
    Reload,
    FrontierTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cue {
    pub filename: &'static str,
    pub speaker: &'static str,
    pub subtitle: &'static str,
    pub music: bool,
}

impl Sfx {
    pub const ALL: [Self; 11] = [
        Self::PistolShot,
        Self::RifleShot,
        Self::Hit,
        Self::Miss,
        Self::Click,
        Self::Select,
        Self::Victory,
        Self::Death,
        Self::Move,
        Self::Reload,
        Self::FrontierTheme,
    ];

    pub const fn cue(self) -> Cue {
        match self {
            Self::PistolShot => Cue {
                filename: "pistol_shot.wav",
                speaker: "Battlefield",
                subtitle: "Pistol shot",
                music: false,
            },
            Self::RifleShot => Cue {
                filename: "rifle_shot.wav",
                speaker: "Battlefield",
                subtitle: "Rifle shot",
                music: false,
            },
            Self::Hit => Cue {
                filename: "hit.wav",
                speaker: "Battlefield",
                subtitle: "Bullet strikes",
                music: false,
            },
            Self::Miss => Cue {
                filename: "miss.wav",
                speaker: "Battlefield",
                subtitle: "Bullet passes wide",
                music: false,
            },
            Self::Click => Cue {
                filename: "click.wav",
                speaker: "Interface",
                subtitle: "Click",
                music: false,
            },
            Self::Select => Cue {
                filename: "select.wav",
                speaker: "Company",
                subtitle: "Companion selected",
                music: false,
            },
            Self::Victory => Cue {
                filename: "victory.wav",
                speaker: "Music",
                subtitle: "Victory theme",
                music: true,
            },
            Self::Death => Cue {
                filename: "death.wav",
                speaker: "Battlefield",
                subtitle: "A combatant falls",
                music: false,
            },
            Self::Move => Cue {
                filename: "move.wav",
                speaker: "Battlefield",
                subtitle: "Footsteps",
                music: false,
            },
            Self::Reload => Cue {
                filename: "reload.wav",
                speaker: "Battlefield",
                subtitle: "Weapon reloads",
                music: false,
            },
            Self::FrontierTheme => Cue {
                filename: "frontier_theme.wav",
                speaker: "Music",
                subtitle: "Frontier theme",
                music: true,
            },
        }
    }

    pub const fn filename(self) -> &'static str {
        self.cue().filename
    }
}
