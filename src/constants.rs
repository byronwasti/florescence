use std::time::Duration;

#[allow(unused)]
pub(crate) const MPSC_CHANNEL_SIZE: usize = 1;
pub(crate) const TICK_TIME: Duration = Duration::from_secs(1);
pub(crate) const PROPAGATION_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(2);
