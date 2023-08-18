@0x9045d8bef5c9e129;

using Types = import "types.capnp";

interface UserGroupControl {
  listStaticUser @0 () -> (result :List(Text));
  listDynamicUser @1 () -> (result :List(Text));
  publishDynamicUser @2 (contents :Text) -> (result :Types.OperationResult);
}
