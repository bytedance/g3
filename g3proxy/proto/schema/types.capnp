@0x9cf25b8f324f3eef;

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

struct UtcOffset {
  hours @0 :Int8 = 0;
  minutes @1 :Int8 = 0;
  seconds @2 :Int8 = 0;
}
