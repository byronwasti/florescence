use petgraph::graph::NodeIndex;
use rand::Rng;
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Debug)]
pub struct Mailbox<Message> {
    inner: BinaryHeap<Mail<Message>>,
}

impl<Message> Mailbox<Message> {
    pub fn new() -> Mailbox<Message> {
        Self {
            inner: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, mail: Mail<Message>) {
        self.inner.push(mail);
    }

    /// Returns a tuple of (Mail, Delivery) to fascillitate history.
    pub fn get_delivery(&mut self) -> Option<Delivery<Message>> {
        let mail = self.inner.pop()?;
        Some(Delivery {
            delivered: false,
            mail,
        })
    }
}

pub struct Delivery<Message> {
    delivered: bool,
    mail: Mail<Message>,
}

impl<Message: Clone> Delivery<Message> {
    // TODO: better name
    pub fn take(&mut self) -> Mail<Message> {
        self.delivered = true;
        self.mail.clone()
    }

    pub fn take_final(self) -> Mail<Message> {
        self.mail
    }

    pub fn delivered(&self) -> bool {
        self.delivered
    }
}

#[derive(Debug, Clone)]
pub struct Mail<Message> {
    sort: u64,
    pub from: NodeIndex,
    pub msg: Message,
}

impl<M> Mail<M> {
    pub fn new<R: Rng + ?Sized>(rng: &mut R, from: NodeIndex, msg: M) -> Mail<M> {
        Self {
            sort: rng.random(),
            from,
            msg,
        }
    }
}

impl<M> PartialEq for Mail<M> {
    fn eq(&self, other: &Self) -> bool {
        self.sort.eq(&other.sort)
    }
}

impl<M> Eq for Mail<M> {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl<M> PartialOrd for Mail<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.sort.cmp(&other.sort))
    }
}

impl<M> Ord for Mail<M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort.cmp(&other.sort)
    }
}
