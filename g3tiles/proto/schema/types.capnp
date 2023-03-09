@0x921969fb6e11b80a;

struct Error {
  code @0 :Int32 = -1;
  reason @1 :Text;
}

struct OperationResult {
  union {
    ok @0 :Text;
    err @1 :Error;
  }
}

struct FetchResult(Data) {
  union {
    data @0 :Data;
    err @1 :Error;
  }
}
