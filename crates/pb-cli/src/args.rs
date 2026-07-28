//! Manual argument parser for pbcli subcommands.
//! No clap dependency — parses env::args() directly.

use std::ffi::OsString;
use std::path::PathBuf;

/// Parsed pbcli arguments.
#[derive(Debug, Clone)]
pub struct Args {
    /// Subcommand name.
    pub subcommand: String,
    /// Path to the content root (--content-root or -C).
    pub content_root: Option<PathBuf>,
    /// Path to a journal file (--journal or -j).
    pub journal: Option<PathBuf>,
    /// Scenario id to run.
    pub scenario: Option<String>,
    /// Output path for save (--output or -o, also --save, --out).
    pub output: Option<PathBuf>,
    /// Input path for load/resume (--input or -i, also --resume).
    pub input: Option<PathBuf>,
    /// Path to a campaign save file (--campaign, also --save).
    pub campaign_path: Option<PathBuf>,
    /// Campaign seed (--seed, also --company-seed).
    pub seed: Option<u64>,
    /// Whether to emit state hash.
    pub emit_hash: bool,
    /// Whether to emit events.
    pub emit_events: bool,
    /// Suspend simulation at this tick.
    pub suspend_at_tick: Option<u64>,
    /// Expected hash for replay matching.
    pub expect: Option<String>,
    /// Number of iterations for benchmarks.
    pub iterations: Option<u64>,
    /// Scenario name for benchmarks.
    pub bench_scenario: Option<String>,
    /// Asset root for provenance checks.
    pub asset_root: Option<PathBuf>,
    /// Path to a golden file for refresh.
    pub golden_path: Option<PathBuf>,
    /// Image file path for image stats.
    pub image_path: Option<PathBuf>,
    /// Atlas output path.
    pub atlas_output: Option<PathBuf>,
    /// Remaining positional arguments.
    pub positional: Vec<String>,

    /// Number of frames for bench frame (--frames).
    pub frames: Option<u64>,

    // === NEW FLAGS (live-fire.sh support) ===
    /// Whether to emit outcome line (campaign play).
    pub emit_outcome: bool,
    /// Whether to emit available missions (campaign play).
    pub emit_manifest: bool,
    /// Whether to emit budget timing (bench turn).
    pub emit_budget: bool,
    /// Whether to check for dangling references (campaign audit).
    pub dangling_refs: bool,
    /// Whether to create a new campaign before playing (--from-new).
    pub from_new: bool,
    /// Path to a script file for automated journal playback.
    pub script_path: Option<PathBuf>,
    /// GPU adapter path for headless capture.
    pub adapter: Option<String>,
    /// Campaign branch choice (--choice <value>).
    pub choice: Option<String>,
    /// Whether to emit all SPEC-007 metrics (--emit-metrics).
    pub emit_metrics: bool,
    /// Whether capture should also run timing benchmark (--bench).
    pub capture_bench: bool,
    /// Drive a campaign mission through the real kernel with deterministic
    /// scripted tactics.
    pub autoplay: bool,
    /// Named recruited companion the hostile script must kill before the
    /// company completes an autoplayed mission.
    pub required_casualty: Option<String>,
    /// Trace every AI candidate considered for this actor id or instance name.
    pub trace_actor: Option<String>,
    /// Trace all ten stages for shot commands.
    pub trace_shot: bool,
    /// Trace every counter-addressed RNG draw.
    pub trace_rng: bool,
    /// Loopback TCP port for the feature-gated replay server.
    pub port: Option<u16>,
}

impl Args {
    /// Parse arguments from environment variable.
    pub fn from_env() -> Result<Self, String> {
        let raw: Vec<OsString> = std::env::args_os().collect();
        Self::parse(raw)
    }

    /// Parse from a vector of OsStrings (first element is the program name, skipped).
    pub fn parse(raw: Vec<OsString>) -> Result<Self, String> {
        let mut positional: Vec<String> = Vec::new();
        let mut content_root: Option<PathBuf> = None;
        let mut journal: Option<PathBuf> = None;
        let mut scenario: Option<String> = None;
        let mut output: Option<PathBuf> = None;
        let mut input: Option<PathBuf> = None;
        let mut campaign_path: Option<PathBuf> = None;
        let mut seed: Option<u64> = None;
        let mut emit_hash = false;
        let mut emit_events = false;
        let mut suspend_at_tick: Option<u64> = None;
        let mut expect: Option<String> = None;
        let mut iterations: Option<u64> = None;
        let mut bench_scenario: Option<String> = None;
        let mut asset_root: Option<PathBuf> = None;
        let mut golden_path: Option<PathBuf> = None;
        let mut image_path: Option<PathBuf> = None;
        let mut atlas_output: Option<PathBuf> = None;
        let mut emit_outcome = false;
        let mut emit_manifest = false;
        let mut emit_budget = false;
        let mut dangling_refs = false;
        let mut from_new = false;
        let mut script_path: Option<PathBuf> = None;
        let mut adapter: Option<String> = None;
        let mut choice: Option<String> = None;
        let mut emit_metrics = false;
        let mut frames: Option<u64> = None;
        let mut capture_bench = false;
        let mut autoplay = false;
        let mut required_casualty: Option<String> = None;
        let mut trace_actor: Option<String> = None;
        let mut trace_shot = false;
        let mut trace_rng = false;
        let mut port: Option<u16> = None;

        let mut i = 1; // skip program name
        while i < raw.len() {
            let arg = raw[i]
                .to_str()
                .ok_or_else(|| format!("non-UTF-8 argument at position {}", i))?
                .to_string();

            match arg.as_str() {
                "--content-root" | "-C" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--content-root requires a value")?;
                    content_root = Some(PathBuf::from(val));
                }
                "--journal" | "-j" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--journal requires a value")?;
                    journal = Some(PathBuf::from(val));
                }
                "--scenario" | "-s" | "--mission" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--scenario/--mission requires a value")?;
                    scenario = Some(
                        val.to_str()
                            .ok_or("--scenario value not UTF-8")?
                            .to_string(),
                    );
                }
                "--output" | "-o" | "--out" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--output/--out requires a value")?;
                    output = Some(PathBuf::from(val));
                }
                "--input" | "-i" | "--resume" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--input/--resume requires a value")?;
                    input = Some(PathBuf::from(val));
                }
                "--campaign" | "--save" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--campaign/--save requires a value")?;
                    let p = PathBuf::from(val);
                    campaign_path = Some(p.clone());
                    // --save also sets output so sim --suspend-at-tick --save works
                    output.get_or_insert(p);
                }
                "--seed" | "--company-seed" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--seed/--company-seed requires a value")?;
                    let s: u64 = val
                        .to_str()
                        .ok_or("--seed value not UTF-8")?
                        .parse()
                        .map_err(|e| format!("invalid seed: {}", e))?;
                    seed = Some(s);
                }
                "--emit-hash" => {
                    emit_hash = true;
                }
                "--emit-events" => {
                    emit_events = true;
                }
                "--emit-outcome" => {
                    emit_outcome = true;
                }
                "--emit-manifest" => {
                    emit_manifest = true;
                }
                "--emit-budget" => {
                    emit_budget = true;
                }
                "--dangling-refs" => {
                    dangling_refs = true;
                }
                "--from-new" => {
                    from_new = true;
                }
                "--script" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--script requires a value")?;
                    script_path = Some(PathBuf::from(val));
                }
                "--adapter" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--adapter requires a value")?;
                    adapter = Some(val.to_str().ok_or("--adapter value not UTF-8")?.to_string());
                }
                "--choice" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--choice requires a value")?;
                    choice = Some(val.to_str().ok_or("--choice value not UTF-8")?.to_string());
                }
                "--suspend-at-tick" | "--tick" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--suspend-at-tick requires a value")?;
                    let t: u64 = val
                        .to_str()
                        .ok_or("not UTF-8")?
                        .parse()
                        .map_err(|e| format!("invalid tick: {}", e))?;
                    suspend_at_tick = Some(t);
                }
                "--expect" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--expect requires a value")?;
                    expect = Some(val.to_str().ok_or("--expect value not UTF-8")?.to_string());
                }
                "--iterations" | "-n" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--iterations requires a value")?;
                    let n: u64 = val
                        .to_str()
                        .ok_or("not UTF-8")?
                        .parse()
                        .map_err(|e| format!("invalid iterations: {}", e))?;
                    iterations = Some(n);
                }
                "--bench-scenario" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--bench-scenario requires a value")?;
                    bench_scenario = Some(val.to_str().ok_or("not UTF-8")?.to_string());
                }
                "--asset-root" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--asset-root requires a value")?;
                    asset_root = Some(PathBuf::from(val));
                }
                "--golden" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--golden requires a value")?;
                    golden_path = Some(PathBuf::from(val));
                }
                "--image" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--image requires a value")?;
                    image_path = Some(PathBuf::from(val));
                }
                "--atlas-output" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--atlas-output requires a value")?;
                    atlas_output = Some(PathBuf::from(val));
                }
                "--emit-metrics" => {
                    emit_metrics = true;
                }
                "--frames" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--frames requires a value")?;
                    let n: u64 = val
                        .to_str()
                        .ok_or("not UTF-8")?
                        .parse()
                        .map_err(|e| format!("invalid frames: {}", e))?;
                    frames = Some(n);
                }
                "--bench" => {
                    capture_bench = true;
                }
                "--autoplay" => {
                    autoplay = true;
                }
                "--required-casualty" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--required-casualty requires a value")?;
                    required_casualty = Some(
                        val.to_str()
                            .ok_or("--required-casualty value not UTF-8")?
                            .to_string(),
                    );
                }
                "--trace-actor" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--trace-actor requires a value")?;
                    trace_actor = Some(
                        val.to_str()
                            .ok_or("--trace-actor value not UTF-8")?
                            .to_string(),
                    );
                }
                "--trace-shot" => {
                    trace_shot = true;
                }
                "--trace-rng" => {
                    trace_rng = true;
                }
                "--port" => {
                    i += 1;
                    let val = raw.get(i).ok_or("--port requires a value")?;
                    port = Some(
                        val.to_str()
                            .ok_or("--port value not UTF-8")?
                            .parse()
                            .map_err(|e| format!("invalid port: {}", e))?,
                    );
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown flag: {}", arg));
                }
                _ => {
                    positional.push(arg);
                }
            }
            i += 1;
        }

        // First positional is the subcommand
        let subcommand = positional.first().cloned().unwrap_or_default();

        Ok(Args {
            subcommand,
            content_root,
            journal,
            scenario,
            output,
            input,
            campaign_path,
            seed,
            emit_hash,
            emit_events,
            suspend_at_tick,
            expect,
            iterations,
            bench_scenario,
            asset_root,
            golden_path,
            image_path,
            atlas_output,
            positional,
            frames,
            emit_outcome,
            emit_manifest,
            emit_budget,
            dangling_refs,
            from_new,
            script_path,
            adapter,
            choice,
            emit_metrics,
            capture_bench,
            autoplay,
            required_casualty,
            trace_actor,
            trace_shot,
            trace_rng,
            port,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_complete_locked_flag_surface() {
        let raw = [
            "pbcli",
            "sim",
            "--content-root",
            "content",
            "--journal",
            "turns.jrnl",
            "--scenario",
            "prov_full_battle",
            "--out",
            "frame.png",
            "--resume",
            "resume.pbsave",
            "--save",
            "campaign.pbsave",
            "--seed",
            "1867",
            "--emit-hash",
            "--emit-events",
            "--emit-outcome",
            "--emit-manifest",
            "--emit-budget",
            "--dangling-refs",
            "--from-new",
            "--script",
            "branch.script",
            "--adapter",
            "gl",
            "--choice",
            "warn_adobe_walls",
            "--suspend-at-tick",
            "144",
            "--expect",
            "deadbeef",
            "--iterations",
            "20",
            "--bench-scenario",
            "prov_sixty_actors",
            "--asset-root",
            "assets",
            "--golden",
            "golden.hash",
            "--image",
            "capture.png",
            "--atlas-output",
            "atlas.png",
            "--emit-metrics",
            "--frames",
            "60",
            "--bench",
            "--autoplay",
            "--required-casualty",
            "c_whitehorse",
            "--trace-actor",
            "e_shooter",
            "--trace-shot",
            "--trace-rng",
            "--port",
            "7878",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let args = Args::parse(raw).expect("complete flag surface");
        assert_eq!(args.subcommand, "sim");
        assert_eq!(args.seed, Some(1867));
        assert_eq!(args.suspend_at_tick, Some(144));
        assert_eq!(args.iterations, Some(20));
        assert_eq!(args.frames, Some(60));
        assert_eq!(args.port, Some(7878));
        assert!(args.emit_hash && args.emit_events && args.emit_metrics);
        assert!(args.autoplay && args.trace_shot && args.trace_rng);
        assert_eq!(args.trace_actor.as_deref(), Some("e_shooter"));
        assert_eq!(args.required_casualty.as_deref(), Some("c_whitehorse"));
    }

    #[test]
    fn rejects_unknown_and_malformed_values() {
        let unknown = vec![OsString::from("pbcli"), OsString::from("--unknown")];
        assert!(Args::parse(unknown).is_err());
        let malformed = ["pbcli", "sim", "--frames", "many"]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(Args::parse(malformed).is_err());
    }
}
