Code Coverage
-----

Source based code coverage is available since rust version 1.60.

# Compilation

The following RUSTFLAGS should be set before compiling:

Shell:
```shell
export RUSTFLAGS="-C instrument-coverage"
```

Fish:
```fish
set -x RUSTFLAGS "-C instrument-coverage"
```

Before running tests, the 
[LLVM_PROFILE_FILE](https://clang.llvm.org/docs/SourceBasedCodeCoverage.html#running-the-instrumented-program)
can be used to set name of the generated profraw files:

Shell:
```shell
export LLVM_PROFILE_FILE "test-%p-%m.profraw"
```

Fish:
```fish
set -x LLVM_PROFILE_FILE "test-%p-%m.profraw"
```

# Parse and Report

LLVM coverage tools are needed to process coverage data and generate reports.

## Independent llvm-tools

[llvm-profdata](https://llvm.org/docs/CommandGuide/llvm-profdata.html)
is needed to merge all raw profile data files into indexed profile data file:

Shell:
```shell
llvm-profdata merge -o a.profdata $(find . -type f -name "*profraw" -exec ls \{\} \;)
```

Fish:
```fish
llvm-profdata merge -o a.profdata (find . -type f -name "*profraw" -exec ls \{\} \;)
```

[llvm-cov](https://llvm.org/docs/CommandGuide/llvm-cov.html) is needed to generate reports:

```shell
llvm-cov report --instr-profile=a.profdata --ignore-filename-regex=".cargo" --ignore-filename-regex="rustc" -object <BIN/OBJ>[ -object <BIN/OBJ>]...
```

## Bundled llvm-tools-preview

The llvm-tools you installed may be not compatible with the ones rustc used. You can use llvm-tools-preview via rustup:

```shell
rustup component add llvm-tools-preview
cargo install cargo-binutils
```

To run the bundled tools, just use ```cargo <cmd> -- <params>```.
