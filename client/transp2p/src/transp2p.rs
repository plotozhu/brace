

use sc_network::{PeerId,Transpp,discovery};
use std::time::{Duration, SystemTime};
use lru::LruCache;
use crate::routetab::{routePathItem,routeTable};
use super::Transpp;
use crate::{Network, Validator};
const maxPendingReq:u16 = 128
///路由发现协议的回应
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct FindRoute{
    createTime:SystemTime, //此路由的请求时间 
    dest:PeerId,      //目标地址，此处为H
    alpha:u8,     //扩散值 
    path:Vec<PeerId>,   //源路径为空
    orgTTL:u8,      //初始TTL值，
    sign:Vec<u8>,      // B的签名
}

///路由发现协议的回应
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct FindRouteResp{
        createTime:SystemTime,
        route:Vec<routePathItem>,
        sign:Vec<u8>,
    
}

pub struct TransP2P{
    pending_req:LruCache<PeerId,Vec<PeerId>>,  //findroute 请求
    table:routeTable, //路由表
    network: Box<dyn Network<B> + Send>,

} 
/**
 * 第一个版本中，签名先不实现
 */
impl TransP2P {
    pub fn new(&mut transpp:Transpp, &mut )->self{
        TransP2P{
            pending_req:LruCache::new(maxPendingReq),
            table:routeTable::new(),
        }
    }

    pub findRoute(dest PeerId,alpha u8,ttl u8) {

    }
}