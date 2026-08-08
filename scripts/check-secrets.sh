#!/usr/bin/env bash
# Fail if denylisted paths are tracked/staged, or secret-like content is present.
# Safe for CI, pre-commit, humans, harnesses, and AI coding agents.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DENY_PATHS="${ROOT}/security/commit-denylist.txt"
DENY_CONTENT="${ROOT}/security/content-deny-patterns.txt"
MODE="${1:-all}" # all | staged | tracked

RED=$'\033[31m'
GRN=$'\033[32m'
YLW=$'\033[33m'
RST=$'\033[0m'

fail=0

log_err() { printf '%s%s%s\n' "$RED" "$*" "$RST" >&2; }
log_ok()  { printf '%s%s%s\n' "$GRN" "$*" "$RST"; }
log_info(){ printf '%s%s%s\n' "$YLW" "$*" "$RST"; }

# ---- collect file list ----
list_files() {
  case "$MODE" in
    staged)
      git diff --cached --name-only --diff-filter=ACM
      ;;
    tracked)
      git ls-files
      ;;
    all)
      # Tracked + staged + untracked (respects gitignore)
      {
        git ls-files
        git diff --cached --name-only --diff-filter=ACM
        git ls-files --others --exclude-standard
      } | sort -u
      ;;
    *)
      log_err "usage: $0 [all|staged|tracked]"
      exit 2
      ;;
  esac
}

# ---- path denylist ----
# Convert simple globs in denylist to match tracked paths.
path_denied() {
  local path="$1"
  local pattern
  while IFS= read -r pattern || [[ -n "$pattern" ]]; do
    [[ -z "$pattern" || "$pattern" =~ ^[[:space:]]*# ]] && continue
    # strip leading ./ 
    pattern="${pattern#./}"
    # Use bash pattern matching; translate ** to *
    local pat
    pat="$(printf '%s' "$pattern" | sed 's/\*\*/\*/g')"
    # shellcheck disable=SC2254
    case "$path" in
      $pat) return 0 ;;
    esac
    # also match if path ends with denylisted basename for **/foo
    if [[ "$pattern" == */* ]]; then
      case "$path" in
        */${pattern##*/}) 
          # only if pattern was **/name style
          if [[ "$pattern" == \*\*/?* && "$path" == */"${pattern#**/}" ]]; then
            return 0
          fi
          ;;
      esac
    fi
  done < "$DENY_PATHS"
  return 1
}

# Stronger explicit checks (always)
explicit_forbidden_tracked() {
  local f
  for f in \
    infra/cdk.context.json \
    .env \
    .aws/credentials \
    credentials
  do
    if git ls-files --error-unmatch "$f" >/dev/null 2>&1; then
      log_err "FORBIDDEN tracked file: $f"
      fail=1
    fi
  done

  # Any cdk.context.json anywhere in the index
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    log_err "FORBIDDEN tracked file: $f"
    fail=1
  done < <(git ls-files | grep -E '(^|/)cdk\.context\.json$' || true)

  # cdk.out artifacts
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    log_err "FORBIDDEN tracked path under cdk.out: $f"
    fail=1
  done < <(git ls-files | grep -E '(^|/)cdk\.out(/|$)' || true)
}

# Staged-only explicit block (pre-commit)
explicit_forbidden_staged() {
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    case "$f" in
      */cdk.context.json|cdk.context.json|infra/cdk.context.json)
        log_err "BLOCKED staged file: $f (AWS account/zone binding — never commit)"
        fail=1
        ;;
      .env|.env.*|*/.env|*/.env.*)
        if [[ "$f" != *.example ]]; then
          log_err "BLOCKED staged env file: $f"
          fail=1
        fi
        ;;
      *.pem|*.p12|*.pfx|*credentials*|*/credentials)
        log_err "BLOCKED staged credential-like file: $f"
        fail=1
        ;;
      web/pkg/*|infra/cdk.out/*)
        log_err "BLOCKED staged build artifact: $f"
        fail=1
        ;;
    esac
  done < <(git diff --cached --name-only --diff-filter=ACM)
}

# ---- content scan ----
scan_content() {
  local files=()
  while IFS= read -r f; do
    [[ -z "$f" || ! -f "$f" ]] && continue
    # skip binaries / locks
    case "$f" in
      Cargo.lock|*/Cargo.lock|package-lock.json|*/package-lock.json) continue ;;
      *.wasm|*.png|*.jpg|*.jpeg|*.gif|*.ico|*.pdf|*.zip) continue ;;
    esac
    # only text-ish
    if file -b --mime-encoding "$f" 2>/dev/null | grep -qi binary; then
      continue
    fi
    files+=("$f")
  done < <(list_files)

  if [[ ${#files[@]} -eq 0 ]]; then
    return 0
  fi

  local pattern
  while IFS= read -r pattern || [[ -n "$pattern" ]]; do
    [[ -z "$pattern" || "$pattern" =~ ^[[:space:]]*# ]] && continue
    # grep staged/tracked files for pattern
    if grep -nIE -- "$pattern" "${files[@]}" 2>/dev/null; then
      log_err "BLOCKED content matching pattern: $pattern"
      fail=1
    fi
  done < "$DENY_CONTENT"
}

# ---- never allow force-add circumvention reminder for AI ----
check_assume_unchanged_context() {
  # Local cdk.context.json is fine; it must remain untracked / ignored.
  if [[ -f infra/cdk.context.json ]]; then
    if git ls-files --error-unmatch infra/cdk.context.json >/dev/null 2>&1; then
      log_err "infra/cdk.context.json is tracked — remove with: git rm --cached infra/cdk.context.json"
      fail=1
    fi
  fi
}

log_info "check-secrets mode=${MODE}"

explicit_forbidden_tracked
if [[ "$MODE" == "staged" || "$MODE" == "all" ]]; then
  explicit_forbidden_staged
fi
check_assume_unchanged_context
scan_content

if [[ "$fail" -ne 0 ]]; then
  log_err ""
  log_err "Secret/sensitive-path guard FAILED."
  log_err "Do NOT commit AWS account bindings, credentials, or cdk.context.json."
  log_err "See security/commit-denylist.txt and docs/SECURITY_GUARDS.md"
  exit 1
fi

log_ok "check-secrets: OK (no forbidden paths/content)"
exit 0
