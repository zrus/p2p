mod behaviour;
mod command;
mod event;
mod interface;
mod peer;
mod system;
mod utils;

use ::std::sync::Arc;

use ::anyhow::anyhow;
use ::dashmap::DashMap;
use ::log::*;
use ::once_cell::sync::Lazy;
use ::tiny_tokio_actor::{ActorPath, ActorRef, ActorSystem, EventBus};

pub use command::Command;
use event::SysEvent;
use interface::IActor;
pub use peer::Peer;
use system::GlobalSystem;

pub(crate) static RUNTIME: Lazy<
  ::os_thread_local::ThreadLocal<::tokio::runtime::Runtime>,
> = Lazy::new(|| {
  ::os_thread_local::ThreadLocal::new(|| {
    ::tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .unwrap()
  })
});
static SYSTEM: Lazy<GlobalSystem> = Lazy::new(|| {
  let event_bus = EventBus::<event::SysEvent>::new(1024);
  let actor_sys = ActorSystem::new("p2p-chat", event_bus);
  GlobalSystem::init(actor_sys)
});
static PATHS: Lazy<Arc<DashMap<String, ActorPath>>> =
  Lazy::new(|| Default::default());

pub fn init() {
  _ = &SYSTEM;
}

pub fn run<A: IActor>(actor: A) -> utils::R {
  let actor_ref = RUNTIME.with(|inner| {
    inner.block_on(SYSTEM.actor_sys().create_actor(&A::named(), actor))
  })?;
  PATHS.insert(A::named(), actor_ref.path().clone());
  Ok(())
}

pub fn actor<A: IActor>() -> utils::R<ActorRef<SysEvent, A>> {
  debug!("Getting actor name: {}", A::named());
  let actor_path = PATHS
    .get(&A::named())
    .ok_or(anyhow!("Actor has not been added to memory yet"))?
    .value()
    .clone();
  RUNTIME
    .with(|inner| inner.block_on(SYSTEM.actor_sys().get_actor(&actor_path)))
    .ok_or(anyhow!("Actor has not been initialized yet"))
}

pub fn block_until_stopped() {
  SYSTEM.wait_until_stopped();
}

pub fn stop() {
  SYSTEM.notify_stopped()
}
