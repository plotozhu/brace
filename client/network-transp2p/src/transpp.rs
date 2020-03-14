// Copyright 2019-2020 Minicoin Fund Technologies (UK) Ltd.
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

//!
//! Transp2p提供基础的邻节点数据传输的方法，其基本方法为先传输数据哈希，邻节点如果收到哈希推送的哈希后，本地没有，就向源节点请求一次读取哈希对应的数据
//!   

use crate::{Network, MessageIntent, Validator, ValidatorContext, ValidationResult};

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::sync::Arc;
use std::iter;
use std::time;
use log::{trace, debug};
use futures::channel::mpsc;
use lru::LruCache;
use libp2p::PeerId;
use sp_runtime::traits::{Block as BlockT, Hash, HashFor};
use sp_runtime::ConsensusEngineId;
pub use sc_network::message::generic::{Message, ConsensusMessage};
use sc_network::config::Roles;
use wasm_timer::Instant;

// FIXME: Add additional spam/DoS attack protection: https://github.com/paritytech/substrate/issues/1115
const KNOWN_MESSAGES_CACHE_SIZE: usize = 4096;

pub struct transpp<B: BlockT>  {
    received_hashes: LruCache<B::Hash, ()>,
}