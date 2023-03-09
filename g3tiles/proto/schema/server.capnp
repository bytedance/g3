@0xa37d35b77bba8fa9;

struct ServerStats {
  online @0 :Bool;
  aliveTaskCount @1 :Int32;
  totalConnCount @2 :UInt64;
  totalTaskCount @3 :UInt64;
}

interface ServerControl {
  status @0 () -> (status :ServerStats);
}
