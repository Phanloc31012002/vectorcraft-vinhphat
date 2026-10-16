#!/usr/bin/env bash
# DanCard (IN Vĩnh Phát): gộp bản mới của VectorCraft (storytold/vectorcraft) vào nhánh hiện tại
# (`dancard`), rồi kiểm tra các lệnh DanCard. Dùng chung cho workflow "DanCard đồng bộ VectorCraft"
# và cho phiên Claude chạy tay. Xem mục "Cập nhật khi VectorCraft ra bản mới" trong DANCARD.md.
#
#   bash .github/dancard/sync.sh            # gộp + kiểm tra đủ
#   CHECK=none bash .github/dancard/sync.sh # chỉ gộp (kiểm tra sau)
#
# Kết quả (dòng `status=` cũng ghi vào $GITHUB_OUTPUT khi chạy trên Actions):
#   status=uptodate  đã có bản mới nhất, không làm gì
#   status=merged    đã gộp, kiểm tra qua (commit gộp nằm trên nhánh hiện tại, chưa push)
#   status=conflict  hai bên sửa cùng chỗ: đã huỷ gộp, in danh sách file, thoát mã 2
#   status=failed    gộp được nhưng build/test hỏng, thoát mã 3 (chạy tay thì commit gộp còn trên
#                    máy để sửa tiếp; trên Actions thì không đẩy gì), hoặc git merge hỏng vì lý do
#                    khác như file chưa theo dõi sẽ bị ghi đè, thoát mã 4
set -euo pipefail

UPSTREAM_URL=${UPSTREAM_URL:-https://github.com/storytold/vectorcraft}
UPSTREAM_BRANCH=${UPSTREAM_BRANCH:-main}
CHECK=${CHECK:-full}
out() {
    echo "$1"
    if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "$1" >>"$GITHUB_OUTPUT"; fi
}
summary() {
    if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$1" >>"$GITHUB_STEP_SUMMARY"; fi
}

cd "$(git rev-parse --show-toplevel)"
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Có thay đổi chưa commit: commit hoặc cất đi rồi chạy lại." >&2
    exit 1
fi
git remote get-url upstream >/dev/null 2>&1 || git remote add upstream "$UPSTREAM_URL"
git fetch --no-tags upstream "$UPSTREAM_BRANCH"
target="upstream/$UPSTREAM_BRANCH"
behind=$(git rev-list --count "HEAD..$target")
if [ "$behind" = 0 ]; then
    summary "Đã có bản mới nhất của VectorCraft."
    out status=uptodate
    exit 0
fi
version=$(git show "$target:Cargo.toml" | sed -n 's/^version = "\([^"]*\)".*/\1/p' | head -n 1)
echo "VectorCraft có $behind commit mới (bản ${version:-?})."
message="Gộp VectorCraft ${version:-mới} (storytold/vectorcraft $UPSTREAM_BRANCH) vào dancard"
if ! git merge --no-edit -m "$message" "$target"; then
    files=$(git diff --name-only --diff-filter=U)
    # Hỏng vì lý do khác (vd. file chưa theo dõi sẽ bị ghi đè) thì không có gì để huỷ.
    git merge --abort 2>/dev/null || true
    if [ -z "$files" ]; then
        summary "### Không gộp được VectorCraft ${version:-mới}: git merge báo lỗi (xem log của bước này)."
        out status=failed
        exit 4
    fi
    echo "Đụng nhau với code DanCard ở các file:"
    echo "$files" | sed 's/^/  /'
    summary "### Không gộp tự động được VectorCraft ${version:-mới}"
    summary "Các file đụng nhau (danh sách chỗ DanCard sửa file gốc nằm trong DANCARD.md):"
    summary "$(echo "$files" | sed 's/^/- /')"
    out status=conflict
    exit 2
fi
summary "Đã gộp VectorCraft ${version:-mới} ($behind commit)."

if [ "$CHECK" = none ]; then
    out status=merged
    exit 0
fi
fail() {
    summary "Kiểm tra hỏng ở bước: $1"
    out status=failed
    exit 3
}
cargo fmt --all -- --check || fail "cargo fmt"
cargo test -p vectorcraft-engine --lib -- dancard tests_prefs || fail "test DanCard (engine)"
cargo test -p vectorcraft-ui-egui --lib -- dancard i18n:: canvas:: dialogs:: prefs || fail "test DanCard (giao diện)"
cargo test -p vectorcraft --bin vectorcraft -- dancard_update single_instance || fail "test DanCard (app: tự cập nhật, một cửa sổ)"
cargo test -p vectorcraft-engine --test command_sweep || fail "quét mọi lệnh với tham số rác"
# Clippy: build hỏng là lỗi; cảnh báo chỉ tính ở file của DanCard (cảnh báo mới do clippy bản mới
# trong code gốc VectorCraft thì không chặn việc đồng bộ).
if ! clippy_out=$(cargo clippy -p vectorcraft-engine -p vectorcraft-ui-egui -p vectorcraft --all-targets --message-format=short 2>&1); then
    echo "$clippy_out" | tail -n 40
    fail "cargo clippy"
fi
if echo "$clippy_out" | grep -E '(dancard[^ :]*|single_instance)\.rs:[0-9]+:[0-9]+: (warning|error)'; then
    fail "cargo clippy (cảnh báo trong code DanCard)"
fi
cargo run -q -p xtask -- layers || fail "phân tầng crate (xtask layers)"
summary "Kiểm tra DanCard: qua."
out status=merged
