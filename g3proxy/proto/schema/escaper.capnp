@0xcd40f64a57dc11cf;

using Types = import "types.capnp";

interface EscaperControl {
  publish @0 (data :Text) -> (result :Types.OperationResult);
}
