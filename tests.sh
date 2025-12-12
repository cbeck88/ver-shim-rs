#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "=== Testing ver-shim-rs ==="
echo
echo "Environment:"
cargo --version
rustc --version
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

# Clean up before tests (examples are excluded from workspace, have their own targets)
echo "Cleaning up..."
cargo clean 2>/dev/null || true
rm -rf ver-shim-example-objcopy/target ver-shim-example-build/target 2>/dev/null || true
echo

# Build the ver-shim CLI tool first
echo "--- Building ver-shim CLI tool ---"
cargo build -p ver-shim-build --features cli 2>&1
VER_SHIM="$(pwd)/target/debug/ver-shim"
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

# Test 3: Patch binary with ver-shim patch (debug)
echo "--- Test: Patch binary with ver-shim patch (debug) ---"
$VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
pass "ver-shim patch works in debug mode"
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

# Test 5: Build and patch objcopy example (release)
echo "--- Test: Build and patch objcopy example (release) ---"
(cd ver-shim-example-objcopy && cargo build --release 2>&1)
$VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/release/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/release/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "objcopy example works in release mode"
else
    fail "objcopy example should work in release mode"
fi
echo

# Test 6: Alternative approach - ver-shim -o + cargo objcopy
echo "--- Test: Alternative approach (ver-shim -o + cargo objcopy) ---"
# Generate section data file
$VER_SHIM --all-git --all-build-time -o ver-shim-example-objcopy/target/ver_shim_data 2>&1
# Use cargo objcopy to patch
cargo objcopy --manifest-path ver-shim-example-objcopy/Cargo.toml \
    --bin ver-shim-example-objcopy -- \
    --update-section .ver_shim_data=ver-shim-example-objcopy/target/ver_shim_data \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy-alt.bin 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy-alt.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "alternative approach (ver-shim -o + cargo objcopy) works"
else
    fail "alternative approach should work"
fi
echo

# Test 7: Build nightly example (ver-shim-example-build)
echo "--- Test: Build nightly example (ver-shim-example-build) ---"
(cd ver-shim-example-build && cargo +nightly build 2>&1)
OUTPUT=$(./ver-shim-example-build/target/debug/ver-shim-example.bin 2>&1)
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "nightly example builds and works"
else
    fail "nightly example should build and work"
fi
echo

# Test 8: VER_SHIM_BUFFER_SIZE=1024 should work
echo "--- Test: VER_SHIM_BUFFER_SIZE=1024 works ---"
if (cd ver-shim-example-objcopy && VER_SHIM_BUFFER_SIZE=1024 cargo build 2>&1); then
    pass "VER_SHIM_BUFFER_SIZE=1024 works"
else
    fail "VER_SHIM_BUFFER_SIZE=1024 should work"
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
echo "--- Test: VER_SHIM_BUILD_TIME with unix timestamp ---"
VER_SHIM_BUILD_TIME=1700000000 $VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "build timestamp: 2023-11-14"; then
    pass "VER_SHIM_BUILD_TIME unix timestamp works (2023-11-14)"
else
    fail "VER_SHIM_BUILD_TIME unix timestamp should produce 2023-11-14"
fi
echo

# Test 12: VER_SHIM_BUILD_TIME with RFC 3339
echo "--- Test: VER_SHIM_BUILD_TIME with RFC 3339 ---"
VER_SHIM_BUILD_TIME="2024-06-15T12:30:00Z" $VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -q "build timestamp: 2024-06-15"; then
    pass "VER_SHIM_BUILD_TIME RFC 3339 works (2024-06-15)"
else
    fail "VER_SHIM_BUILD_TIME RFC 3339 should produce 2024-06-15"
fi
echo

# Test 13: VER_SHIM_BUILD_TIME with invalid value should fail
echo "--- Test: VER_SHIM_BUILD_TIME with invalid value fails ---"
if VER_SHIM_BUILD_TIME="not-a-timestamp" $VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1; then
    fail "VER_SHIM_BUILD_TIME with invalid value should fail"
else
    pass "VER_SHIM_BUILD_TIME with invalid value correctly fails"
fi
echo

# Test 14: VER_SHIM_IDEMPOTENT skips build time
echo "--- Test: VER_SHIM_IDEMPOTENT skips build time ---"
VER_SHIM_IDEMPOTENT=1 $VER_SHIM --all-git --all-build-time patch \
    ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -qE "build timestamp:\s+\(not set\)" && echo "$OUTPUT" | grep -qE "build date:\s+\(not set\)"; then
    pass "VER_SHIM_IDEMPOTENT skips build timestamp and date"
else
    fail "VER_SHIM_IDEMPOTENT should skip build timestamp/date, got: $OUTPUT"
fi
# Verify git info is still included
if echo "$OUTPUT" | grep -qE "git sha:\s+[0-9a-f]+"; then
    pass "VER_SHIM_IDEMPOTENT still includes git info"
else
    fail "VER_SHIM_IDEMPOTENT should still include git info, got: $OUTPUT"
fi
echo

# Test 15: Patching updates git info without rebuild
echo "--- Test: Patching updates git info without rebuild ---"

# Get current branch for later comparison
ORIGINAL_BRANCH=$(git rev-parse --abbrev-ref HEAD)

# Clean build first, and preemptively delete test branch in case previous run failed
git branch -D testtesttesttest 2>/dev/null || true
(cd ver-shim-example-objcopy && cargo clean 2>/dev/null || true)
(cd ver-shim-example-objcopy && cargo build 2>&1)
$VER_SHIM --all-git patch ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
# Use regex to match "git branch:" followed by whitespace and branch name
if echo "$OUTPUT" | grep -qE "git branch:\s+$ORIGINAL_BRANCH"; then
    pass "initial build shows branch '$ORIGINAL_BRANCH'"
else
    fail "initial build should show branch '$ORIGINAL_BRANCH', got: $OUTPUT"
fi

# Create and checkout a new branch
git checkout -b testtesttesttest 2>&1

# Build and patch again - should not recompile
BUILD_OUTPUT=$(cd ver-shim-example-objcopy && cargo build 2>&1)
echo "$BUILD_OUTPUT"
if echo "$BUILD_OUTPUT" | grep -q "Compiling"; then
    fail "switching branches should not trigger recompilation"
else
    pass "no recompilation after branch switch"
fi

$VER_SHIM --all-git patch ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
if echo "$OUTPUT" | grep -qE "git branch:\s+testtesttesttest"; then
    pass "patched binary shows new branch 'testtesttesttest'"
else
    fail "patched binary should show branch 'testtesttesttest', got: $OUTPUT"
fi

# Go back to previous HEAD using reflog syntax (puts us in detached HEAD state)
git checkout HEAD@{1} 2>&1

# Build and patch again - should not recompile
BUILD_OUTPUT=$(cd ver-shim-example-objcopy && cargo build 2>&1)
echo "$BUILD_OUTPUT"
if echo "$BUILD_OUTPUT" | grep -q "Compiling"; then
    fail "checkout via reflog should not trigger recompilation"
else
    pass "no recompilation after reflog checkout"
fi

$VER_SHIM --all-git patch ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy 2>&1
OUTPUT=$(./ver-shim-example-objcopy/target/debug/ver-shim-example-objcopy.bin 2>&1)
# In detached HEAD state, git rev-parse --abbrev-ref HEAD returns "HEAD"
if echo "$OUTPUT" | grep -qE "git branch:\s+HEAD"; then
    pass "patched binary shows detached HEAD after reflog checkout"
else
    fail "patched binary should show 'HEAD' (detached), got: $OUTPUT"
fi

# Return to original branch and clean up test branch
git checkout "$ORIGINAL_BRANCH" 2>&1
git branch -D testtesttesttest 2>&1
pass "test branch cleaned up"
echo

echo -e "${GREEN}=== All tests passed ===${NC}"
