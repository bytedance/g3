RUSTFLAGS="-C instrument-coverage"
LLVM_PROFILE_FILE="${TEST_NAME}-%p-%m.profraw"
PROF_DATA_FILE="${TEST_NAME}.profdata"

clear_profiles()
{
	find . -type f -regex ".*/${TEST_NAME}.*\.profraw" -exec rm \{\} \;
	[ ! -f "${PROF_DATA_FILE}" ] || rm "${PROF_DATA_FILE}"
}

cd "${PROJECT_DIR}"

clear_profiles
trap clear_profiles EXIT

export RUSTFLAGS LLVM_PROFILE_FILE
