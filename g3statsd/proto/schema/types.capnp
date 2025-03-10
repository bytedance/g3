@0xfe51079061e90943;

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
