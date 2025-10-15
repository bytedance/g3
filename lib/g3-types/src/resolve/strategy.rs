/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::str::FromStr;

use anyhow::anyhow;

use crate::collection::{SelectiveItem, SelectiveVecBuilder};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QueryStrategy {
    Ipv4Only,
    Ipv6Only,
    #[default]
    Ipv4First,
    Ipv6First,
}

impl QueryStrategy {
    fn adjust_to(self, other: Self) -> Self {
        if matches!(self, Self::Ipv4Only | Self::Ipv6Only) {
            self
        } else {
            other
        }
    }
}

impl FromStr for QueryStrategy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "ipv4only" | "ipv4_only" => Ok(QueryStrategy::Ipv4Only),
            "ipv6only" | "ipv6_only" => Ok(QueryStrategy::Ipv6Only),
            "ipv4first" | "ipv4_first" => Ok(QueryStrategy::Ipv4First),
            "ipv6first" | "ipv6_first" => Ok(QueryStrategy::Ipv6First),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PickStrategy {
    #[default]
    Random,
    Serial,
}

impl FromStr for PickStrategy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "random" => Ok(PickStrategy::Random),
            "serial" | "first" => Ok(PickStrategy::Serial),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResolveStrategy {
    pub query: QueryStrategy,
    pub pick: PickStrategy,
}

impl ResolveStrategy {
    #[must_use]
    pub fn adjust_to(self, other: Self) -> Self {
        let query = self.query.adjust_to(other.query);
        ResolveStrategy {
            query,
            pick: other.pick,
        }
    }

    #[inline]
    pub fn query_v4only(&mut self) {
        self.query = QueryStrategy::Ipv4Only;
    }

    #[inline]
    pub fn query_v6only(&mut self) {
        self.query = QueryStrategy::Ipv6Only;
    }

    pub fn update_query_strategy(&mut self, no_ipv4: bool, no_ipv6: bool) -> anyhow::Result<()> {
        if no_ipv4 {
            match self.query {
                QueryStrategy::Ipv4Only => {
                    return Err(anyhow!(
                        "query strategy is {:?} but ipv4 is disabled",
                        self.query
                    ));
                }
                QueryStrategy::Ipv6Only => {}
                QueryStrategy::Ipv6First | QueryStrategy::Ipv4First => {
                    self.query = QueryStrategy::Ipv6Only;
                }
            };
        }
        if no_ipv6 {
            match self.query {
                QueryStrategy::Ipv6Only => {
                    return Err(anyhow!(
                        "query strategy is {:?} but ipv6 is disabled",
                        self.query
                    ));
                }
                QueryStrategy::Ipv4Only => {}
                QueryStrategy::Ipv4First | QueryStrategy::Ipv6First => {
                    self.query = QueryStrategy::Ipv4Only;
                }
            };
        }
        Ok(())
    }

    pub fn pick_many<T: Copy>(&self, mut all: Vec<T>, count: usize) -> Vec<T> {
        if all.len() > 1 {
            match self.pick {
                PickStrategy::Serial => {
                    all.truncate(count);
                }
                PickStrategy::Random => {
                    fastrand::shuffle(&mut all);
                    all.truncate(count);
                }
            }
        }
        all
    }

    pub fn pick_best<T: Copy>(&self, mut all: Vec<T>) -> Option<T> {
        if all.len() > 1 {
            match self.pick {
                PickStrategy::Serial => all.into_iter().next(),
                PickStrategy::Random => fastrand::choice(all),
            }
        } else {
            all.pop()
        }
    }

    pub fn pick_ketama<T, K>(mut all: Vec<T>, key: &K) -> Option<T>
    where
        T: SelectiveItem + Ord + Copy,
        K: Hash + ?Sized,
    {
        all.sort();
        let builder = SelectiveVecBuilder::with_inner(all);
        let selective_vec = builder.build()?;
        Some(*selective_vec.pick_ketama(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_query() {
        assert_eq!(
            QueryStrategy::Ipv4Only.adjust_to(QueryStrategy::Ipv4First),
            QueryStrategy::Ipv4Only
        );

        assert_eq!(
            QueryStrategy::Ipv6Only.adjust_to(QueryStrategy::Ipv6First),
            QueryStrategy::Ipv6Only
        );

        assert_eq!(
            QueryStrategy::Ipv4First.adjust_to(QueryStrategy::Ipv4Only),
            QueryStrategy::Ipv4Only
        );

        assert_eq!(
            QueryStrategy::Ipv6First.adjust_to(QueryStrategy::Ipv4Only),
            QueryStrategy::Ipv4Only
        );
    }

    #[test]
    fn t_resolve() {
        let mut s = ResolveStrategy::default();
        s.update_query_strategy(false, true).unwrap();
        assert_eq!(s.query, QueryStrategy::Ipv4Only);

        let mut s = ResolveStrategy::default();
        s.update_query_strategy(true, false).unwrap();
        assert_eq!(s.query, QueryStrategy::Ipv6Only);

        let mut s = ResolveStrategy::default();
        s.update_query_strategy(false, true).unwrap();
        assert_eq!(s.query, QueryStrategy::Ipv4Only);

        let mut s = ResolveStrategy::default();
        s.query_v6only();
        assert!(s.update_query_strategy(false, true).is_err());

        let mut s = ResolveStrategy::default();
        s.query_v4only();
        assert!(s.update_query_strategy(true, false).is_err());

        let mut s = ResolveStrategy::default();
        assert!(s.update_query_strategy(true, true).is_err());

        let s = ResolveStrategy {
            pick: PickStrategy::Serial,
            ..Default::default()
        };
        assert_eq!(s.pick_best(vec![1, 2]), Some(1));
        assert_eq!(s.pick_many(vec![1, 2, 3], 2), vec![1, 2]);
    }
}
