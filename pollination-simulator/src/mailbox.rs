use petgraph::graph::NodeIndex;
use rand::Rng;
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Debug, Default)]
pub struct Mailbox<Message> {
    counter: u64,
    inner: BinaryHeap<Mail<Message>>,
}

impl<Message> Mailbox<Message> {
    pub fn new() -> Mailbox<Message> {
        Self {
            counter: 0,
            inner: BinaryHeap::new(),
        }
    }

    pub fn push<R: Rng + ?Sized>(&mut self, rng: &mut R, from: NodeIndex, msg: Message) {
        // TODO: Go back to random
        //self.inner.push(Mail::new(rng, from, msg));
        self.inner.push(Mail::new_fixed(from, msg, self.counter));
        self.counter += 1;
    }

    pub fn push_mail(&mut self, mail: Mail<Message>) {
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

    pub fn iter(&self) -> impl Iterator<Item = &Mail<Message>> {
        self.inner.iter()
    }
}

#[derive(Debug)]
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

    pub(crate) fn take_final(self) -> Mail<Message> {
        self.mail
    }

    pub fn delivered(&self) -> bool {
        self.delivered
    }
}

#[derive(Debug, Clone)]
pub struct Mail<Message> {
    pub sort: std::cmp::Reverse<u64>,
    pub from: NodeIndex,
    pub msg: Message,
}

impl<M> Mail<M> {
    pub fn new<R: Rng + ?Sized>(rng: &mut R, from: NodeIndex, msg: M) -> Mail<M> {
        Self {
            sort: std::cmp::Reverse(rng.random()),
            from,
            msg,
        }
    }

    pub fn new_fixed(from: NodeIndex, msg: M, sort: u64) -> Mail<M> {
        Self {
            sort: std::cmp::Reverse(sort),
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
