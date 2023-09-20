#[derive(Debug, Clone)]
pub struct SysEvent();

impl ::tiny_tokio_actor::SystemEvent for SysEvent {}
