
use sc_network::{PeerId};
use std::time::{Duration, SystemTime};
use lru::LruCache;

const MaxRouteItemCount 10240
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct routePathItem{
    next:sc_network::PeerId,
    path:Vec<routePath>
}
/// item for 
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct routePath {
    minTtl:u8,
    next:sc_network::PeerId,
    path:Vec<routePath>
}
/// items with bls signature
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct routeItemWithSign {

    //创建时间
    createTime:SystemTime,
    //下一跳列表
    nextHops:Vec<routePath>,
    //总签名
    signature:Vec<u8>,
}


pub struct routeTable{
    routeItems:LruCache<sc_network::PeerId,routeItemWithSign>,

}
impl  routeTable {
    pub fn new() -> self{
        routeTable{
            routeItems:LruCache::new(MaxRouteItemCount)
        }
    }
    pub fn add(newItem routeItemWithSign){

    }
    pub fn get(peerId: sc_network::PeerId){

    }
}