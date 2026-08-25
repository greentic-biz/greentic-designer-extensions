//! Execution limits applied to every wasmtime `Store`.
//!
//! An extension is trusted only as far as the load gate goes, and TOFU means
//! "signed" is "signed by the first key we ever saw for this id". Nothing about
//! that stops a loaded extension from exporting a tool whose body is
//! `loop { v.push(0) }`. Without a limit, `func.call(...)` never returns and
//! cannot be interrupted; the designer runs these under `spawn_blocking`, so a
//! handful of such calls exhausts the blocking pool and the process stops
//! serving. Linear memory was likewise bounded only by the wasm32 4 GiB ceiling.
//!
//! Two mechanisms, because they catch different things:
//!
//! - **Epoch interruption** bounds *wall-clock* time. A ticker thread bumps the
//!   engine's epoch; wasm checks it at loop backedges and function entries and
//!   traps once the store's deadline passes. Time spent inside a host call
//!   counts, which is the intent — the budget is on the whole dispatch.
//! - **`StoreLimits`** bounds *memory and table growth*, which epochs cannot see
//!   at all: a guest can exhaust host memory without ever failing to make
//!   progress.
//!
//! Fuel would be the third option and is deliberately not used: it meters
//! instructions rather than time, costs roughly 2x throughput, and gives a
//! budget nobody can reason about from the outside.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wasmtime::{Engine, Store, StoreLimits, StoreLimitsBuilder};

/// Wall-clock budget for a single dispatch, when the host does not set one.
///
/// Generous on purpose. `deploy` is documented as taking up to ~2 minutes for
/// network-bound extensions and `render_bundle` is not fast either, so the
/// default has to clear a legitimate slow call by a wide margin. It is not
/// trying to be a tight SLA — it is the difference between a wedged worker
/// thread and a trapped call.
pub const DEFAULT_DISPATCH_TIMEOUT: Duration = Duration::from_mins(5);

/// How often the ticker bumps the engine epoch. Also the granularity of the
/// deadline: a 1 s tick means a 300 s budget expires somewhere in [300, 301).
const EPOCH_TICK: Duration = Duration::from_secs(1);

/// Linear memory a single extension instance may grow to.
///
/// Well above any real design extension (schemas, card JSON, a bundle being
/// assembled) and well below what threatens the host.
const MAX_MEMORY_BYTES: usize = 512 * 1024 * 1024;

/// Table entries a single instance may allocate. Component-model glue uses a
/// handful; anything near this is a runaway.
const MAX_TABLE_ELEMENTS: usize = 100_000;

/// Concurrent instances, memories, and tables within one store. Each dispatch
/// instantiates exactly one component, so these are runaway guards, not budgets.
const MAX_INSTANCES: usize = 64;
const MAX_MEMORIES: usize = 64;
const MAX_TABLES: usize = 64;

/// Background ticker that advances an [`Engine`]'s epoch.
///
/// Held by the runtime for its whole life; dropping it stops the thread. One
/// ticker per engine is enough — every store built from that engine reads the
/// same counter.
pub struct EpochTicker {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl EpochTicker {
    /// Spawn the ticker for `engine`.
    ///
    /// Holds a `Weak`, not a clone: an `Engine` clone would keep the engine —
    /// and its compiled modules — alive for as long as the thread, which is
    /// exactly the leak a "just keep it simple" version introduces.
    pub fn spawn(engine: &Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let weak = engine.weak();
        let flag = stop.clone();
        let spawned = std::thread::Builder::new()
            .name("greentic-ext-epoch".to_string())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    // The engine outliving this thread is the normal case; the
                    // reverse means the runtime is gone and so is the reason to
                    // keep ticking.
                    let Some(engine) = weak.upgrade() else {
                        return;
                    };
                    engine.increment_epoch();
                }
            });

        // Without this thread the epoch never advances, so every dispatch
        // deadline is unreachable and the runaway-guest guard is off — the one
        // failure here that must not pass in silence, because everything
        // downstream keeps working exactly as if it were armed.
        let join = match spawned {
            Ok(handle) => Some(handle),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "could not start the epoch ticker; extension dispatch deadlines will NOT fire"
                );
                None
            }
        };
        Self { stop, join }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // The thread wakes at most `EPOCH_TICK` from now and exits. Detach
        // rather than join: a `Drop` that blocks up to a second on a background
        // ticker is a worse trade than letting it finish on its own.
        drop(self.join.take());
    }
}

/// Apply the memory/table ceilings and the wall-clock deadline to `store`.
///
/// `timeout` of `None` leaves the store without a deadline — for a host that
/// has its own supervision and wants the call to run to completion. The memory
/// ceilings apply either way; there is no reason to want those unbounded.
pub fn apply(store: &mut Store<crate::host_state::HostState>, timeout: Option<Duration>) {
    store.limiter(|state| &mut state.limits);

    if let Some(timeout) = timeout {
        store.set_epoch_deadline(deadline_ticks(timeout));
    }
}

/// How many epoch ticks `timeout` is worth, as at least one.
///
/// Integer nanoseconds throughout: a float round-trip would need a lossy cast
/// back to `u64` at the end, and rounding *down* there is the one direction
/// that matters — a deadline of zero traps the call immediately. Rounds up, so
/// a sub-tick timeout still gets one whole tick, and saturates rather than
/// wrapping if a host configures an absurd duration.
fn deadline_ticks(timeout: Duration) -> u64 {
    let per_tick = EPOCH_TICK.as_nanos().max(1);
    let ticks = timeout.as_nanos().div_ceil(per_tick).max(1);
    u64::try_from(ticks).unwrap_or(u64::MAX)
}

/// The per-store resource ceilings, built once per `HostState`.
#[must_use]
pub fn store_limits() -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(MAX_MEMORY_BYTES)
        .table_elements(MAX_TABLE_ELEMENTS)
        .instances(MAX_INSTANCES)
        .memories(MAX_MEMORIES)
        .tables(MAX_TABLES)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sub_tick_timeout_still_gets_a_whole_tick() {
        // Rounding down here would produce a deadline of zero, which traps the
        // call before it runs a single instruction.
        assert_eq!(deadline_ticks(Duration::from_millis(1)), 1);
        assert_eq!(deadline_ticks(Duration::ZERO), 1);
    }

    #[test]
    fn a_whole_number_of_ticks_is_not_rounded_up() {
        assert_eq!(deadline_ticks(EPOCH_TICK), 1);
        assert_eq!(deadline_ticks(EPOCH_TICK * 5), 5);
    }

    #[test]
    fn a_partial_tick_rounds_up() {
        assert_eq!(deadline_ticks(EPOCH_TICK * 5 + Duration::from_nanos(1)), 6);
    }

    #[test]
    fn the_default_budget_clears_a_slow_deploy_by_a_wide_margin() {
        // `deploy` is documented as taking up to ~2 minutes for network-bound
        // extensions; the default has to be comfortably above that or it turns
        // a slow success into a trap.
        assert!(DEFAULT_DISPATCH_TIMEOUT >= Duration::from_mins(4));
    }

    #[test]
    fn an_absurd_timeout_saturates_instead_of_wrapping() {
        assert_eq!(deadline_ticks(Duration::MAX), u64::MAX);
    }
}
