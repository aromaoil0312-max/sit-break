//! Pure, deterministic timer. `now` is awake monotonic milliseconds, never wall time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Work,
    Break,
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub mode: Mode,
    pub paused: bool,
    pub blocked: bool,
    remaining_ms: u64,
    last_ms: u64,
    work_ms: u64,
    break_ms: u64,
    emergency_ms: Option<u64>,
}

impl Timer {
    pub fn new(now: u64, work_ms: u64, break_ms: u64) -> Self {
        assert!(work_ms > 0 && break_ms > 0);
        Self {
            mode: Mode::Work,
            paused: false,
            blocked: false,
            remaining_ms: work_ms,
            last_ms: now,
            work_ms,
            break_ms,
            emergency_ms: None,
        }
    }

    pub fn remaining(&self) -> u64 {
        self.remaining_ms.div_ceil(1000)
    }
    pub fn emergency_remaining(&self) -> Option<u64> {
        self.emergency_ms.map(|v| v.div_ceil(1000))
    }

    pub fn advance(&mut self, now: u64) {
        let elapsed = now.saturating_sub(self.last_ms);
        self.last_ms = now;
        if self.blocked || (self.mode == Mode::Work && self.paused) {
            return;
        }
        if let Some(ms) = self.emergency_ms.as_mut() {
            *ms = ms.saturating_sub(elapsed);
        }
        if elapsed < self.remaining_ms {
            self.remaining_ms -= elapsed;
            return;
        }
        // Never credit a break that was not actually presented. A delayed scheduler
        // starts a full break now instead of fast-forwarding through invisible cycles.
        self.mode = match self.mode {
            Mode::Work => Mode::Break,
            Mode::Break => Mode::Work,
        };
        self.remaining_ms = if self.mode == Mode::Work {
            self.work_ms
        } else {
            self.break_ms
        };
        self.paused = false;
        self.emergency_ms = None;
    }

    pub fn set_blocked(&mut self, now: u64, blocked: bool) {
        self.advance(now);
        self.blocked = blocked;
        if blocked {
            self.emergency_ms = None;
        }
    }

    pub fn toggle_pause(&mut self, now: u64) -> Result<(), &'static str> {
        self.advance(now);
        self.require_work()?;
        self.paused = !self.paused;
        Ok(())
    }

    pub fn reset(&mut self, now: u64) -> Result<(), &'static str> {
        self.advance(now);
        self.require_work()?;
        self.restart_work(now);
        Ok(())
    }

    pub fn require_work(&self) -> Result<(), &'static str> {
        if self.mode == Mode::Break {
            Err("休憩中はこの操作を利用できません。")
        } else {
            Ok(())
        }
    }

    pub fn configure(&mut self, now: u64, work_ms: u64, break_ms: u64) -> Result<(), &'static str> {
        self.advance(now);
        self.require_work()?;
        let changed = self.work_ms != work_ms;
        self.work_ms = work_ms;
        self.break_ms = break_ms;
        if changed {
            self.remaining_ms = work_ms;
        }
        Ok(())
    }

    fn restart_work(&mut self, now: u64) {
        self.mode = Mode::Work;
        self.paused = false;
        self.remaining_ms = self.work_ms;
        self.last_ms = now;
        self.emergency_ms = None;
    }

    pub fn request_emergency(&mut self, now: u64) -> Result<(), &'static str> {
        self.advance(now);
        if self.mode != Mode::Break || self.blocked {
            return Err("休憩画面で操作してください。");
        }
        // Repeated requests never shorten the original waiting period.
        self.emergency_ms.get_or_insert(30_000);
        Ok(())
    }
    pub fn cancel_emergency(&mut self) {
        self.emergency_ms = None;
    }
    pub fn confirm_emergency(&mut self, now: u64) -> Result<(), &'static str> {
        self.advance(now);
        if self.mode != Mode::Break || self.blocked || self.emergency_ms != Some(0) {
            return Err("緊急解除には30秒の待機と再確認が必要です。");
        }
        self.restart_work(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_defaults_cycle() {
        let mut t = Timer::new(0, 120 * 60_000, 5 * 60_000);
        assert_eq!(t.remaining(), 7200);
        t.advance(7_200_000);
        assert_eq!(t.mode, Mode::Break);
        assert_eq!(t.remaining(), 300);
        t.advance(7_500_000);
        assert_eq!(t.mode, Mode::Work);
        assert_eq!(t.remaining(), 7200);
    }
    #[test]
    fn one_minute_cycle_is_automatic() {
        let mut t = Timer::new(0, 60_000, 60_000);
        t.advance(59_999);
        assert_eq!(t.mode, Mode::Work);
        assert_eq!(t.remaining(), 1);
        t.advance(60_000);
        assert_eq!(t.mode, Mode::Break);
        t.advance(120_000);
        assert_eq!(t.mode, Mode::Work);
        assert_eq!(t.remaining(), 60);
    }
    #[test]
    fn pause_resume_preserves_fractional_time() {
        let mut t = Timer::new(0, 60_000, 30_000);
        t.toggle_pause(15_125).unwrap();
        t.advance(999_999);
        assert_eq!(t.remaining(), 45);
        t.toggle_pause(1_000_000).unwrap();
        t.advance(1_044_874);
        assert_eq!(t.mode, Mode::Work);
        t.advance(1_044_875);
        assert_eq!(t.mode, Mode::Break);
    }
    #[test]
    fn eighty_minutes_then_one_hour_locked_leaves_forty() {
        let mut t = Timer::new(0, 120 * 60_000, 300_000);
        t.set_blocked(80 * 60_000, true);
        t.advance(140 * 60_000);
        t.set_blocked(140 * 60_000, false);
        assert_eq!(t.remaining(), 40 * 60);
        t.advance(141 * 60_000);
        assert_eq!(t.remaining(), 39 * 60);
    }
    #[test]
    fn sleep_does_not_advance_awake_clock() {
        let mut t = Timer::new(0, 120 * 60_000, 300_000);
        t.advance(80 * 60_000);
        t.advance(80 * 60_000);
        assert_eq!(t.remaining(), 40 * 60);
    }
    #[test]
    fn break_has_no_pause_reset_or_settings_bypass() {
        let mut t = Timer::new(0, 60_000, 300_000);
        t.advance(60_000);
        assert!(t.toggle_pause(61_000).is_err());
        assert!(t.reset(61_000).is_err());
        assert!(t.configure(61_000, 1000, 1000).is_err());
        assert!(t.confirm_emergency(61_000).is_err());
        assert_eq!(t.mode, Mode::Break);
    }
    #[test]
    fn emergency_requires_thirty_seconds_and_confirmation() {
        let mut t = Timer::new(0, 60_000, 300_000);
        t.advance(60_000);
        t.request_emergency(65_000).unwrap();
        t.request_emergency(70_000).unwrap();
        assert!(t.confirm_emergency(94_999).is_err());
        t.advance(95_000);
        assert_eq!(t.mode, Mode::Break);
        t.confirm_emergency(95_000).unwrap();
        assert_eq!(t.mode, Mode::Work);
        assert_eq!(t.remaining(), 60);
    }
    #[test]
    fn lock_cancels_emergency_and_preserves_break() {
        let mut t = Timer::new(0, 60_000, 300_000);
        t.advance(60_000);
        t.request_emergency(65_000).unwrap();
        t.set_blocked(80_000, true);
        t.set_blocked(3_680_000, false);
        assert_eq!(t.remaining(), 280);
        assert!(t.confirm_emergency(3_680_000).is_err());
    }
    #[test]
    fn delayed_worker_never_skips_an_unseen_break() {
        let mut t = Timer::new(0, 60_000, 30_000);
        t.advance(1_000_000);
        assert_eq!(t.mode, Mode::Break);
        assert_eq!(t.remaining(), 30);
    }
    #[test]
    fn irregular_ticks_do_not_accumulate_drift() {
        let mut t = Timer::new(0, 7_200_000, 300_000);
        for now in (0..7_200_000).step_by(137) {
            t.advance(now);
        }
        t.advance(7_199_999);
        assert_eq!(t.remaining(), 1);
        t.advance(7_200_000);
        assert_eq!(t.mode, Mode::Break);
    }
    #[test]
    fn long_running_cycles_and_cancel() {
        let mut t = Timer::new(0, 7_200_000, 300_000);
        for n in 0..1000 {
            let now = n * 7_500_000;
            t.advance(now + 7_200_000);
            t.request_emergency(now + 7_201_000).unwrap();
            t.cancel_emergency();
            assert!(t.confirm_emergency(now + 7_231_000).is_err());
            t.advance(now + 7_500_000);
            assert_eq!(t.remaining(), 7200);
        }
    }
    #[test]
    fn reset_resumes_but_sound_only_changes_do_not_reset() {
        let mut t = Timer::new(0, 60_000, 300_000);
        t.toggle_pause(10_000).unwrap();
        t.configure(20_000, 60_000, 300_000).unwrap();
        assert_eq!(t.remaining(), 50);
        t.reset(30_000).unwrap();
        assert!(!t.paused);
        assert_eq!(t.remaining(), 60);
    }
}
