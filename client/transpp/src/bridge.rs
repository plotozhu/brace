// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

use crate::{Network, Validator};
use crate::state_machine::{TransPPEngine, TopicNotification, PERIODIC_MAINTENANCE_INTERVAL,MessageWithTopic,PPMsg,PPHandleID,MessageWithHash};

use codec::{Encode, Decode};

use sc_network::{Event, ReputationChange};

use futures::{prelude::*, channel::mpsc};
use libp2p::PeerId;
use sp_runtime::{traits::Block as BlockT, ConsensusEngineId};
use std::{borrow::Cow, pin::Pin, sync::Arc, task::{Context, Poll}};

/// Wraps around an implementation of the `Network` crate and provides gossiping capabilities on
/// top of it.
pub struct GossipEngine<B: BlockT> {
	state_machine: TransPPEngine<B>,
	network: Box<dyn Network<B> + Send>,
	periodic_maintenance_interval: futures_timer::Delay,
	network_event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
	engine_id: ConsensusEngineId,
}

impl<B: BlockT> Unpin for GossipEngine<B> {}

impl<B: BlockT> GossipEngine<B> {
	/// Create a new instance.
	pub fn new<N: Network<B> + Send + Clone + 'static>(
		mut network: N,
		engine_id: ConsensusEngineId,
		protocol_name: impl Into<Cow<'static, [u8]>>,
		validator: Arc<dyn Validator<B>>,
	) -> Self where B: 'static {
		let mut state_machine = TransPPEngine::new();

		// We grab the event stream before registering the notifications protocol, otherwise we
		// might miss events.
		let network_event_stream = network.event_stream();

		network.register_notifications_protocol(engine_id, protocol_name.into());
	
		GossipEngine {
			state_machine,
			network: Box::new(network),
			periodic_maintenance_interval: futures_timer::Delay::new(PERIODIC_MAINTENANCE_INTERVAL),
			network_event_stream,
			engine_id,
		}
	}

	pub fn report(&self, who: PeerId, reputation: ReputationChange) {
		self.network.report_peer(who, reputation);
	}


	/// Get data of valid, incoming messages for a topic (but might have expired meanwhile).
	pub fn messages_for(&mut self, topic: Vec<u8>)
		-> mpsc::UnboundedReceiver<TopicNotification>
	{
		self.state_machine.messages_for( topic)
	}




	/// Send addressed message to the given peers. The message is not kept or multicast
	/// later on.
	pub fn send_message(&mut self, who: Vec<sc_network::PeerId>, topic: Vec<u8>,data: Vec<u8>) {
		self.state_machine.send_message(&mut *self.network, who, MessageWithTopic {
			topic:topic.clone(),
			message: data.clone(),
		});
	}

	/// Notify everyone we're connected to that we have the given block.
	///
	/// Note: this method isn't strictly related to gossiping and should eventually be moved
	/// somewhere else.
	pub fn announce(&self, block: B::Hash, associated_data: Vec<u8>) {
		self.network.announce(block, associated_data);
	}
}

impl<B: BlockT> Future for GossipEngine<B> {
	type Output = ();

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
		let this = &mut *self;

		while let Poll::Ready(Some(event)) = this.network_event_stream.poll_next_unpin(cx) {
			match event {
				Event::NotificationStreamOpened { remote, engine_id: msg_engine_id, roles } => {
					if msg_engine_id != this.engine_id {
						continue;
					}
					this.state_machine.new_peer(&mut *this.network, remote, roles);
				}
				Event::NotificationStreamClosed { remote, engine_id: msg_engine_id } => {
					if msg_engine_id != this.engine_id {
						continue;
					}
					this.state_machine.peer_disconnected(&mut *this.network, remote);
				},
				Event::NotificationsReceived { remote, messages } => {
					this.state_machine.on_incoming(
						&mut *this.network,
						remote,
						messages.into_iter()
							.filter_map(|(handle, data)| {
								let pp_handle = handle[0] as PPHandleID;
								match pp_handle {
									PUSH_HASH =>{
										let hash = B::Hash::decode(&mut &data[..]).unwrap();
										let msg = PPMsg::PushHash(hash);
										Some(msg)
										
									},
									PULLL_DATA=>{
										let hash = B::Hash::decode(&mut &data[..]).unwrap();
										let msg = PPMsg::PullData(hash);
										Some(msg)
									},
									PUSH_DATA=>{
										
										let msg = MessageWithHash::<B>::decode(&mut &data[..]).unwrap();
								
										Some(PPMsg::PushData(msg.hash,msg.msg.clone()))
									}
								}
							}
							)
							.collect()
					);
				},
				Event::Dht(_) => {}
			}
		}

		while let Poll::Ready(()) = this.periodic_maintenance_interval.poll_unpin(cx) {
			this.periodic_maintenance_interval.reset(PERIODIC_MAINTENANCE_INTERVAL);
		
		}

		Poll::Pending
	}
}
