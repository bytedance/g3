@0xbc46b8175881f106;

using Types = import "types.capnp";

interface ProcControl {
  #

  version @0 () -> (version :Text);
  offline @1 () -> (result :Types.OperationResult);
  cancelShutdown @2 () -> (result :Types.OperationResult);
  releaseController @3 () -> (result :Types.OperationResult);

  reloadImporter @4 (name :Text) -> (result :Types.OperationResult);
  listImporter @5 () -> (result :List(Text));

  reloadCollector @6 (name :Text) -> (result :Types.OperationResult);
  listCollector @7 () -> (result :List(Text));

  reloadExporter @8 (name :Text) -> (result :Types.OperationResult);
  listExporter @9 () -> (result :List(Text));
}
