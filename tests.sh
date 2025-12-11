#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "=== Testing ver-shim-rs ==="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}PASS${NC}: $1"
}

fail() {
    echo -e "${RED}FAIL${NC}: $1"
    exit 1
}

# Clean up before tests (workspace shares target directory)
echo "Cleaning up..."
cargo clean 2>/dev/null || true
echo

# Test 1: Build objcopy example (debug)
echo "--- Test: Build objcopy example (debug) ---"
(cd ver-shim-example-objcopy && cargo build 2>&1)
pass "objcopy example builds in debug mode"
echo

# Test 2: Unpatched binary should show "(not set)" and not panic
echo "--- Test: Unpatched binary shows '(not set)' ---"
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1)
if echo "$OUTPUT" | grep -q "(not set)"; then
    pass "unpatched binary shows '(not set)'"
else
    fail "unpatched binary should show '(not set)'"
fi
echo

# Test 3: Patch binary with objcopy (debug)
echo "--- Test: Patch binary with objcopy (debug) ---"
(cd ver-shim-example-objcopy && \
    cargo objcopy --bin ver-shim-example-objcopy -- \
    --update-section .ver_shim_data=target/ver_shim_data \
    target/debug/ver-shim-example-objcopy.bin 2>&1)
pass "objcopy patching works in debug mode"
echo

# Test 4: Patched binary should show git info
echo "--- Test: Patched binary shows git info ---"
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "patched binary shows git sha"
else
    fail "patched binary should show git sha"
fi
if echo "$OUTPUT" | grep -q "build timestamp:" && ! echo "$OUTPUT" | grep -q "build timestamp:.*not set"; then
    pass "patched binary shows build timestamp"
else
    fail "patched binary should show build timestamp"
fi
echo

# Test 5: Build objcopy example (release)
echo "--- Test: Build objcopy example (release) ---"
(cd ver-shim-example-objcopy && \
    cargo objcopy --release --bin ver-shim-example-objcopy -- \
    --update-section .ver_shim_data=target/ver_shim_data \
    target/release/ver-shim-example-objcopy.bin 2>&1)
OUTPUT=$(./ver-shim-example-objcopy/target/release/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "objcopy example works in release mode"
else
    fail "objcopy example should work in release mode"
fi
echo

# Test 6: Build nightly example (ver-shim-example-build)
echo "--- Test: Build nightly example (ver-shim-example-build) ---"
(cd ver-shim-example-build && cargo +nightly build 2>&1)
OUTPUT=$(./ver-shim-example-build/target/debug/ver-shim-example.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "nightly example builds and works"
else
    fail "nightly example should build and work"
fi
echo

# Test 7: VER_SHIM_BUFFER_SIZE=1024 should work and trigger rebuild
# We already have a build from test 5 (release) - now build with different buffer size
echo "--- Test: VER_SHIM_BUFFER_SIZE=1024 triggers rebuild ---"
BUILD_OUTPUT=$(cd ver-shim-example-objcopy && VER_SHIM_BUFFER_SIZE=1024 cargo build 2>&1)
if echo "$BUILD_OUTPUT" | grep -q "Compiling ver-shim"; then
    pass "VER_SHIM_BUFFER_SIZE=1024 triggers rebuild"
else
    fail "VER_SHIM_BUFFER_SIZE should trigger rebuild"
fi
echo

# Test 8: VER_SHIM_BUFFER_SIZE=65535 should work
echo "--- Test: VER_SHIM_BUFFER_SIZE=65535 (max u16) works ---"
if (cd ver-shim-example-objcopy && VER_SHIM_BUFFER_SIZE=65535 cargo build 2>&1); then
    pass "VER_SHIM_BUFFER_SIZE=65535 works"
else
    fail "VER_SHIM_BUFFER_SIZE=65535 should work"
fi
echo

# Test 9: VER_SHIM_BUFFER_SIZE=65536 should fail
echo "--- Test: VER_SHIM_BUFFER_SIZE=65536 (overflow) fails ---"
if (cd ver-shim-example-objcopy && VER_SHIM_BUFFER_SIZE=65536 cargo build 2>&1); then
    fail "VER_SHIM_BUFFER_SIZE=65536 should fail"
else
    pass "VER_SHIM_BUFFER_SIZE=65536 correctly fails"
fi
echo

# Test 10: VER_SHIM_BUFFER_SIZE=32 (too small) should fail
echo "--- Test: VER_SHIM_BUFFER_SIZE=32 (too small) fails ---"
if (cd ver-shim-example-objcopy && VER_SHIM_BUFFER_SIZE=32 cargo build 2>&1); then
    fail "VER_SHIM_BUFFER_SIZE=32 should fail (must be > 32)"
else
    pass "VER_SHIM_BUFFER_SIZE=32 correctly fails"
fi
echo

# Build a baseline before VER_SHIM_BUILD_TIME tests (test 10 left things in a failed state)
echo "--- Building baseline for VER_SHIM_BUILD_TIME tests ---"
(cd ver-shim-example-objcopy && cargo build 2>&1)
echo

# Test 11: VER_SHIM_BUILD_TIME with unix timestamp
# Note: We don't use cargo clean here - rerun-if-env-changed should trigger rebuild
echo "--- Test: VER_SHIM_BUILD_TIME with unix timestamp ---"
(cd ver-shim-example-objcopy && \
    VER_SHIM_BUILD_TIME=1700000000 cargo objcopy --bin ver-shim-example-objcopy -- \
    --update-section .ver_shim_data=target/ver_shim_data \
    target/debug/ver-shim-example-objcopy.bin 2>&1)
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "build timestamp: 2023-11-14"; then
    pass "VER_SHIM_BUILD_TIME unix timestamp works (2023-11-14)"
else
    fail "VER_SHIM_BUILD_TIME unix timestamp should produce 2023-11-14"
fi
echo

# Test 12: VER_SHIM_BUILD_TIME with RFC 3339
echo "--- Test: VER_SHIM_BUILD_TIME with RFC 3339 ---"
(cd ver-shim-example-objcopy && \
    VER_SHIM_BUILD_TIME="2024-06-15T12:30:00Z" cargo objcopy --bin ver-shim-example-objcopy -- \
    --update-section .ver_shim_data=target/ver_shim_data \
    target/debug/ver-shim-example-objcopy.bin 2>&1)
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "build timestamp: 2024-06-15"; then
    pass "VER_SHIM_BUILD_TIME RFC 3339 works (2024-06-15)"
else
    fail "VER_SHIM_BUILD_TIME RFC 3339 should produce 2024-06-15"
fi
echo

# Test 13: VER_SHIM_BUILD_TIME with invalid value should fail
echo "--- Test: VER_SHIM_BUILD_TIME with invalid value fails ---"
if (cd ver-shim-example-objcopy && VER_SHIM_BUILD_TIME="not-a-timestamp" cargo build 2>&1); then
    fail "VER_SHIM_BUILD_TIME with invalid value should fail"
else
    pass "VER_SHIM_BUILD_TIME with invalid value correctly fails"
fi
echo

echo -e "${GREEN}=== All tests passed ===${NC}"
