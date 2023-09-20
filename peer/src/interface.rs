use crate::event::SysEvent;

pub trait IActor: ::tiny_tokio_actor::Actor<SysEvent> {
  fn named() -> String {
    String::new()
  }
}
