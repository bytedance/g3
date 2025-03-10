@0xbc46b8175881f106;

using Types = import "types.capnp";

interface ProcControl {
  #

  version @0 () -> (version :Text);
  offline @1 () -> (result :Types.OperationResult);
  cancelShutdown @2 () -> (result :Types.OperationResult);
  releaseController @3 () -> (result :Types.OperationResult);

  reloadInput @4 (name :Text) -> (result :Types.OperationResult);
  listInput @5 () -> (result :List(Text));
}
