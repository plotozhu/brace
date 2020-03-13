## 理解
此片的network网络指的是区块链网络，而不仅仅是传输网络
## NetworkWorker::new
1. 创建消息接收队列
2. 读取配置
3. 分析bootnode
4. 删除重复的bootnode
5. 初始化 保留的节点
6. 配置peerset
7. 分析本机的帐号和公私钥
8. 创建检查器？
9. 使用Protocol::new创建新的协议
10. 创建swarm，这个是libp2p的 swarm  
  new函数使用Params类型参数来创建，Params是如下的格式
```rust
pub struct Params<B: BlockT, H: ExHashT> {
	/// 角色 (全节点, 轻节点, ...).
	pub roles: Roles,
	/// 这个executor是如何生成后台任务的，如果是`None`，使用默认的
	pub executor: Option<Box<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send>>,
	/// 这个才是网络层的配置
	pub network_config: NetworkConfiguration,
	/// 处理链的客户端
	pub chain: Arc<dyn Client<B>>,
	///最终确定化的证明
	///这是一个`Some()`对象，如果有，将使用他来执行最终确定化
	pub finality_proof_provider: Option<Arc<dyn FinalityProofProvider<B>>>,
	/// 这个是用来确认什么样的区块需要执行最终确定化.
	////这是一个`Some()`对象，如果有，将使用他来决定是否需要最终确定化
	pub finality_proof_request_builder: Option<BoxFinalityProofRequestBuilder<B>>,

	/// `OnDemand` 对象作为一个从客户端获取区块数据的接收器
	/// 如果是`Some`对象,网络工作器将会处理这些请求并且返回给他们，一般来说是轻节点用的
	pub on_demand: Option<Arc<OnDemand<B>>>,

	/// 交易池，网络工作器将从从对象读取交易向网络中广播
	pub transaction_pool: Arc<dyn TransactionPool<H, B>>,

	/// 在连接上使用的协议的名字，每个链都应该不是样
	pub protocol_id: ProtocolId,

	/// Import queue to use.
	///
	/// The import queue is the component that verifies that blocks received from other nodes are
	/// valid.
	pub import_queue: Box<dyn ImportQueue<B>>,

	/// Type to check incoming block announcements.
	pub block_announce_validator: Box<dyn BlockAnnounceValidator<B> + Send>,

	/// Registry for recording prometheus metrics to.
	pub metrics_registry: Option<Registry>,
}

```
## network::src::protocol::generic_proto::behaviour.rs
这是通用的协议处理，以下是代码内文档的翻译  
Network behaviour 处理与其他节点连接的自定义协议的子流

### 传统与新协议
`GenericProto`有以下的行为方式
- 一旦建立一个连接，我们打开一个子流（在源代码里称为`legacy protocol`）。这个子流的名称由初化时传入的`protocol_id`和`version`决定。如果远端拒绝此子流，我们就关闭这个连接
- 对于每一个已经注册的协议，我们同样需要打开为此协议打开一个子流，即使对方拒绝此子流，同样是可以的（忽略，并不需要关闭连接）。
- 当我们需要发送消息时，我们可以调用`send_packet`来强制向该子流发送，或者使用`write_notification`来提示一个已经注册的协议（子流），如果对端拒绝或是不支持该协议，就使用传统的方式（`send_packet`）发送。

### 如何工作
`GenericProto`角色用来同步以下几个组件
- libp2p的swarm：打开新连接，通知连接断开
- 连接句柄（查阅`handler.rs`)：处理独立的连接
- 连接集管理器（PSM）：请求连接或断开节点
- 扩展API： 给需要了解已经建立连端的外部客户代码

每个连接句柄可以处于4种状态： 使能+打开、使能+关闭、禁用+打开以及禁用+关闭，使能/禁用必须与PSM同步，例如，如果PSM要求断开连接，我们需要在此禁用此连接。打开/关闭需要与扩展API同步。  
然而连接句柄只有当我们实际与某个节点连接后才会存在。这意味着对于每个节点，我们可能有六个状态，断开、拨号中、使能+打开、使能+关闭、禁用+打开以及禁用+关闭，需要特别注意的是拨号中状态必须与PSM中的“连接已建立”对应。换句主说，PSM并不区分节点是正在拨号中还是已经连接。  
还有，存在一个“禁止”系统，如果我们拨号某个节点失败，我们将"禁止“该节点几秒钟。如果PSM在此期间内请求拨号该节点，我们将等到”禁止“时间失效后才会执行，但是PSM仍旧认为已经建立连接了。  
注册这个”禁止“是不完全的，如果一个被”禁止“的节点主动拨号连接我们，我们将接受此连接。即”禁止“系统仅仅是禁止拨出。

## 对象`GenericProto`
`GenericProto::new`需要三个参数，协议名称，协议版本和PSM
`GenericProto::register_notif_protocol`可以用来注册协议的通知，就是向`notif_protocol`数组里添加了(proto_name,engine和handshake_msg)的三元组