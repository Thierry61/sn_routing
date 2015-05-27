// Copyright 2015 MaidSafe.net limited.
//
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

//! This module handle all connections that are not managed by the routing table.
//!
//! As such the relay module handles messages that need to flow in or out of the SAFE network.
//! These messages include bootstrap actions by starting nodes or relay messages for clients.

use std::collections::{BTreeMap, BTreeSet};
use crust::Endpoint;
use types::PublicId;

const MAX_RELAY : usize = 5;

// #[derive(Clone, Debug)]
// pub struct IpNodeInfo {
//     pub fob: PublicId,
//     // drop IpNodeInfo upon losing connection
//     pub connected_endpoint: Endpoint
// }
//
// impl IpNodeInfo {
//     pub fn new(fob: PublicId, connected_endpoint: Endpoint) -> IpNodeInfo {
//         IpNodeInfo {
//             fob: fob,
//             connected_endpoint: connected_endpoint
//         }
//     }
// }

/// The relay map is used to maintain a list of contacts for whom
/// we are relaying messages, when we are ourselves connected to the network.
pub struct RelayMap {
    relay_map: BTreeMap<NameType, (PublicId, BTreeSet<Endpoint>)>,
    lookup_map: HashMap<Endpoint, NameType>,
    our_id: NameType
}

impl RelayMap {
    /// This creates a new RelayMap.
    pub fn new(our_id: NameType) -> RelayMap {
        RelayMap {
            relay_map: BTreeMap::with_capacity(MAX_RELAY),
            lookup_map: HashMap::<Endpoint, NameType>::new(),
            our_id: our_id
        }
    }

    /// Adds an IP Node info to the relay map if the relay map has open
    /// slots.  This returns true if Info was addded.
    /// Returns true is the endpoint is newly added, or was already present.
    /// Returns false if the threshold was reached or name is our name.
    pub fn add_ip_node(&mut self, relay_info: PublicId, relay_endpoint: Endpoint) -> bool {
        // always reject our own id
        if self.our_id == their_info.name {
            return false;
        }

        // impose limit on number of relay nodes active
        if !self.relay_map.contains_key(relay_info.fob.name)
            && self.relay_map.len() >= MAX_RELAY {
            return false;
        }

        let new_set = || { (relay_info, BTreeSet::<Endpoint>::new()) };
        // returns true if the endpoint is newly added, false if the
        self.relay_map.entry(relay_info.fob.name).or_insert_with(new_set).1
                      .insert(relay_endpoint);
        true
    }

    /// This removes the ip_node from the relay map.
    pub fn drop_ip_node(&mut self, ip_node_to_drop: &NameType) {

    }
}

/// Bootstrap endpoints are used to connect to the network before
/// routing table connections are established.
pub struct BootstrapEndpoints {
    bootstrap_endpoints: Vec<IpNodeInfo>,
}
