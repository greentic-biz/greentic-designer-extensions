//! Finite state machine for the dev loop.

/// High-level states the dev loop traverses per change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle { last_build_ok: bool },
    Debouncing,
    Building,
    Packing,
    Installing,
    Error,
}

/// Input events fed into the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    FsChange,
    DebounceElapsed,
    BuildOk,
    BuildFailed,
    PackOk,
    PackFailed,
    InstallOk,
    InstallFailed,
    Shutdown,
}

impl State {
    pub fn initial() -> Self {
        State::Idle { last_build_ok: true }
    }

    /// Apply an input and return the next state. Unexpected transitions keep
    /// the current state (a no-op), so callers can safely replay events.
    #[must_use]
    pub fn next(self, input: Input) -> State {
        use Input::*;
        use State::*;
        match (self, input) {
            (_, Shutdown) => Error,
            (Idle { .. }, FsChange) | (Debouncing, FsChange) => Debouncing,
            (Debouncing, DebounceElapsed) => Building,
            (Building, BuildOk) => Packing,
            (Building, BuildFailed) => Idle { last_build_ok: false },
            (Packing, PackOk) => Installing,
            (Packing, PackFailed) => Idle { last_build_ok: false },
            (Installing, InstallOk) => Idle { last_build_ok: true },
            (Installing, InstallFailed) => Idle { last_build_ok: false },
            (state, _) => state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Input::*, State::*, *};

    #[test]
    fn happy_path_cycles_back_to_idle() {
        let s = State::initial();
        let s = s.next(FsChange);
        assert!(matches!(s, Debouncing));
        let s = s.next(DebounceElapsed);
        assert!(matches!(s, Building));
        let s = s.next(BuildOk);
        assert!(matches!(s, Packing));
        let s = s.next(PackOk);
        assert!(matches!(s, Installing));
        let s = s.next(InstallOk);
        assert!(matches!(s, Idle { last_build_ok: true }));
    }

    #[test]
    fn build_failure_returns_to_idle_with_flag_set() {
        let s = State::initial().next(FsChange).next(DebounceElapsed);
        assert!(matches!(s, Building));
        let s = s.next(BuildFailed);
        assert!(matches!(s, Idle { last_build_ok: false }));
    }

    #[test]
    fn install_failure_returns_to_idle_failed() {
        let s = State::initial()
            .next(FsChange)
            .next(DebounceElapsed)
            .next(BuildOk)
            .next(PackOk);
        assert!(matches!(s, Installing));
        let s = s.next(InstallFailed);
        assert!(matches!(s, Idle { last_build_ok: false }));
    }

    #[test]
    fn fs_change_during_debounce_stays_debouncing() {
        let s = State::initial().next(FsChange);
        let s = s.next(FsChange);
        assert!(matches!(s, Debouncing));
    }

    #[test]
    fn unrelated_input_is_noop() {
        let s = State::initial();
        let s2 = s.next(BuildOk);
        assert_eq!(s, s2);
    }

    #[test]
    fn shutdown_always_transitions_to_error() {
        for start in [
            State::initial(),
            Debouncing,
            Building,
            Packing,
            Installing,
        ] {
            assert!(matches!(start.next(Shutdown), Error));
        }
    }
}
