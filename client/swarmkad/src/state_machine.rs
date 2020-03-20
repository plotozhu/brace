// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

use crate::{Network};

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::time;
use log::{trace, debug};
use futures::channel::mpsc;
use lru::LruCache;
use sc_network::PeerId;
use sp_runtime::traits::{Block as BlockT, Hash, HashFor};
pub use sc_network::message::generic::{Message, ConsensusMessage};
use sc_network::config::Roles;
use wasm_timer::Instant;
use codec::{Encode, Decode,Input,Output};
use sp_runtime::{ConsensusEngineId};
// FIXME: Add additional spam/DoS attack protection: https://github.com/paritytech/substrate/issues/1115
const KNOWN_MESSAGES_CACHE_SIZE: usize = 40960;
const MAX_PENDING_DATA: usize = 4096;
const MAX_DIRECT_PUSH_SIZE: usize = 256;
const REBROADCAST_INTERVAL: time::Duration = time::Duration::from_secs(30);

pub(crate) const PERIODIC_MAINTENANCE_INTERVAL: time::Duration = time::Duration::from_millis(1100);

//某个节点已经的消息哈希以及该节点的角色
#[derive(Clone)]
struct PeerConsensus<H> {
	known_messages: HashSet<H>,
	roles: Roles,
}
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct MessageWithTopic {
	pub topic:Vec<u8>,
	pub  message:Vec<u8>
}
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct MessageWithHash<B:BlockT> {
	pub hash : B::Hash,
	pub msg :	MessageWithTopic,
}

#[derive(Debug, Eq, PartialEq,Clone,Encode)]
pub enum PPMsg<B:BlockT>{
	PushHash(B::Hash),
	PullData(B::Hash),
	PushData(B::Hash, MessageWithTopic),
}



pub type PPHandleID =  u8;

pub const PUSH_HASH:PPHandleID = 0x01;
pub const PULL_DATA:PPHandleID = 0x02;
pub const PUSH_DATA:PPHandleID = 0x03;

/// Topic stream message with sender.
#[derive(Debug, Eq, PartialEq)]
pub struct TopicNotification {
	/// Message data.
	pub message: Vec<u8>,
	/// Sender if available.
	pub sender: Option<PeerId>,
}


/// Consensus network protocol handler. Manages statements and candidate requests.

pub struct TransPPEngine<B: BlockT> {
	// peers supporting push/pull protocol
	peers: HashMap<PeerId, PeerConsensus<B::Hash>>,
	
	live_message_sinks: HashMap<Vec<u8>, Vec<mpsc::UnboundedSender<TopicNotification>>>,
	// messages those hash has been sent
	pending_messages: LruCache<B::Hash,MessageWithTopic >,
	// record recently received hashes
	received_messages: LruCache<B::Hash, ()>,
}

impl<B: BlockT> TransPPEngine<B> {
	/// Create a new instance.
	pub fn new() -> Self {
		TransPPEngine {
			peers: HashMap::new(),
			live_message_sinks: HashMap::new(),
			pending_messages:  LruCache::new(MAX_PENDING_DATA),
			received_messages: LruCache::new(KNOWN_MESSAGES_CACHE_SIZE),

		}
	}

	/// record peers supporting push/pull protocol
	pub fn new_peer(&mut self, network: &mut dyn Network<B>, who: PeerId, roles: Roles) {
		// light nodes are not valid targets for consensus gossip messages

		trace!(target:"gossip", "Registering {:?} {}", roles, who);
		self.peers.insert(who.clone(), PeerConsensus {
			known_messages: HashSet::new(),
			roles,
		});

	}
		/// Call when a peer has been disconnected to stop tracking gossip status.
		pub fn peer_disconnected(&mut self, network: &mut dyn Network<B>, who: PeerId) {
			// for (engine_id, v) in self.validators.clone() {
			// 	let mut context = NetworkContext { gossip: self, network, engine_id: engine_id.clone() };
			// 	v.peer_disconnected(&mut context, &who);
			// }
			self.peers.remove(&who);
		}

	 pub fn send_message(&mut self,network:&mut dyn Network<B>,peers :Vec<sc_network::PeerId>,message:MessageWithTopic){
		let msg_to_send  ;
		let mut cmd = PUSH_HASH;
		if message.message.len() > MAX_DIRECT_PUSH_SIZE {
			 let  hash = HashFor::<B>::hash_of(&message);
			 self.pending_messages.put(hash,message);
			 msg_to_send = PPMsg::PushHash(hash);
		 }else {
			let  hash = HashFor::<B>::hash_of(&message);
			 msg_to_send = PPMsg::PushData(hash,message);
			 cmd = PUSH_DATA;
		 }
		 for peer in peers {
			 //check if this peer is in our 
			if  self.peers.contains_key(&peer) {
				self.do_send_message(network,peer,cmd,msg_to_send.clone());
			};
		 }

	 }
	 fn response_push_hash(&mut self,network:&mut dyn Network<B>,incoming_peer:sc_network::PeerId,hash: B::Hash){
		 match self.received_messages.get(& hash) {
			 None =>{
				 self.do_send_message(network,incoming_peer,PULL_DATA,PPMsg::PullData(hash))
			 },
			 // ignored on message received
			 _ => return,
		 }
	 }
	 fn respone_pull_data(&mut self,network:&mut dyn Network<B>,incoming_peer:sc_network::PeerId,hash: B::Hash){
		match self.pending_messages.get(& hash) {
			Some(msgWithTopic) =>{
				let result = msgWithTopic.clone();
				self.do_send_message(network,incoming_peer,PUSH_DATA,PPMsg::PushData(hash,result))
			},
			_ => return,
		} 
	 }
	/// Send addressed message to a peer. The message is not kept or multicast
	/// later on.
	 fn do_send_message(
		&mut self,
		network: &mut dyn Network<B>,
		who: sc_network::PeerId,
		cmd: PPHandleID,
		message: PPMsg<B>,
	){
		let to_cmd = vec![cmd,0,0,0]  ;
		let mut engine_id = [0; 4];
		engine_id.copy_from_slice(&to_cmd[0..4]);
		network.write_notification(who.clone(),engine_id, message.encode());
	}

	


	/// Get data of valid, incoming messages for a topic (but might have expired meanwhile)
	pub fn messages_for(&mut self, topic: Vec<u8>)
		-> mpsc::UnboundedReceiver<TopicNotification>
	{
		let (tx, rx) = mpsc::unbounded();

		self.live_message_sinks.entry(topic).or_default().push(tx);

		rx
	}

	fn streamout(&mut self,who:sc_network::PeerId, topic:Vec<u8>, message:Vec<u8>){

		if let Entry::Occupied(mut entry) = self.live_message_sinks.entry(topic) {
			//这个entry是Vec类型的
		//	debug!(target: "gossip", "Pushing consensus message to sinks for {}.", topic);
			entry.get_mut().retain(|sink| {
				if let Err(e) = sink.unbounded_send(TopicNotification {
					message: message.clone(),
					sender: Some(who.clone())
				}) {
					trace!(target: "gossip", "Error broadcasting message notification: {:?}", e);
				}
				!sink.is_closed()
			});
			if entry.get().is_empty() {
				entry.remove_entry();
			}
		}
	}

	/// Handle an incoming ConsensusMessage for topic by who via protocol. Discard message if topic
	/// already known, the message is old, its source peers isn't a registered peer or the connection
	/// to them is broken. Return `Some(topic, message)` if it was added to the internal queue, `None`
	/// in all other cases.
	pub fn on_incoming(
		&mut self,
		network: &mut dyn Network<B>,
		who: sc_network::PeerId,
		messages: Vec<PPMsg<B>>,
	) {
		if !messages.is_empty() {
			trace!(target: "gossip", "Received {} messages from peer {}", messages.len(), who);
		}

		for message in messages {
			match Some(message) {
				Some(PPMsg::PushHash(data)) => self.response_push_hash(network,who.clone(),data),
				Some( PPMsg::PushData(topic,data)) =>self.streamout(who.clone(),data.topic,data.message),
				Some(PPMsg::PullData(data)) => self.respone_pull_data(network,who.clone(),data),
				None =>{},
			}
			
		}
	}
	

}
mod rep {
	use sc_network::ReputationChange as Rep;
	/// Reputation change when a peer sends us a gossip message that we didn't know about.
	pub const GOSSIP_SUCCESS: Rep = Rep::new(1 << 4, "Successfull gossip");
	/// Reputation change when a peer sends us a gossip message that we already knew about.
	pub const DUPLICATE_GOSSIP: Rep = Rep::new(-(1 << 2), "Duplicate gossip");
	/// Reputation change when a peer sends us a gossip message for an unknown engine, whatever that
	/// means.
	pub const UNKNOWN_GOSSIP: Rep = Rep::new(-(1 << 6), "Unknown gossip message engine id");
	/// Reputation change when a peer sends a message from a topic it isn't registered on.
	pub const UNREGISTERED_TOPIC: Rep = Rep::new(-(1 << 10), "Unregistered gossip message topic");
}

