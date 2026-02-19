use crate::{config::Config, history::*, mailbox::Mailbox, traits::*};
use petgraph::graph::NodeIndex;
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::panic;
use thiserror::Error;

#[derive(Debug)]
pub struct SimNode<S: Simulee> {
    pub id: NodeIndex,
    mailbox: Mailbox<S::Message>,
    simulee: Option<S>,
}

impl<S: Simulee> SimNode<S> {
    pub fn new<R: Rng + ?Sized>(
        rng: &mut R,
        config: &Config<S::Config>,
        id: NodeIndex,
    ) -> SimNode<S> {
        Self {
            id,
            mailbox: Mailbox::new(),
            simulee: Some(S::new(rng, config, id)),
        }
    }

    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        wall_time: u64,
        config: &Config<S::Config>,
    ) -> Result<NodeRecord<S>, SimNodeError> {
        // Before doing anything, we want to snapshot the node as-is.
        let snapshot = self
            .simulee
            .as_ref()
            .ok_or(SimNodeError::Panic("No simulee available.".to_string()))?
            .clone();

        let mut delivery = self.mailbox.get_delivery();
        let mut simulee = self
            .simulee
            .take()
            .ok_or(SimNodeError::Panic("No simulee available.".to_string()))?;
        let config = config.clone();
        let seed = rng.random();
        let res = panic::catch_unwind(move || {
            // Can't pass an Rng across the unwind boundary, so just reseed a new one.
            let mut rng = StdRng::seed_from_u64(seed);
            let res = simulee.step(&mut rng, &config, wall_time, &mut delivery);
            (simulee, delivery, res)
        })
        .map_err(|err| {
            if let Some(err) = err.downcast_ref::<&str>() {
                SimNodeError::Panic(err.to_string())
            } else if let Some(err) = err.downcast_ref::<String>() {
                SimNodeError::Panic(err.to_owned())
            } else {
                SimNodeError::PanicUnknown
            }
        })?;

        let (simulee, delivery, res) = res;
        self.simulee = Some(simulee);

        let msg_in = if let Some(delivery) = delivery {
            let delivered = delivery.delivered();
            let mail = delivery.take_final();
            if delivered {
                Some(mail)
            } else {
                self.mailbox.push(mail);
                None
            }
        } else {
            None
        };

        match (msg_in, res) {
            (msg_in, Some((event, msgs_out))) => Ok(NodeRecord {
                id: self.id,
                snapshot,
                event,
                msg_in,
                msgs_out,
            }),
            (Some(_), None) => {
                // Node took a message but didn't record any event. This is an error
                Err(SimNodeError::Panic("Stolen message".to_string()))
            }
            (None, None) => Err(SimNodeError::NoAction),
        }
    }
}

#[derive(Debug, Error)]
pub enum SimNodeError {
    #[error("No action")]
    NoAction,

    #[error("Node hit a panic: {0}")]
    Panic(String),

    #[error("Node hit a panic (unknown payload)")]
    PanicUnknown,
}
