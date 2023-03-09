@0xd317f85459da5d44;

enum QueryStrategy {
  ipv4First @0;
  ipv6First @1;
  ipv4Only @2;
  ipv6Only @3;
}

struct QueryResult {
  union {
    ip @0 :List(Text);
    err @1 :Text;
  }
}

interface ResolverControl {
  query @0 (domain :Text, strategy :QueryStrategy, resolutionDelay :UInt16 = 50) -> (result :QueryResult);
}
