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
#![allow(unused_variables)]

use cbor::{Decoder, Encoder, CborError};
use rand;
use rustc_serialize::{Decodable, Encodable};
use sodiumoxide;
use sodiumoxide::crypto::sign::verify_detached;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc;
use std::boxed::Box;
use std::ops::DerefMut;
use std::sync::mpsc::Receiver;
use time::{Duration, SteadyTime};

use challenge::{ChallengeRequest, ChallengeResponse, validate};
use crust;
use lru_time_cache::LruCache;
use message_filter::MessageFilter;
use NameType;
use name_type::{closer_to_target_or_equal, NAME_TYPE_LEN};
use node_interface;
use node_interface::{Interface, CreatePersonas};
use routing_table::{RoutingTable, NodeInfo};
use relay::RelayMap;
use routing_membrane::RoutingMembrane;
use sendable::Sendable;
use types;
use types::{MessageId, NameAndTypeId, Signature, Bytes};
use authority::{Authority, our_authority};
use message_header::MessageHeader;
use messages::get_data::GetData;
use messages::get_data_response::GetDataResponse;
use messages::put_data::PutData;
use messages::put_data_response::PutDataResponse;
use messages::connect_request::ConnectRequest;
use messages::connect_response::ConnectResponse;
use messages::connect_success::ConnectSuccess;
use messages::find_group::FindGroup;
use messages::find_group_response::FindGroupResponse;
use messages::get_group_key::GetGroupKey;
use messages::get_group_key_response::GetGroupKeyResponse;
use messages::post::Post;
use messages::get_client_key::GetKey;
use messages::get_client_key_response::GetKeyResponse;
use messages::put_public_id::PutPublicId;
use messages::put_public_id_response::PutPublicIdResponse;
use messages::{RoutingMessage, MessageTypeTag};
use types::{MessageAction};
use error::{RoutingError, InterfaceError, ResponseError};
use std::thread::{spawn, JoinHandle};

use std::convert::From;
use std::marker::PhantomData;

type ConnectionManager = crust::ConnectionManager;
type Event = crust::Event;
pub type Endpoint = crust::Endpoint;
type PortAndProtocol = crust::Port;

type RoutingResult = Result<(), RoutingError>;

/// DHT node
pub struct RoutingNode<F, G> where F : Interface + 'static,
                                   G : CreatePersonas<F> {
    genesis: Box<G>,
    phantom: PhantomData<F>,
    id: types::Id,
    own_name: NameType,
    // event_input: Receiver<Event>,
    // connection_manager: ConnectionManager,
    // accepting_on: Vec<Endpoint>,
    next_message_id: MessageId,
    bootstrap_endpoint: Option<Endpoint>,
    bootstrap_node_id: Option<NameType>,
    // membrane_handle: Option<JoinHandle<_>>
}

impl<F, G> RoutingNode<F, G> where F: Interface + 'static,
                                   G : CreatePersonas<F> {
    pub fn new(genesis: G) -> RoutingNode<F, G> {
        sodiumoxide::init();  // enable shared global (i.e. safe to multithread now)
        // let (event_output, event_input) = mpsc::channel();
        let id = types::Id::new();
        let own_name = id.get_name();
        // let mut cm = crust::ConnectionManager::new(event_output);
        // TODO: Default Protocol and Port need to be passed down
        // let ports_and_protocols : Vec<PortAndProtocol> = Vec::new();
        // TODO: Beacon port should be passed down
        // let beacon_port = Some(5483u16);
        // let listeners = match cm.start_listening2(ports_and_protocols, beacon_port) {
        //     Err(reason) => {
        //         println!("Failed to start listening: {:?}", reason);
        //         (vec![], None)
        //     }
        //     Ok(listeners_and_beacon) => listeners_and_beacon
        // };
        // println!("{:?}  -- listening on : {:?}", own_name, listeners.0);
        RoutingNode { genesis: Box::new(genesis),
                      phantom: PhantomData,
                      id : id,
                      own_name : own_name.clone(),
                      // event_input: event_input,
                      // connection_manager: cm,
                      // accepting_on: listeners.0,
                      next_message_id: rand::random::<MessageId>(),
                      bootstrap_endpoint: None,
                      bootstrap_node_id: None,
                    }
    }

    /// Starts a node without requiring responses from the network.
    /// It will relocate its own address with the a double hash.
    /// This allows the network to later reject this zero node
    /// when the routing_table is full.
    pub fn zero_node(genesis: G) -> RoutingNode<F, G> {
        sodiumoxide::init();  // enable shared global (i.e. safe to multithread now)
        let mut id = types::Id::new();
        let original_name = id.get_name();
        let self_relocated_name = match types::calculate_relocated_name(
            vec![original_name.clone()], &original_name) {
            Ok(self_relocated_name) => self_relocated_name,
            Err(_) => panic!("Could not self-relocate our name.") // unreachable
        };
        id.assign_relocated_name(self_relocated_name);
        let own_name = id.get_name(); // is equal to self_relocated_name
        RoutingNode { genesis: Box::new(genesis),
                      phantom: PhantomData,
                      id : id,
                      own_name : own_name.clone(),
                      next_message_id: rand::random::<MessageId>(),
                      bootstrap_endpoint: None,
                      bootstrap_node_id: None,
                    }
    }

    /// run_membrane spawns a new thread and moves a newly constructed Membrane into this thread.
    /// Routing node uses the genesis object to create a new instance of the personas to embed
    /// inside the membrane.
    //  TODO: a (two-way) channel should be passed in to control the membrane.
    //        connection_manager should also be moved into the membrane;
    //        firstly moving most ownership of the constructor into this function.
    fn run_membrane(&mut self)  {
        let (event_output, event_input) = mpsc::channel();
        let mut cm = crust::ConnectionManager::new(event_output);
        // TODO: Default Protocol and Port need to be passed down
        let ports_and_protocols : Vec<PortAndProtocol> = Vec::new();
        // TODO: Beacon port should be passed down
        let beacon_port = Some(5483u16);
        let listeners = match cm.start_listening2(ports_and_protocols, beacon_port) {
            Err(reason) => {
                println!("Failed to start listening: {:?}", reason);
                (vec![], None)
            }
            Ok(listeners_and_beacon) => listeners_and_beacon
        };

        let relocated_id = self.bootstrap();
        // for now just write out explicitly in this function the bootstrapping
        loop {
            match event_input.recv() {
                Err(_) => (),
                Ok(crust::Event::NewMessage(endpoint, bytes)) => {

                },
                Ok(crust::Event::NewConnection(endpoint)) => {

                },
                Ok(crust::Event::LostConnection(endpoint)) => {

                }
            }
        }

        match (self.bootstrap_node_id.clone(), self.bootstrap_endpoint.clone()) {
            (Some(name), Some(endpoint)) => {
                let mut membrane = RoutingMembrane::new(
                    cm, event_input, (name, endpoint),
                    listeners.0, relocated_id,
                    self.genesis.create_personas());
                spawn(move || membrane.run());
            },
            _ => () // failed to bootstrap
        }
    }

    /// bootstrap
    fn bootstrap(&mut self) -> types::Id  {
        types::Id::new()  // TODO: placeholder
    }

}

fn encode<T>(value: &T) -> Result<Bytes, CborError> where T: Encodable {
    let mut enc = Encoder::from_memory();
    try!(enc.encode(&[value]));
    Ok(enc.into_bytes())
}

fn decode<T>(bytes: &Bytes) -> Result<T, CborError> where T: Decodable {
    let mut dec = Decoder::from_bytes(&bytes[..]);
    match dec.decode().next() {
        Some(result) => result,
        None => Err(CborError::UnexpectedEOF)
    }
}

fn ignore<R,E>(_: Result<R,E>) {}

#[cfg(test)]
mod test {

}
