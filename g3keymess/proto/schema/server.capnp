@0xdf328b7e02123e83;

using Types = import "types.capnp";

struct ServerStats {
  online @0 :Bool;
  aliveTaskCount @1 :Int32;
  totalTaskCount @2 :UInt64;
}

interface ServerControl {
  status @0 () -> (status :ServerStats);

  addMetricsTag @1 (name :Text, value :Text) -> (result :Types.OperationResult);
}
