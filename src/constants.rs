use std::time::Duration;

pub(crate) const MPSC_CHANNEL_SIZE: usize = 1;
pub(crate) const TICK_TIME: Duration = Duration::from_secs(1);
pub(crate) const PROPAGATION_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(2);
