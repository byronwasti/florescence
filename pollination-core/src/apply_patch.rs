use crate::node_info::NodeInfo;
use std::{cmp::Ordering, fmt::Debug};
use tracing::{debug, info, warn};
use treeclocks::{IdTree, itc_map::ItcMapTree};

#[derive(Clone, Debug, PartialEq)]
pub struct PatchOutcome<'a, A> {
    pub insert: Vec<(IdTree, &'a NodeInfo<A>)>,
}

impl<'a, A> Default for PatchOutcome<'a, A> {
    fn default() -> Self {
        PatchOutcome { insert: vec![] }
    }
}

impl<'a, A> PatchOutcome<'a, A> {
    fn new(id: IdTree, node_info: &'a NodeInfo<A>) -> Self {
        let insert = vec![(id, node_info)];
        PatchOutcome { insert }
    }

    fn merge(mut self, mut other: PatchOutcome<'a, A>) -> Self {
        self.insert.append(&mut other.insert);
        self
    }
}

pub fn apply_patch<'a, A>(
    own_map: &'a ItcMapTree<NodeInfo<A>>,
    peer_map: &'a ItcMapTree<NodeInfo<A>>,
) -> PatchOutcome<'a, A>
where
    A: PartialEq + Debug,
{
    use ItcMapTree::*;

    match (own_map, peer_map) {
        // TODO: Ideally own_map containing Unknown doesn't happen. More checks should be put in
        // place to do that.
        (Unknown, Unknown) => PatchOutcome::default(),
        (Unknown, Leaf { id_tree, value, .. }) => PatchOutcome::new((*id_tree).clone(), value),
        (a @ Unknown, SubTree(l, r)) => {
            let l = apply_patch(a, l);
            let r = apply_patch(a, r);

            l.merge(r)
        }
        (_, Unknown) => PatchOutcome::default(),

        (
            Leaf {
                id_tree: id0,
                timestamp: ts0,
                value: v0,
            },
            Leaf {
                id_tree: id1,
                timestamp: ts1,
                value: v1,
            },
        ) => {
            debug!("id0={id0} id1={id1} ts0={ts0} ts1={ts1} v0={v0:?} v1={v1:?}");
            match ts0.cmp(ts1) {
                Ordering::Greater => {
                    info!("Self contains a greater timestamp, ignoring patch");
                    PatchOutcome::default()
                }
                Ordering::Less => PatchOutcome::new((*id1).clone(), v1),
                Ordering::Equal => {
                    if v0.uuid != v1.uuid {
                        // Panicking for now because we don't have removal of peers yet, so this
                        // should never happen. In the future we just take the greater
                        warn!(
                            "UUID skew between equal peers. Indicates a partition event occurred. {} != {}",
                            v0.uuid, v1.uuid,
                        );
                        // TODO: UGH this stuff is hard
                        //panic!("TODO: Remove panic");
                        if v1.uuid > v0.uuid {
                            PatchOutcome::new((*id1).clone(), v1)
                        } else {
                            PatchOutcome::default()
                        }
                    } else {
                        // TODO: Both of these asserts _should_ apply, but unfortunately
                        // don't during convergence. Need to figure out a way to make them work.
                        //assert_eq!(id0, id1);
                        //assert_eq!(v0, v1);
                        PatchOutcome::default()
                    }
                }
            }
        }

        (a @ Leaf { .. }, SubTree(l, r)) => {
            let l = apply_patch(a, l);
            let r = apply_patch(a, r);

            l.merge(r)
        }

        (SubTree(l, r), b @ Leaf { .. }) => {
            let l = apply_patch(l, b);
            let r = apply_patch(r, b);

            l.merge(r)
        }

        (SubTree(l0, r0), SubTree(l1, r1)) => {
            let l = apply_patch(l0, l1);
            let r = apply_patch(r0, r1);
            l.merge(r)
        }
    }
}
