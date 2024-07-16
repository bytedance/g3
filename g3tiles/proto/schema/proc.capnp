@0x9b282abca7d080f1;

using Types = import "types.capnp";

using Server = import "server.capnp";

interface ProcControl {
  #

  version @0 () -> (version :Text);
  offline @1 () -> (result :Types.OperationResult);
  cancelShutdown @11 () -> (result :Types.OperationResult);
  releaseController @12 () -> (result :Types.OperationResult);

  reloadServer @2 (name :Text) -> (result :Types.OperationResult);
  getServer @3 (name: Text) -> (server :Types.FetchResult(Server.ServerControl));
  listServer @4 () -> (result :List(Text));

  forceQuitOfflineServers @5 () -> (result :Types.OperationResult);
  forceQuitOfflineServer @6 (name :Text) -> (result :Types.OperationResult);

  reloadDiscover @7 (name :Text) -> (result :Types.OperationResult);
  listDiscover @8 () -> (result :List(Text));

  reloadBackend @9 (name :Text) -> (result :Types.OperationResult);
  listBackend @10 () -> (result :List(Text));
}
