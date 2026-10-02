//! Picking which monitor(s) a command applies to.

use anyhow::{Result, bail};

use crate::backend::Monitor;

/// A monitor together with its 1-based number from `dellctl list`.
pub struct Indexed {
    pub index: usize,
    pub monitor: Box<dyn Monitor>,
}

pub fn index(monitors: Vec<Box<dyn Monitor>>) -> Vec<Indexed> {
    monitors
        .into_iter()
        .enumerate()
        .map(|(i, monitor)| Indexed {
            index: i + 1,
            monitor,
        })
        .collect()
}

/// Choose exactly one monitor: the one `--monitor` names, the only monitor
/// connected, or the only Dell. Anything ambiguous is an error that lists
/// the candidates rather than a guess.
pub fn one(monitors: Vec<Box<dyn Monitor>>, wanted: Option<&str>) -> Result<Indexed> {
    let mut candidates = index(monitors);
    if candidates.is_empty() {
        bail!(no_monitors_message());
    }

    if let Some(wanted) = wanted {
        return by_selector(candidates, wanted);
    }
    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }
    let dells: Vec<usize> = candidates
        .iter()
        .filter(|c| c.monitor.info().is_dell())
        .map(|c| c.index)
        .collect();
    if let [only] = dells[..] {
        return Ok(candidates.remove(only - 1));
    }
    bail!(
        "several monitors are connected; choose one with --monitor:\n{}",
        listing(&candidates)
    )
}

/// For `status`: every monitor when none was chosen, otherwise the chosen one.
pub fn all_or_one(monitors: Vec<Box<dyn Monitor>>, wanted: Option<&str>) -> Result<Vec<Indexed>> {
    if wanted.is_some() {
        return Ok(vec![one(monitors, wanted)?]);
    }
    let all = index(monitors);
    if all.is_empty() {
        bail!(no_monitors_message());
    }
    Ok(all)
}

fn by_selector(mut candidates: Vec<Indexed>, wanted: &str) -> Result<Indexed> {
    if let Ok(n) = wanted.trim().parse::<usize>() {
        if n == 0 || n > candidates.len() {
            bail!(
                "there's no monitor {n}; connected monitors:\n{}",
                listing(&candidates)
            );
        }
        return Ok(candidates.remove(n - 1));
    }

    let needle = wanted.to_ascii_lowercase();
    let matches: Vec<usize> = candidates
        .iter()
        .filter(|c| c.monitor.info().name.to_ascii_lowercase().contains(&needle))
        .map(|c| c.index)
        .collect();
    match matches[..] {
        [only] => Ok(candidates.remove(only - 1)),
        [] => bail!(
            "no monitor matches '{wanted}'; connected monitors:\n{}",
            listing(&candidates)
        ),
        _ => bail!(
            "'{wanted}' matches several monitors; choose one by number:\n{}",
            listing(&candidates)
        ),
    }
}

fn listing(candidates: &[Indexed]) -> String {
    candidates
        .iter()
        .map(|c| format!("  {}  {}", c.index, c.monitor.info().name))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What to check when a monitor can't be reached over DDC/CI at all.
pub const DDC_HINT: &str = "make sure DDC/CI is enabled in the monitor's on-screen menu (Others → DDC/CI); \
     some docks, adapters, and KVM switches block DDC/CI, so try a direct cable";

fn no_monitors_message() -> String {
    // On Linux, a missing i2c-dev module or I²C permissions are diagnosed
    // separately by the backend, so by this point the devices look fine.
    let extra = if cfg!(target_os = "windows") {
        ", and that your graphics driver is up to date"
    } else {
        ""
    };
    format!("no DDC/CI-capable monitors found\nhint: {DDC_HINT}{extra}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::fake::{FakeBackend, FakeMonitor};
    use crate::backend::{Backend, MonitorInfo};

    fn monitors(list: Vec<FakeMonitor>) -> Vec<Box<dyn Monitor>> {
        FakeBackend(list).enumerate().unwrap()
    }

    fn lg() -> FakeMonitor {
        FakeMonitor::named("LG 27UK850")
    }

    fn picked(result: Result<Indexed>) -> (usize, String) {
        let m = result.unwrap();
        (m.index, m.monitor.info().name.clone())
    }

    #[test]
    fn single_monitor_is_picked() {
        assert_eq!(
            picked(one(monitors(vec![lg()]), None)),
            (1, "LG 27UK850".into())
        );
    }

    #[test]
    fn only_dell_is_picked_among_several() {
        let got = picked(one(monitors(vec![lg(), FakeMonitor::u3425we()]), None));
        assert_eq!(got, (2, "DELL U3425WE".into()));
    }

    #[test]
    fn two_dells_need_a_choice() {
        let err = one(
            monitors(vec![
                FakeMonitor::u3425we(),
                FakeMonitor::named("DELL U2723QE"),
            ]),
            None,
        )
        .err()
        .unwrap()
        .to_string();
        assert!(err.contains("--monitor"), "{err}");
        assert!(err.contains("2  DELL U2723QE"), "{err}");
    }

    #[test]
    fn selects_by_index_and_text() {
        let list = || monitors(vec![lg(), FakeMonitor::u3425we()]);
        assert_eq!(picked(one(list(), Some("1"))).0, 1);
        assert_eq!(picked(one(list(), Some("u3425"))).0, 2);
    }

    #[test]
    fn bad_selectors_explain_themselves() {
        let list = || monitors(vec![lg(), FakeMonitor::u3425we()]);
        assert!(
            one(list(), Some("3"))
                .err()
                .unwrap()
                .to_string()
                .starts_with("there's no monitor 3")
        );
        assert!(one(list(), Some("0")).is_err());
        assert!(
            one(list(), Some("samsung"))
                .err()
                .unwrap()
                .to_string()
                .contains("no monitor matches")
        );
        let both = monitors(vec![FakeMonitor::u3425we(), FakeMonitor::u3425we()]);
        assert!(
            one(both, Some("dell"))
                .err()
                .unwrap()
                .to_string()
                .contains("matches several")
        );
    }

    #[test]
    fn manufacturer_code_counts_as_dell() {
        let mut generic = FakeMonitor::named("Generic PnP Monitor");
        generic.info = MonitorInfo {
            manufacturer: Some("DEL".into()),
            ..generic.info
        };
        assert_eq!(picked(one(monitors(vec![lg(), generic]), None)).0, 2);
    }

    #[test]
    fn no_monitors_gives_a_hint() {
        let err = one(vec![], None).err().unwrap().to_string();
        assert!(err.contains("hint:"), "{err}");
        assert!(all_or_one(vec![], None).is_err());
    }

    #[test]
    fn status_takes_all_when_unselected() {
        assert_eq!(
            all_or_one(monitors(vec![lg(), FakeMonitor::u3425we()]), None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            all_or_one(monitors(vec![lg(), FakeMonitor::u3425we()]), Some("lg"))
                .unwrap()
                .len(),
            1
        );
    }
}
