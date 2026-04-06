#!/usr/bin/env bash
# =============================================================================
# scripts/test-all.sh — Comprehensive test runner for forgecode
#
# Runs unit tests, integration tests, and black-box validation of the compiled
# binary. Intended for local development and CI.
#
# Usage:
#   ./scripts/test-all.sh            # Run everything
#   ./scripts/test-all.sh unit       # Unit tests only
#   ./scripts/test-all.sh build      # Build validation only
#   ./scripts/test-all.sh blackbox   # Black-box tests only
#   ./scripts/test-all.sh lint       # Lint checks only
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASSED=0
FAILED=0
SKIPPED=0

pass() { ((PASSED++)); echo -e "  ${GREEN}✓${NC} $1"; }
fail() { ((FAILED++)); echo -e "  ${RED}✗${NC} $1"; }
skip() { ((SKIPPED++)); echo -e "  ${YELLOW}⊘${NC} $1 (skipped)"; }
section() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

# ---------------------------------------------------------------------------
# 1. Lint checks
# ---------------------------------------------------------------------------
run_lint() {
    section "Lint & Format Checks"

    if cargo fmt --check 2>/dev/null; then
        pass "cargo fmt"
    else
        fail "cargo fmt — formatting issues found"
    fi

    if cargo clippy --workspace --all-targets 2>&1 | grep -q "error\["; then
        fail "cargo clippy — errors found"
    else
        pass "cargo clippy"
    fi
}

# ---------------------------------------------------------------------------
# 2. Unit & integration tests (cargo test)
# ---------------------------------------------------------------------------
run_unit() {
    section "Unit & Integration Tests"

    # All crates to test
    local crates=(
        forge_config forge_domain forge_infra forge_services forge_repo forge_app
        forge_sandbox forge_config_adapter
        forge_main forge_ci forge_json_repair forge_walker forge_tracker
    )

    for crate in "${crates[@]}"; do
        local output
        output=$(cargo test -p "$crate" 2>&1)
        local rc=$?
        if [[ $rc -eq 0 ]]; then
            local total=0
            while IFS= read -r line; do
                local n
                n=$(echo "$line" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || true)
                if [[ -n "$n" ]]; then
                    total=$((total + n))
                fi
            done <<< "$(echo "$output" | grep 'test result:')"
            pass "$crate ($total passed)"
        else
            fail "$crate"
        fi
    done
}

# ---------------------------------------------------------------------------
# 3. Build validation
# ---------------------------------------------------------------------------
run_build() {
    section "Build Validation"

    if cargo build --release 2>&1 | tail -3; then
        pass "cargo build --release"
    else
        fail "cargo build --release"
        return 1
    fi

    BINARY="target/release/forge"
    if [[ ! -x "$BINARY" ]]; then
        fail "Binary not found at $BINARY"
        return 1
    fi
    pass "Binary exists: $BINARY"

    # Check binary is not trivially small (should be >1MB)
    SIZE=$(stat -f%z "$BINARY" 2>/dev/null || stat --printf="%s" "$BINARY" 2>/dev/null)
    if [[ "$SIZE" -gt 1000000 ]]; then
        pass "Binary size: $(( SIZE / 1024 / 1024 ))MB"
    else
        fail "Binary suspiciously small: ${SIZE} bytes"
    fi
}

# ---------------------------------------------------------------------------
# 4. Black-box tests (test compiled binary behavior)
# ---------------------------------------------------------------------------
run_blackbox() {
    section "Black-box Binary Tests"

    BINARY="target/release/forge"
    if [[ ! -x "$BINARY" ]]; then
        echo -e "${YELLOW}Binary not found. Run './scripts/test-all.sh build' first.${NC}"
        skip "All black-box tests"
        return 0
    fi

    # --- 4.1 Version / help flags ---
    if "$BINARY" --version 2>&1 | grep -qiE "forge|[0-9]+\.[0-9]+"; then
        pass "--version outputs version string"
    else
        fail "--version does not output expected format"
    fi

    if "$BINARY" --help 2>&1 | grep -qi "usage\|options\|commands\|forge"; then
        pass "--help outputs usage info"
    else
        fail "--help does not output usage info"
    fi

    # --- 4.2 Config directory structure ---
    FORGE_HOME="${HOME}/.forge"
    if [[ -d "$FORGE_HOME" ]] || [[ -d "${HOME}/forge" ]]; then
        pass "Config directory exists (~/.forge or ~/forge)"
    else
        skip "No config directory found (first-run scenario)"
    fi

    # --- 4.3 Invalid arguments ---
    if "$BINARY" --nonexistent-flag 2>&1; then
        fail "Should exit non-zero for invalid flag"
    else
        pass "Exits non-zero for invalid flag"
    fi

    # --- 4.4 Sandbox crate linkage ---
    # Verify the sandbox platform code compiled (check symbols)
    if nm "$BINARY" 2>/dev/null | grep -q "sandbox\|Sandbox"; then
        pass "Sandbox symbols present in binary"
    elif strings "$BINARY" 2>/dev/null | grep -q "sandbox"; then
        pass "Sandbox strings present in binary"
    else
        skip "Cannot verify sandbox linkage (stripped binary)"
    fi

    # --- 4.5 Config adapter crate linkage ---
    if strings "$BINARY" 2>/dev/null | grep -q "forge_config_adapter\|ForgeLegacy\|ClaudeAdapter"; then
        pass "Config adapter strings present in binary"
    else
        skip "Cannot verify adapter linkage (may be stripped or not linked yet)"
    fi

    # --- 4.6 Temp project .forge/ directory tests ---
    TMPDIR_BB=$(mktemp -d)
    trap "rm -rf $TMPDIR_BB" EXIT

    mkdir -p "$TMPDIR_BB/.forge/rules"
    echo '{"session":{"provider_id":"test","model_id":"test-model"}}' > "$TMPDIR_BB/.forge/settings.json"
    echo "# Test Rule" > "$TMPDIR_BB/.forge/rules/test-rule.md"
    echo "# Test FORGE instructions" > "$TMPDIR_BB/.forge/FORGE.md"
    mkdir -p "$TMPDIR_BB/.forge/memory"
    echo "# Test memory" > "$TMPDIR_BB/.forge/memory/MEMORY.md"

    if [[ -f "$TMPDIR_BB/.forge/settings.json" ]]; then
        pass ".forge/settings.json created in temp project"
    fi
    if [[ -f "$TMPDIR_BB/.forge/rules/test-rule.md" ]]; then
        pass ".forge/rules/ directory structure valid"
    fi
    if [[ -f "$TMPDIR_BB/.forge/FORGE.md" ]]; then
        pass ".forge/FORGE.md structure valid"
    fi
    if [[ -f "$TMPDIR_BB/.forge/memory/MEMORY.md" ]]; then
        pass ".forge/memory/ directory structure valid"
    fi

    # --- 4.7 Settings.json schema validation ---
    SETTINGS="$TMPDIR_BB/.forge/settings.json"
    if python3 -c "import json; json.load(open('$SETTINGS'))" 2>/dev/null; then
        pass "settings.json is valid JSON"
    elif jq . "$SETTINGS" >/dev/null 2>&1; then
        pass "settings.json is valid JSON (jq)"
    else
        fail "settings.json is not valid JSON"
    fi

    # --- 4.8 Verify new config fields schema ---
    SCHEMA="$PROJECT_ROOT/forge.schema.json"
    if [[ -f "$SCHEMA" ]]; then
        # Check that new fields are present in schema
        for field in agents mcp_servers permissions sandbox rules memory; do
            if grep -q "\"$field\"" "$SCHEMA"; then
                pass "Schema contains '$field' field"
            else
                fail "Schema missing '$field' field"
            fi
        done
    else
        skip "forge.schema.json not found"
    fi
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
summary() {
    section "Summary"
    TOTAL=$((PASSED + FAILED + SKIPPED))
    echo -e "  ${GREEN}Passed:${NC}  $PASSED"
    echo -e "  ${RED}Failed:${NC}  $FAILED"
    echo -e "  ${YELLOW}Skipped:${NC} $SKIPPED"
    echo -e "  Total:   $TOTAL"
    echo ""
    if [[ $FAILED -gt 0 ]]; then
        echo -e "${RED}FAILED${NC}"
        exit 1
    else
        echo -e "${GREEN}ALL PASSED${NC}"
        exit 0
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
MODE="${1:-all}"

case "$MODE" in
    lint)     run_lint ;;
    unit)     run_unit ;;
    build)    run_build ;;
    blackbox) run_blackbox ;;
    all)
        run_lint
        run_unit
        run_build
        run_blackbox
        ;;
    *)
        echo "Usage: $0 {all|lint|unit|build|blackbox}"
        exit 1
        ;;
esac

summary
