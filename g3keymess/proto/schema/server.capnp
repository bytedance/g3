@0xdf328b7e02123e83;

struct ServerStats {
  online @0 :Bool;
  aliveTaskCount @1 :Int32;
  totalTaskCount @2 :UInt64;
  aliveRequestCount @3: Int32;
  totalRequestCount @4: UInt64;
  passedRequestCount @5: UInt64;
}

interface ServerControl {
  status @0 () -> (status :ServerStats);
}
