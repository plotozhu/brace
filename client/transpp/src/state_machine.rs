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
use codec::{Encode, Decode};

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
	topic:Vec<u8>,
	message:Vec<u8>
}

#[derive(Debug, Eq, PartialEq,Clone)]
pub enum PPMsg<B:BlockT>{
	PushHash(B::Hash),
	PullData(B::Hash),
	PushData(B::Hash, MessageWithTopic),
}
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
		if message.message.len() > MAX_DIRECT_PUSH_SIZE {
			 let  hash = HashFor::<B>::hash_of(&message);
			 self.pending_messages.put(hash,message);
			 msg_to_send = PPMsg::PushHash(hash);
		 }else {
			let  hash = HashFor::<B>::hash_of(&message);
			 msg_to_send = PPMsg::PushData(hash,message)
		 }
		 for peer in peers {
			 //check if this peer is in our 
			if  self.peers.contains_key(&peer) {
				self.do_send_message(network,peer,msg_to_send.clone());
			};
		 }

	 }
	 fn response_push_hash(&mut self,network:&mut dyn Network<B>,incoming_peer:sc_network::PeerId,hash: B::Hash){
		 match self.received_messages.get(& hash) {
			 None =>{
				 self.do_send_message(network,incoming_peer,PPMsg::PullData(hash))
			 },
			 // ignored on message received
			 _ => return,
		 }
	 }
	 fn respone_pull_data(&mut self,network:&mut dyn Network<B>,incoming_peer:sc_network::PeerId,hash: B::Hash){
		match self.pending_messages.get(& hash) {
			Some(msgWithTopic) =>{
				self.do_send_message(network,incoming_peer,PPMsg::PushData(hash,*msgWithTopic))
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
		message: PPMsg<B>,
	){

		network.write_notification(who.clone(), Some(message), message);
	}

	


	/// Get data of valid, incoming messages for a topic (but might have expired meanwhile)
	pub fn messages_for(&mut self, topic: Vec<u8>)
		-> mpsc::UnboundedReceiver<TopicNotification>
	{
		let (tx, rx) = mpsc::unbounded();

		self.live_message_sinks.entry(topic).or_default().push(tx);

		rx
	}

	fn streamout(&mut self,who:PeerId, topic:Vec<u8>, message:Vec<u8>){

		if let Entry::Occupied(mut entry) = self.live_message_sinks.entry(topic) {
			//这个entry是Vec类型的
			debug!(target: "gossip", "Pushing consensus message to sinks for {}.", topic);
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
		who: PeerId,
		messages: Vec<PPMsg<B>>,
	) {
		if !messages.is_empty() {
			trace!(target: "gossip", "Received {} messages from peer {}", messages.len(), who);
		}

		for message in messages {
			match Some(message) {
				Some(PPMsg::PushHash(data)) => self.response_push_hash(network,who,data),
				Some( PPMsg::PushData(topic,data)) =>self.streamout(who,topic,data),
				Some(PPMsg::PullData(data)) => self.respone_pull_data(network,who,data),
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

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use sp_runtime::testing::{H256, Block as RawBlock, ExtrinsicWrapper};
	use futures::executor::block_on_stream;

	use super::*;

	type Block = RawBlock<ExtrinsicWrapper<u64>>;

	macro_rules! push_msg {
		($consensus:expr, $topic:expr, $hash: expr, $m:expr) => {
			if $consensus.known_messages.put($hash, ()).is_none() {
				$consensus.messages.push(MessageEntry {
					message_hash: $hash,
					topic: $topic,
					message: ConsensusMessage { data: $m, engine_id: [0, 0, 0, 0]},
					sender: None,
				});
			}
		}
	}

	struct AllowAll;
	impl Validator<Block> for AllowAll {
		fn validate(
			&self,
			_context: &mut dyn ValidatorContext<Block>,
			_sender: &PeerId,
			_data: &[u8],
		) -> ValidationResult<H256> {
			ValidationResult::ProcessAndKeep(H256::default())
		}
	}

	#[test]
	fn collects_garbage() {
		struct AllowOne;
		impl Validator<Block> for AllowOne {
			fn validate(
				&self,
				_context: &mut dyn ValidatorContext<Block>,
				_sender: &PeerId,
				data: &[u8],
			) -> ValidationResult<H256> {
				if data[0] == 1 {
					ValidationResult::ProcessAndKeep(H256::default())
				} else {
					ValidationResult::Discard
				}
			}

			fn message_expired<'a>(&'a self) -> Box<dyn FnMut(H256, &[u8]) -> bool + 'a> {
				Box::new(move |_topic, data| data[0] != 1)
			}
		}

		let prev_hash = H256::random();
		let best_hash = H256::random();
		let mut consensus = ConsensusGossip::<Block>::new();
		let m1_hash = H256::random();
		let m2_hash = H256::random();
		let m1 = vec![1, 2, 3];
		let m2 = vec![4, 5, 6];

		push_msg!(consensus, prev_hash, m1_hash, m1);
		push_msg!(consensus, best_hash, m2_hash, m2);
		consensus.known_messages.put(m1_hash, ());
		consensus.known_messages.put(m2_hash, ());

		let test_engine_id = Default::default();
		consensus.register_validator_internal(test_engine_id, Arc::new(AllowAll));
		consensus.collect_garbage();
		assert_eq!(consensus.messages.len(), 2);
		assert_eq!(consensus.known_messages.len(), 2);

		consensus.register_validator_internal(test_engine_id, Arc::new(AllowOne));

		// m2 is expired
		consensus.collect_garbage();
		assert_eq!(consensus.messages.len(), 1);
		// known messages are only pruned based on size.
		assert_eq!(consensus.known_messages.len(), 2);
		assert!(consensus.known_messages.contains(&m2_hash));
	}

	#[test]
	fn message_stream_include_those_sent_before_asking_for_stream() {
		let mut consensus = TransPPEngine::<Block>::new();
		consensus.register_validator_internal([0, 0, 0, 0], Arc::new(AllowAll));

		let message = ConsensusMessage { data: vec![4, 5, 6], engine_id: [0, 0, 0, 0] };
		let topic = HashFor::<Block>::hash(&[1,2,3]);

		consensus.register_message(topic, message.clone());
		let mut stream = block_on_stream(consensus.messages_for([0, 0, 0, 0], topic));

		assert_eq!(stream.next(), Some(TopicNotification { message: message.data, sender: None }));
	}

	#[test]
	fn can_keep_multiple_messages_per_topic() {
		let mut consensus = ConsensusGossip::<Block>::new();

		let topic = [1; 32].into();
		let msg_a = ConsensusMessage { data: vec![1, 2, 3], engine_id: [0, 0, 0, 0] };
		let msg_b = ConsensusMessage { data: vec![4, 5, 6], engine_id: [0, 0, 0, 0] };

		consensus.register_message(topic, msg_a);
		consensus.register_message(topic, msg_b);

		assert_eq!(consensus.messages.len(), 2);
	}

	#[test]
	fn can_keep_multiple_subscribers_per_topic() {
		let mut consensus = ConsensusGossip::<Block>::new();
		consensus.register_validator_internal([0, 0, 0, 0], Arc::new(AllowAll));

		let data = vec![4, 5, 6];
		let message = ConsensusMessage { data: data.clone(), engine_id: [0, 0, 0, 0] };
		let topic = HashFor::<Block>::hash(&[1, 2, 3]);

		consensus.register_message(topic, message.clone());

		let mut stream1 = block_on_stream(consensus.messages_for([0, 0, 0, 0], topic));
		let mut stream2 = block_on_stream(consensus.messages_for([0, 0, 0, 0], topic));

		assert_eq!(stream1.next(), Some(TopicNotification { message: data.clone(), sender: None }));
		assert_eq!(stream2.next(), Some(TopicNotification { message: data, sender: None }));
	}

	#[test]
	fn topics_are_localized_to_engine_id() {
		let mut consensus = ConsensusGossip::<Block>::new();
		consensus.register_validator_internal([0, 0, 0, 0], Arc::new(AllowAll));

		let topic = [1; 32].into();
		let msg_a = ConsensusMessage { data: vec![1, 2, 3], engine_id: [0, 0, 0, 0] };
		let msg_b = ConsensusMessage { data: vec![4, 5, 6], engine_id: [0, 0, 0, 1] };

		consensus.register_message(topic, msg_a);
		consensus.register_message(topic, msg_b);

		let mut stream = block_on_stream(consensus.messages_for([0, 0, 0, 0], topic));

		assert_eq!(stream.next(), Some(TopicNotification { message: vec![1, 2, 3], sender: None }));

		let _ = consensus.live_message_sinks.remove(&([0, 0, 0, 0], topic));
		assert_eq!(stream.next(), None);
	}

	#[test]
	fn peer_is_removed_on_disconnect() {
		struct TestNetwork;
		impl Network<Block> for TestNetwork {
			fn event_stream(
				&self,
			) -> std::pin::Pin<Box<dyn futures::Stream<Item = crate::Event> + Send>> {
				unimplemented!("Not required in tests")
			}

			fn report_peer(&self, _: PeerId, _: crate::ReputationChange) {
				unimplemented!("Not required in tests")
			}

			fn disconnect_peer(&self, _: PeerId) {
				unimplemented!("Not required in tests")
			}

			fn write_notification(&self, _: PeerId, _: crate::ConsensusEngineId, _: Vec<u8>) {
				unimplemented!("Not required in tests")
			}

			fn register_notifications_protocol(
				&self,
				_: ConsensusEngineId,
				_: std::borrow::Cow<'static, [u8]>,
			) {
				unimplemented!("Not required in tests")
			}

			fn announce(&self, _: H256, _: Vec<u8>) {
				unimplemented!("Not required in tests")
			}
		}

		let mut consensus = ConsensusGossip::<Block>::new();
		consensus.register_validator_internal([0, 0, 0, 0], Arc::new(AllowAll));

		let mut network = TestNetwork;

		let peer_id = PeerId::random();
		consensus.new_peer(&mut network, peer_id.clone(), Roles::FULL);
		assert!(consensus.peers.contains_key(&peer_id));

		consensus.peer_disconnected(&mut network, peer_id.clone());
		assert!(!consensus.peers.contains_key(&peer_id));
	}
}
