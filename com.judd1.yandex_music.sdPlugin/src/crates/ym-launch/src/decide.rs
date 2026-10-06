use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const GRACE_SECS: u64 = 10;
pub const COOLDOWN: Duration = Duration::from_secs(60);
pub const KICK_SPACING: Duration = Duration::from_secs(5);
pub const MAX_FAILURES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainProc {
    pub pid: u32,
    pub exe: PathBuf,
    pub age_secs: u64,
    pub cmd_unreadable: bool,
}

#[derive(Debug, Clone)]
pub struct DecideInput {
    pub enabled: bool,
    pub connected: bool,
    pub any_local: bool,
    pub kick: bool,
    pub main_proc: Option<MainProc>,
    pub ambiguous: bool,
    pub cooldown_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Nothing,
    Restart { pid: u32, exe: PathBuf },
    Launch,
    HintElevated,
}

pub fn decide(i: &DecideInput) -> Decision {
    if !i.enabled || i.connected || (!i.any_local && !i.kick) {
        return Decision::Nothing;
    }
    if i.ambiguous {
        return Decision::Nothing;
    }
    match &i.main_proc {
        Some(m) if m.cmd_unreadable => {
            if m.age_secs < GRACE_SECS {
                Decision::Nothing
            } else {
                Decision::HintElevated
            }
        }
        Some(m) => {
            if m.age_secs < GRACE_SECS || !i.cooldown_ok {
                Decision::Nothing
            } else {
                Decision::Restart { pid: m.pid, exe: m.exe.clone() }
            }
        }
        None if i.kick && i.cooldown_ok => Decision::Launch,
        None => Decision::Nothing,
    }
}

#[derive(Debug, Default)]
pub struct Backoff {
    last_attempt: Option<Instant>,
    failures: u32,
}

impl Backoff {
    pub fn cooldown_ok(&self, kick: bool, now: Instant) -> bool {
        if kick { self.kick_ok(now) } else { self.auto_ok(now) }
    }
    pub fn auto_ok(&self, now: Instant) -> bool {
        self.failures < MAX_FAILURES
            && self.last_attempt.is_none_or(|t| now.duration_since(t) >= COOLDOWN)
    }
    pub fn kick_ok(&self, now: Instant) -> bool {
        self.last_attempt.is_none_or(|t| now.duration_since(t) >= KICK_SPACING)
    }
    pub fn note_kick(&mut self) {
        self.failures = 0;
    }
    pub fn note_attempt(&mut self, now: Instant) {
        self.last_attempt = Some(now);
    }
    pub fn note_result(&mut self, ok: bool) {
        if ok {
            self.failures = 0;
        } else {
            self.failures += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> DecideInput {
        DecideInput {
            enabled: true,
            connected: false,
            any_local: true,
            kick: false,
            main_proc: None,
            ambiguous: false,
            cooldown_ok: true,
        }
    }

    fn main_proc(age_secs: u64) -> MainProc {
        MainProc {
            pid: 42,
            exe: PathBuf::from("/Applications/Яндекс Музыка.app/Contents/MacOS/Яндекс Музыка"),
            age_secs,
            cmd_unreadable: false,
        }
    }

    fn elevated_proc(age_secs: u64) -> MainProc {
        MainProc { cmd_unreadable: true, ..main_proc(age_secs) }
    }

    #[test]
    fn disabled_connected_or_idle_do_nothing() {
        let mut i = base();
        i.enabled = false;
        i.main_proc = Some(main_proc(999));
        assert_eq!(decide(&i), Decision::Nothing);

        let mut i = base();
        i.connected = true;
        i.main_proc = Some(main_proc(999));
        assert_eq!(decide(&i), Decision::Nothing);

        let mut i = base();
        i.any_local = false;
        i.main_proc = Some(main_proc(999));
        assert_eq!(decide(&i), Decision::Nothing);
    }

    #[test]
    fn kick_alone_opens_the_gate() {
        let mut i = base();
        i.any_local = false;
        i.kick = true;
        i.main_proc = Some(main_proc(999));
        assert!(matches!(decide(&i), Decision::Restart { pid: 42, .. }));
    }

    #[test]
    fn ambiguous_is_passive() {
        let mut i = base();
        i.ambiguous = true;
        assert_eq!(decide(&i), Decision::Nothing);
    }

    #[test]
    fn running_client_without_bridge_is_restarted() {
        let mut i = base();
        i.main_proc = Some(main_proc(999));
        assert_eq!(decide(&i), Decision::Restart { pid: 42, exe: main_proc(0).exe });
    }

    #[test]
    fn young_process_grace_then_restart() {
        let mut i = base();
        i.main_proc = Some(main_proc(GRACE_SECS - 1));
        assert_eq!(decide(&i), Decision::Nothing);
        i.main_proc = Some(main_proc(GRACE_SECS));
        assert!(matches!(decide(&i), Decision::Restart { pid: 42, .. }));
    }

    #[test]
    fn elevated_main_hints_instead_of_restart() {
        let mut i = base();
        i.main_proc = Some(elevated_proc(999));
        assert_eq!(decide(&i), Decision::HintElevated);
        i.cooldown_ok = false;
        assert_eq!(decide(&i), Decision::HintElevated, "hint не зависит от кулдауна перезапусков");
    }

    #[test]
    fn elevated_main_respects_startup_grace() {
        let mut i = base();
        i.main_proc = Some(elevated_proc(GRACE_SECS - 1));
        assert_eq!(decide(&i), Decision::Nothing);
        i.main_proc = Some(elevated_proc(GRACE_SECS));
        assert_eq!(decide(&i), Decision::HintElevated);
    }

    #[test]
    fn restart_respects_cooldown() {
        let mut i = base();
        i.main_proc = Some(main_proc(999));
        i.cooldown_ok = false;
        assert_eq!(decide(&i), Decision::Nothing);
    }

    #[test]
    fn launch_only_on_kick() {
        let mut i = base();
        assert_eq!(decide(&i), Decision::Nothing);
        i.kick = true;
        assert_eq!(decide(&i), Decision::Launch);
        i.cooldown_ok = false;
        assert_eq!(decide(&i), Decision::Nothing);
    }

    #[test]
    fn backoff_limits_and_resets() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        assert!(b.auto_ok(t0));
        assert!(b.kick_ok(t0));

        b.note_attempt(t0);
        assert!(!b.auto_ok(t0 + Duration::from_secs(59)));
        assert!(b.auto_ok(t0 + COOLDOWN));
        assert!(!b.kick_ok(t0 + Duration::from_secs(4)));
        assert!(b.kick_ok(t0 + KICK_SPACING));

        for _ in 0..MAX_FAILURES {
            b.note_result(false);
        }
        assert!(!b.auto_ok(t0 + Duration::from_secs(600)));
        assert!(b.kick_ok(t0 + Duration::from_secs(600)));

        b.note_kick();
        assert!(b.auto_ok(t0 + Duration::from_secs(600)));

        b.note_result(false);
        b.note_result(true);
        assert!(b.auto_ok(t0 + Duration::from_secs(1200)));
    }
}
