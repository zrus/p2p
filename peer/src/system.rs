use ::std::sync::{Condvar, Mutex};

use tiny_tokio_actor::ActorSystem;

use crate::event::SysEvent;

pub(crate) struct GlobalSystem {
  running: Mutex<bool>,
  stopping_cvar: Condvar,
  actor_sys: ActorSystem<SysEvent>,
}

impl GlobalSystem {
  pub fn init(actor_sys: ActorSystem<SysEvent>) -> Self {
    let running = Mutex::new(true);
    let stopping_cvar = Condvar::new();

    Self {
      running,
      stopping_cvar,
      actor_sys,
    }
  }

  pub fn actor_sys(&self) -> &ActorSystem<SysEvent> {
    &self.actor_sys
  }

  pub fn notify_stopped(&self) {
    let running = self.running.try_lock();
    if let Ok(mut lock) = running {
      *lock = false;
    }
    self.stopping_cvar.notify_all();
  }

  pub fn wait_until_stopped(&self) {
    let running = self.running.try_lock();
    if let Ok(mut lock) = running {
      while *lock {
        lock = self.stopping_cvar.wait(lock).unwrap();
      }
    }
  }
}
