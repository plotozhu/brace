
use sc_network::{PeerId};
use std::time::{Duration, SystemTime};
use lru::LruCache;

const MaxRouteItemCount 10240
/// item for on instance
#[derive(Debug, Eq, PartialEq,Clone,Encode,Decode)]
pub struct routeItem {
    minTtl:u8,
    peerId:sc_network::PeerId,
    next:Vec<routeItem>,
}
/// items with bls signature
pub struct routeItemWithSign {

    //创建时间
    createTime:SystemTime,
    //下一跳列表
    nextHops:Vec<routeItem>,
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