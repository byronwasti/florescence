// From sim
/*
nodes.add_node(SimNode {
    inner: PollinationNode::new(
        Uuid::from_u128(rng.random()),
        Topic::new("b".to_string()),
        NodeIndex::from(id as u32),
    ),
    mailbox: BinaryHeap::new(),
    last_heartbeat: 0,
    last_propagation: 0,
    last_reap: 0,
});

    /*
    for i in 0..node_count {
        for _ in 0..connections {
            let j = rng.random_range(0..node_count - 1);
            let j = if j >= i { j + 1 } else { j };
            let (i, j) = (i as u32, j as u32);

            nodes.add_edge(i.into(), j.into(), ());
        }
    }
    */
*/

// From Sim_node
/*
fn select_action<R: Rng + ?Sized>(
    &self,
    rng: &mut R,
    wall_time: u64,
    config: &S::Config,
) -> Option<S::Action> {
    let actions = self
        .simulee
        .as_ref()
        .expect("No simulee available.")
        .list_actions(wall_time, !self.mailbox.is_empty(), config);
    for (action, probability) in actions {
        if rng.random_bool(probability) {
            return Some(action);
        }
    }

    None
}
*/

/*
#[derive(Debug)]
pub struct SimNode {
    mailbox: BinaryHeap<Mail>,
    inner: PollinationNode<NodeIndex>,
    last_heartbeat: u64,
    last_propagation: u64,
    last_reap: u64,
}

impl Default for SimNode {
    fn default() -> SimNode {
        SimNode {
            inner: PollinationNode::new(
                Uuid::from_u128(0),
                Topic::new("Test".to_string()),
                NodeIndex::new(0),
            ),
            ..Default::default()
        }
    }
}

impl SimNode {
    /// Time is only `peace_time`; we don't want to trigger timeouts on normal prop of events
    /// TODO: Allow more propagation timing shenanigans
    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        if rng.random_bool(1. / (1. + self.mailbox.len() as f64)) {
            if let h @ Some(_) = self.step_timeout(rng, time, config) {
                return h;
            }
        }

        self.step_mailbox(rng, time, config)
    }

    fn step_timeout<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step timeout");

        if time - self.last_reap > config.timeout_reap {
            self.last_reap = time;

            if self.inner.reap_souls() {
                return Some(HistoricalEvent::GrimTheReaper);
            }
        }

        if time - self.last_heartbeat > config.timeout_heartbeat || self.last_heartbeat == 0 {
            self.last_heartbeat = time;

            if let Some(msg) = self.inner.msg_heartbeat() {
                return Some(HistoricalEvent::Heartbeat { msg });
            }

            let msg = self.inner.msg_new_member().unwrap();
            Some(HistoricalEvent::NewMember { msg })
        } else {
            None
        }
    }

    fn step_mailbox<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step mailbox");

        let in_msg = self.mailbox.pop()?.msg;

        let out = self.inner.handle_message(in_msg.clone());
        match out {
            Ok(PollinationResponse { response, .. }) => Some(HistoricalEvent::HandleMessage {
                in_msg,
                out_msg: response,
            }),

            Err(error) => Some(HistoricalEvent::HandleMessageError { msg: in_msg, error }),
        }
    }
}
*/

// From history
/*
#[derive(Debug)]
pub enum HistoricalEvent {
    NewMember {
        msg: PollinationMessage,
    },
    Heartbeat {
        msg: PollinationMessage,
    },
    GrimTheReaper,
    HandleMessage {
        in_msg: PollinationMessage,
        out_msg: Option<PollinationMessage>,
    },
    HandleMessageError {
        msg: PollinationMessage,
        error: PollinationError,
    },
    Panic {
        err: String,
    },
}
*/
