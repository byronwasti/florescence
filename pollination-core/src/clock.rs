pub trait Clock {
    fn heartbeat() -> impl Future<Output = Option<()>>;
    fn recycle_ids() -> impl Future<Output = Option<()>>;
}
