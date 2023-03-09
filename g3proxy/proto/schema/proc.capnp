@0xb13a8d4b53f4d79a;

using Types = import "types.capnp";

using UserGroup = import "user_group.capnp";
using Resolver = import "resolver.capnp";
using Escaper = import "escaper.capnp";
using Server = import "server.capnp";

interface ProcControl {
  #

  version @0 () -> (version :Text);
  offline @1 () -> (result :Types.OperationResult);

  reloadUserGroup @2 (name :Text) -> (result :Types.OperationResult);
  reloadResolver @3 (name :Text) -> (result :Types.OperationResult);
  reloadAuditor @16 (name :Text) -> (result: Types.OperationResult);
  reloadEscaper @4 (name :Text) -> (result :Types.OperationResult);
  reloadServer @5 (name :Text) -> (result :Types.OperationResult);

  getUserGroup @6 (name: Text) -> (user_group :Types.FetchResult(UserGroup.UserGroupControl));
  getResolver @7 (name: Text) -> (resolver :Types.FetchResult(Resolver.ResolverControl));
  getEscaper @8 (name: Text) -> (escaper :Types.FetchResult(Escaper.EscaperControl));
  getServer @9 (name: Text) -> (server :Types.FetchResult(Server.ServerControl));

  listUserGroup @10 () -> (result :List(Text));
  listResolver @11 () -> (result :List(Text));
  listAuditor @17 () -> (result :List(Text));
  listEscaper @12 () -> (result :List(Text));
  listServer @13 () -> (result :List(Text));

  getTimeOffset @14 () -> (offset :Types.UtcOffset);
  setTimeOffset @15 (offset :Types.UtcOffset) -> (result :Types.OperationResult);

  forceQuitOfflineServers @18 () -> (result :Types.OperationResult);
  forceQuitOfflineServer @19 (name :Text) -> (result :Types.OperationResult);
}
