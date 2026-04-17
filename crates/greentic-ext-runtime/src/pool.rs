use std::collections::VecDeque;
use std::sync::Mutex;

use wasmtime::Store;
use wasmtime::component::Instance;

pub struct InstancePool {
    capacity: usize,
    free: Mutex<VecDeque<PooledInstance>>,
}

pub struct PooledInstance {
    pub instance: Instance,
    pub store: Store<()>,
}

impl InstancePool {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            free: Mutex::new(VecDeque::new()),
        }
    }

    pub fn acquire<F>(&self, make: F) -> anyhow::Result<PooledInstance>
    where
        F: FnOnce() -> anyhow::Result<PooledInstance>,
    {
        if let Some(inst) = self.free.lock().unwrap().pop_front() {
            return Ok(inst);
        }
        make()
    }

    pub fn release(&self, inst: PooledInstance) {
        let mut q = self.free.lock().unwrap();
        if q.len() < self.capacity {
            q.push_back(inst);
        }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}
