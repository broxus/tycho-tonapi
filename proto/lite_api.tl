int ? = Int;
long ? = Long;
double ? = Double;
string ? = String;
object ? = Object;
function ? = Function;
bytes data:string = Bytes;
true = True;
boolTrue = Bool;
boolFalse = Bool;


vector {t:Type} # [ t ] = Vector t;

int128 4*[ int ] = Int128;
int256 8*[ int ] = Int256;

tcp.pong random_id:long = tcp.Pong;

tonNode.blockId workchain:int shard:long seqno:int = tonNode.BlockId;
tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256 = tonNode.BlockIdExt;
tonNode.zeroStateIdExt workchain:int root_hash:int256 file_hash:int256 = tonNode.ZeroStateIdExt;

adnl.message.query query_id:int256 query:bytes = adnl.Message;
adnl.message.answer query_id:int256 answer:bytes = adnl.Message;

liteServer.error code:int message:string = liteServer.Error;

liteServer.accountId workchain:int id:int256 = liteServer.AccountId;
liteServer.libraryEntry hash:int256 data:bytes = liteServer.LibraryEntry;

liteServer.masterchainInfo last:tonNode.blockIdExt state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfo;
liteServer.masterchainInfoExt mode:# version:int capabilities:long last:tonNode.blockIdExt last_utime:int now:int state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfoExt;
liteServer.currentTime now:int = liteServer.CurrentTime;
liteServer.version mode:# version:int capabilities:long now:int = liteServer.Version;
liteServer.blockData id:tonNode.blockIdExt data:bytes = liteServer.BlockData;
liteServer.blockHeader id:tonNode.blockIdExt mode:# header_proof:bytes = liteServer.BlockHeader;
liteServer.accountState id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes proof:bytes state:bytes = liteServer.AccountState;
liteServer.shardInfo id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes shard_descr:bytes = liteServer.ShardInfo;
liteServer.allShardsInfo id:tonNode.blockIdExt proof:bytes data:bytes = liteServer.AllShardsInfo;
liteServer.libraryResult result:(vector liteServer.libraryEntry) = liteServer.LibraryResult;
liteServer.shardBlockLink id:tonNode.blockIdExt proof:bytes = liteServer.ShardBlockLink;


---functions---

tcp.ping random_id:long = tcp.Pong;

liteServer.getMasterchainInfo = liteServer.MasterchainInfo;
liteServer.getMasterchainInfoExt mode:# = liteServer.MasterchainInfoExt;
liteServer.getTime = liteServer.CurrentTime;
liteServer.getVersion = liteServer.Version;
liteServer.getBlock id:tonNode.blockIdExt = liteServer.BlockData;
liteServer.getAccountState id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
liteServer.getShardInfo id:tonNode.blockIdExt workchain:int shard:long exact:Bool = liteServer.ShardInfo;
liteServer.getAllShardsInfo id:tonNode.blockIdExt = liteServer.AllShardsInfo;
liteServer.lookupBlock mode:# id:tonNode.blockId lt:mode.1?long utime:mode.2?int = liteServer.BlockHeader;
liteServer.getLibraries library_list:(vector int256) = liteServer.LibraryResult;

liteServer.queryPrefix = Object;
liteServer.query data:bytes = Object;
liteServer.waitMasterchainSeqno seqno:int timeout_ms:int = Object; // query prefix
