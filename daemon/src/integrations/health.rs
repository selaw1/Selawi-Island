//! Opt-in system warnings: low memory and high CPU temperature.
//!
//! There is no signal for these, so this integration reads two small kernel
//! files every `INTERVAL`. That is why it is off by default. It only warns:
//! once when a limit is crossed, and again only after the value has
//! recovered past a margin (so a value hovering at the limit is quiet).
//!
//! Sources: `/proc/meminfo` (MemAvailable) and CPU sensors in
//! `/sys/class/hwmon` (coretemp, k10temp, zenpower, cpu_thermal), falling
//! back to `/sys/class/thermal` CPU zones. GPU readings vary by vendor and
//! are not used.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use selawi_core::events::{
    Category, EventId, EventInput, LifecycleState, Payload, Severity, Source, SystemPayload,
};

use super::Stop;
use crate::hub::{Hub, Status};

pub const ID: &str = "health";
pub const NAME: &str = "System warnings";
pub const INTERVAL: Duration = Duration::from_secs(15);
pub const MEMORY_LOW_PERCENT: f64 = 10.0;
const MEMORY_OK_PERCENT: f64 = 15.0;
pub const TEMPERATURE_HIGH_C: f64 = 90.0;
const TEMPERATURE_OK_C: f64 = 80.0;

const CPU_SENSORS: [&str; 5] = [
    "coretemp",
    "k10temp",
    "zenpower",
    "cpu_thermal",
    "cpu-thermal",
];
const CPU_ZONES: [&str; 3] = ["x86_pkg_temp", "cpu-thermal", "cpu_thermal"];

/// `(total_kb, available_kb)` from the text of /proc/meminfo.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| -> Option<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|rest| rest.trim_start_matches(':').split_whitespace().next())
            .and_then(|n| n.parse().ok())
    };
    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    (total > 0).then_some((total, available.min(total)))
}

pub fn read_memory(root: &Path) -> Option<(u64, u64)> {
    parse_meminfo(&fs::read_to_string(root.join("proc/meminfo")).ok()?)
}

fn read_millidegrees(path: &Path) -> Option<f64> {
    let raw: i64 = fs::read_to_string(path).ok()?.trim().parse().ok()?;
    let c = raw as f64 / 1000.0;
    // Ignore sensors reporting nonsense.
    (0.0..150.0).contains(&c).then_some(c)
}

/// Hottest CPU sensor in °C, or `None` if there is no CPU sensor.
pub fn read_cpu_temperature(root: &Path) -> Option<f64> {
    fn keep(hottest: &mut Option<f64>, t: f64) {
        *hottest = Some(hottest.map_or(t, |h| h.max(t)));
    }
    let mut hottest: Option<f64> = None;

    if let Ok(dirs) = fs::read_dir(root.join("sys/class/hwmon")) {
        for dir in dirs.flatten().map(|d| d.path()) {
            let name = fs::read_to_string(dir.join("name")).unwrap_or_default();
            if !CPU_SENSORS.contains(&name.trim()) {
                continue;
            }
            if let Ok(files) = fs::read_dir(&dir) {
                for file in files.flatten().map(|f| f.path()) {
                    let is_input = file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("temp") && n.ends_with("_input"));
                    if let Some(t) = is_input.then(|| read_millidegrees(&file)).flatten() {
                        keep(&mut hottest, t);
                    }
                }
            }
        }
    }
    if hottest.is_none() {
        if let Ok(zones) = fs::read_dir(root.join("sys/class/thermal")) {
            for zone in zones.flatten().map(|z| z.path()) {
                let kind = fs::read_to_string(zone.join("type")).unwrap_or_default();
                if CPU_ZONES.contains(&kind.trim()) {
                    if let Some(t) = read_millidegrees(&zone.join("temp")) {
                        keep(&mut hottest, t);
                    }
                }
            }
        }
    }
    hottest
}

#[derive(Debug, Default)]
pub struct Health {
    memory_warned: bool,
    temperature_warned: bool,
}

/// Warnings for one reading. Each warning is given once, and again only
/// after the value has recovered past its margin.
pub fn check(
    state: &mut Health,
    memory: Option<(u64, u64)>,
    temperature: Option<f64>,
) -> Vec<EventInput> {
    let mut out = Vec::new();
    if let Some((total, available)) = memory {
        let percent = available as f64 * 100.0 / total as f64;
        if percent < MEMORY_LOW_PERCENT && !state.memory_warned {
            state.memory_warned = true;
            let gb = available as f64 / (1024.0 * 1024.0);
            out.push(warning(
                "health:memory",
                "Memory low",
                &format!("{percent:.0}% free · {gb:.1} GB available"),
            ));
        } else if percent >= MEMORY_OK_PERCENT {
            state.memory_warned = false;
        }
    }
    if let Some(celsius) = temperature {
        if celsius >= TEMPERATURE_HIGH_C && !state.temperature_warned {
            state.temperature_warned = true;
            out.push(warning(
                "health:temperature",
                "High temperature",
                &format!("CPU at {celsius:.0} °C"),
            ));
        } else if celsius < TEMPERATURE_OK_C {
            state.temperature_warned = false;
        }
    }
    out
}

fn warning(id: &str, title: &str, detail: &str) -> EventInput {
    EventInput {
        id: EventId::new(id).unwrap_or_else(|_| unreachable!("constant ids are valid")),
        source: Source {
            app: "System".into(),
            integration: ID.into(),
            icon: None,
        },
        category: Category::System,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: Some(15_000),
        group: None,
        payload: Payload::System(SystemPayload {
            title: title.into(),
            detail: Some(detail.into()),
            severity: Severity::Warning,
        }),
        actions: Vec::new(),
    }
}

pub async fn run(hub: Arc<Hub>) -> Result<(), Stop> {
    watch(hub, Path::new("/")).await
}

/// Checks the files under `root` every `INTERVAL` (root is "/" in production).
pub async fn watch(hub: Arc<Hub>, root: &Path) -> Result<(), Stop> {
    let mut state = Health::default();
    let mut ticker = tokio::time::interval(INTERVAL);
    loop {
        ticker.tick().await;
        let memory = read_memory(root);
        let temperature = read_cpu_temperature(root);
        if memory.is_none() && temperature.is_none() {
            return Err(Stop::Unavailable(
                "no memory or CPU temperature readings".into(),
            ));
        }
        let mut seen = Vec::new();
        if memory.is_some() {
            seen.push("memory");
        }
        if temperature.is_some() {
            seen.push("CPU temperature");
        }
        let detail = format!(
            "checks {} every {} s",
            seen.join(" and "),
            INTERVAL.as_secs()
        );
        hub.set_status(ID, NAME, Status::Active(detail)).await;
        for input in check(&mut state, memory, temperature) {
            if let Err(e) = hub.submit(input).await {
                eprintln!("Health event rejected: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const MEMINFO: &str =
        "MemTotal:       16000000 kB\nMemFree:          500000 kB\nMemAvailable:    1200000 kB\n";

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("selawi-health-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
        let path = root.join(rel);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)
    }

    #[test]
    fn parses_meminfo() {
        assert_eq!(parse_meminfo(MEMINFO), Some((16_000_000, 1_200_000)));
        assert_eq!(
            parse_meminfo("MemTotal: 10 kB\n"),
            None,
            "needs MemAvailable"
        );
        assert_eq!(parse_meminfo("garbage"), None);
    }

    #[test]
    fn memory_warning_once_until_recovered() {
        let mut state = Health::default();
        let low = Some((100, 8));
        let titles = |v: Vec<EventInput>| v.len();
        assert_eq!(titles(check(&mut state, low, None)), 1);
        assert_eq!(
            titles(check(&mut state, Some((100, 9)), None)),
            0,
            "still low: quiet"
        );
        assert_eq!(
            titles(check(&mut state, Some((100, 12)), None)),
            0,
            "inside the margin: still quiet"
        );
        assert_eq!(
            titles(check(&mut state, Some((100, 20)), None)),
            0,
            "recovered"
        );
        assert_eq!(
            titles(check(&mut state, low, None)),
            1,
            "warns again after recovering"
        );
    }

    #[test]
    fn temperature_warning_with_margin() {
        let mut state = Health::default();
        let out = check(&mut state, None, Some(93.4));
        assert_eq!(out.len(), 1);
        assert!(out[0].validate().is_ok());
        let Payload::System(p) = &out[0].payload else {
            panic!("not system")
        };
        assert_eq!(p.detail.as_deref(), Some("CPU at 93 °C"));
        assert!(check(&mut state, None, Some(85.0)).is_empty());
        assert!(
            check(&mut state, None, Some(91.0)).is_empty(),
            "not recovered below 80 yet"
        );
        assert!(check(&mut state, None, Some(70.0)).is_empty());
        assert_eq!(check(&mut state, None, Some(95.0)).len(), 1);
    }

    #[test]
    fn reads_fake_proc_and_sys() -> std::io::Result<()> {
        let root = temp_root("files");
        write(&root, "proc/meminfo", MEMINFO)?;
        write(&root, "sys/class/hwmon/hwmon0/name", "nvme\n")?;
        write(&root, "sys/class/hwmon/hwmon0/temp1_input", "99000\n")?;
        write(&root, "sys/class/hwmon/hwmon1/name", "coretemp\n")?;
        write(&root, "sys/class/hwmon/hwmon1/temp1_input", "54000\n")?;
        write(&root, "sys/class/hwmon/hwmon1/temp2_input", "61500\n")?;
        write(&root, "sys/class/hwmon/hwmon1/temp2_label", "Core 0\n")?;
        assert_eq!(read_memory(&root), Some((16_000_000, 1_200_000)));
        assert_eq!(
            read_cpu_temperature(&root),
            Some(61.5),
            "only CPU sensors, the hottest one"
        );
        fs::remove_dir_all(&root)
    }

    #[test]
    fn falls_back_to_thermal_zones_and_ignores_nonsense() -> std::io::Result<()> {
        let root = temp_root("zones");
        write(&root, "sys/class/thermal/thermal_zone0/type", "acpitz\n")?;
        write(&root, "sys/class/thermal/thermal_zone0/temp", "27800\n")?;
        write(
            &root,
            "sys/class/thermal/thermal_zone1/type",
            "x86_pkg_temp\n",
        )?;
        write(&root, "sys/class/thermal/thermal_zone1/temp", "47000\n")?;
        assert_eq!(read_cpu_temperature(&root), Some(47.0));
        write(&root, "sys/class/thermal/thermal_zone1/temp", "-40000\n")?;
        assert_eq!(read_cpu_temperature(&root), None);
        assert_eq!(read_memory(&root), None);
        fs::remove_dir_all(&root)
    }
}
