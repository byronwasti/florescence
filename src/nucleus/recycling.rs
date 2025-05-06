use treeclocks::IdTree;

pub fn claim_ids(own: IdTree, dead_peers: IdTree) -> IdTree {
    let reclaim_tree = claim_ids_recurse(own, dead_peers);
    convert(reclaim_tree)
}

fn claim_ids_recurse(own: IdTree, dead_peers: IdTree) -> IdReclaimTree {
    println!("{own} : {dead_peers}");
    use IdTree::*;
    match (own, dead_peers) {
        (Zero, Zero) => IdReclaimTree::Zero,
        (Zero, One) => IdReclaimTree::Dead,
        (One, Zero) => IdReclaimTree::One,
        (One, One) => panic!("Logic bug"),

        (Zero, SubTree(..)) => IdReclaimTree::Zero,
        (One, SubTree(..)) => panic!("Logic bug"),

        (SubTree(..), One) => panic!("Logic bug"),
        (SubTree(l, r), Zero) => {
            let l = claim_ids_recurse(*l, Zero);
            let r = claim_ids_recurse(*r, Zero);
            match (l, r) {
                (l @ IdReclaimTree::TrendingLeft(..) | l @ IdReclaimTree::One, r) => {
                    IdReclaimTree::TrendingLeft(Box::new(l), Box::new(r))
                }
                (l, r @ IdReclaimTree::TrendingRight(..) | r @ IdReclaimTree::One) => {
                    IdReclaimTree::TrendingRight(Box::new(l), Box::new(r))
                }
                (l, r) => IdReclaimTree::SubTree(Box::new(l), Box::new(r)),
            }
        }
        (SubTree(l0, r0), SubTree(l1, r1)) => {
            let l = claim_ids_recurse(*l0, *l1);
            let r = claim_ids_recurse(*r0, *r1);
            use IdReclaimTree as Irt;
            match (l, r) {
                (Irt::Dead, Irt::Dead) => panic!("Logic bug"),
                (Irt::Dead | Irt::Zero, Irt::Dead | Irt::Zero) => Irt::Zero,

                (Irt::One, Irt::Dead) | (Irt::Dead, Irt::One) => Irt::One,
                (Irt::TrendingRight(..), Irt::Dead) => {
                    Irt::SubTree(Box::new(Irt::Zero), Box::new(Irt::One))
                }
                (Irt::Dead, Irt::TrendingLeft(..)) => {
                    Irt::SubTree(Box::new(Irt::One), Box::new(Irt::Zero))
                }

                (l @ Irt::TrendingLeft(..), Irt::Zero) => {
                    Irt::TrendingLeft(Box::new(l), Box::new(Irt::Zero))
                }
                (Irt::Zero, r @ Irt::TrendingRight(..)) => {
                    Irt::TrendingRight(Box::new(Irt::Zero), Box::new(r))
                }
                (
                    IdReclaimTree::TrendingLeft(..)
                    | IdReclaimTree::TrendingRight(..)
                    | IdReclaimTree::SubTree(..),
                    IdReclaimTree::TrendingLeft(..)
                    | IdReclaimTree::TrendingRight(..)
                    | IdReclaimTree::SubTree(..),
                ) => panic!("Logic Bug"),

                (l, r) => Irt::SubTree(Box::new(l), Box::new(r)),
            }
        }
    }
}

fn convert(irt: IdReclaimTree) -> IdTree {
    use IdReclaimTree::*;
    match irt {
        Dead | Zero => IdTree::Zero,
        One => IdTree::One,
        SubTree(l, r) | TrendingLeft(l, r) | TrendingRight(l, r) => {
            let l = convert(*l);
            let r = convert(*r);
            IdTree::SubTree(Box::new(l), Box::new(r))
        }
    }
}

#[derive(Debug)]
enum IdReclaimTree {
    Dead,
    Zero,
    One,
    SubTree(Box<IdReclaimTree>, Box<IdReclaimTree>),
    TrendingLeft(Box<IdReclaimTree>, Box<IdReclaimTree>),
    TrendingRight(Box<IdReclaimTree>, Box<IdReclaimTree>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_reclaim() {
        use IdTree::*;

        let i0 = SubTree(Box::new(One), Box::new(Zero));
        let i1 = SubTree(Box::new(Zero), Box::new(One));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "1".to_string());
    }

    #[test]
    fn test_nested_reclaim_left() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(Zero),
            Box::new(SubTree(Box::new(One), Box::new(Zero))),
        );
        let i1 = SubTree(Box::new(One), Box::new(Zero));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "(1, 0)".to_string());
    }

    #[test]
    fn test_doubly_nested_reclaim_left() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(Zero),
            Box::new(SubTree(
                Box::new(SubTree(Box::new(One), Box::new(Zero))),
                Box::new(Zero),
            )),
        );
        let i1 = SubTree(Box::new(One), Box::new(Zero));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "(1, 0)".to_string());
    }

    #[test]
    fn test_nested_reclaim_right() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(SubTree(Box::new(Zero), Box::new(One))),
            Box::new(Zero),
        );
        let i1 = SubTree(Box::new(Zero), Box::new(One));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "(0, 1)".to_string());
    }

    #[test]
    fn test_doubly_nested_reclaim_right() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(SubTree(
                Box::new(Zero),
                Box::new(SubTree(Box::new(Zero), Box::new(One))),
            )),
            Box::new(Zero),
        );
        let i1 = SubTree(Box::new(Zero), Box::new(One));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "(0, 1)".to_string());
    }

    #[test]
    fn test_no_reclaim() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(SubTree(
                Box::new(Zero),
                Box::new(SubTree(Box::new(One), Box::new(Zero))),
            )),
            Box::new(Zero),
        );
        let i1 = SubTree(Box::new(Zero), Box::new(One));

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "((0, (1, 0)), 0)".to_string());
    }

    #[test]
    fn some_reclaim() {
        use IdTree::*;

        let i0 = SubTree(
            Box::new(SubTree(
                Box::new(Zero),
                Box::new(SubTree(Box::new(One), Box::new(Zero))),
            )),
            Box::new(Zero),
        );
        let i1 = SubTree(
            Box::new(SubTree(
                Box::new(Zero),
                Box::new(SubTree(Box::new(Zero), Box::new(One))),
            )),
            Box::new(One),
        );

        let new_id = claim_ids(i0, i1);

        assert_eq!(new_id.to_string(), "((0, 1), 0)".to_string());
    }
}
