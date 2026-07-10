//! Sound effects — a thin layer over the toolkit `SoundManager`. All SFX
//! are short synthesized WAVs under `assets/sfx/`. Loading failures
//! degrade to silence (never a crash), so the game runs fine with or
//! without an audio device (headless capture, muted browsers, etc.).

use macroquad_toolkit::audio::SoundManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    /// UI confirm: tool selected, job reassigned.
    Select,
    /// A build site was placed.
    Build,
    /// A goal landed: building finished, victory, factory complete.
    Complete,
    /// A wild beetle was snared.
    Capture,
    /// Raid or famine warning.
    Alarm,
    /// An action was refused (can't build/afford).
    Deny,
    /// The Colossal Worm stirs.
    Worm,
}

impl Sfx {
    fn file(self) -> &'static str {
        match self {
            Sfx::Select => "assets/sfx/select.wav",
            Sfx::Build => "assets/sfx/build.wav",
            Sfx::Complete => "assets/sfx/complete.wav",
            Sfx::Capture => "assets/sfx/capture.wav",
            Sfx::Alarm => "assets/sfx/alarm.wav",
            Sfx::Deny => "assets/sfx/deny.wav",
            Sfx::Worm => "assets/sfx/worm.wav",
        }
    }

    const ALL: [Sfx; 7] = [
        Sfx::Select,
        Sfx::Build,
        Sfx::Complete,
        Sfx::Capture,
        Sfx::Alarm,
        Sfx::Deny,
        Sfx::Worm,
    ];
}

/// The game's sound bank with per-effect volume trims.
pub struct Audio {
    manager: SoundManager<Sfx>,
}

impl Audio {
    /// Loads every SFX; checks the published asset pack first, then loose
    /// files. Missing sounds are skipped silently.
    pub async fn load() -> Self {
        let mut manager = SoundManager::new();
        manager.sfx_volume = 0.6;
        let _ = manager.load_asset_pack("assets.zip").await;
        for sfx in Sfx::ALL {
            let _ = manager.load_sound(sfx, sfx.file()).await;
        }
        Self { manager }
    }

    pub fn play(&self, sfx: Sfx) {
        let vol = match sfx {
            Sfx::Select => 0.6,
            Sfx::Worm => 1.0,
            Sfx::Alarm => 0.9,
            _ => 0.8,
        };
        self.manager.play_sfx(sfx, vol);
    }
}
