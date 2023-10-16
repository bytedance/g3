@0xa51475273bd1dfb5;

using Types = import "types.capnp";

using Server = import "server.capnp";

interface ProcControl {
  #

  version @0 () -> (version :Text);
  offline @1 () -> (result :Types.OperationResult);

  listServer @2 () -> (result :List(Text));
  getServer @3 (name: Text) -> (server :Types.FetchResult(Server.ServerControl));

  publishKey @4 (pem: Text) -> (result :Types.OperationResult);
  listKeys @5 () -> (result :List(Data));
  checkKey @7 (ski: Data) -> (result: Types.OperationResult);

  addMetricsTag @6 (name :Text, value :Text) -> (result :Types.OperationResult);
}
