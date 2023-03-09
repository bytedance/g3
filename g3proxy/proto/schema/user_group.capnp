@0x9045d8bef5c9e129;

interface UserGroupControl {
  listStaticUser @0 () -> (result :List(Text));
  listDynamicUser @1 () -> (result :List(Text));
}
