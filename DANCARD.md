# DanCard trong VectorCraft (IN Vĩnh Phát)

Fork `Phanloc31012002/vectorcraft-vinhphat` của VectorCraft (Apache-2.0 / MIT), thêm các lệnh và
panel của Công cụ bình. Mọi code DanCard nằm trong các file riêng để kéo bản mới từ upstream ít bị
xung đột.

## Đã có

| Lệnh / panel | Làm gì |
|---|---|
| `dancard.transform` | Đọc khung transform của vùng chọn và từng object: góc xoay, rộng × cao theo khung, tâm, 4 góc, vị trí so với artboard. |
| Panel **Công cụ bình** (Window › Công cụ bình) | Hiện số đo của `dancard.transform` cho thứ đang chọn, tự cập nhật khi chọn object khác. |
| `dancard.rename` | Đổi tên hàng loạt, cùng quy tắc `dcRename` (tiền tố, số/chữ, đệm 0, trên/dưới, lặp nhãn). |
| `dancard.clipToSize` | Clip theo kích thước đúng tâm phần nhìn thấy; `followAngle` cắt theo khung xoay. |

### Cách `dancard.transform` đo

- Đo **phần nhìn thấy**: clip group lấy theo mask (giao với nội dung), group lồng bao nhiêu tầng cũng
  đi hết. Không phải dò clip cha/con, group cha/con trong tool nữa.
- Bỏ qua object ẩn, guide, path không tô.
- **Góc xoay**:
  - `transform`: object được xoay ngay trong VectorCraft, nên lấy góc VectorCraft lưu (chính xác tuyệt đối).
  - `detected`: file .ai/PDF/SVG không lưu góc, nên tự dò từ hình dạng. Lấy khung chữ nhật nhỏ nhất ôm
    phần nhìn thấy. Chỉ nhận góc khi hình có cạnh thẳng nằm sát khung (từ 80% chu vi trở lên): card,
    tem, khung clip. Dòng chữ hay nhóm object rời rạc thì giữ khung thẳng.
  - `page`: khung thẳng.
- Góc dò được đưa về khoảng (-45°, 45°]. Card xoay đúng 90° sẽ đọc là khung thẳng 5,6 × 9,2.

### Gọi lệnh

```sh
# Chạy không giao diện (thư mục cài có sẵn vectorcraft-cli.exe)
vectorcraft-cli run --in "file.ai" --cmd select.all --cmd dancard.transform

# Qua cổng điều khiển của app đang mở: vectorcraft --control 7979
{"id":1,"method":"engine.execute","params":{"command":"dancard.transform","params":{"unit":"cm"}}}
```

## Build bản Windows

GitHub Actions › **DanCard Windows x64** › Run workflow. Workflow cũng tự chạy khi push lên nhánh
`dancard`. Kết quả là artifact chứa `vectorcraft-<ver>-windows-x64-portable.zip` và file `.msi`.
Hai file này chưa ký số, nên Windows SmartScreen có thể cảnh báo khi mở.

## Lấy bản mới từ upstream

```sh
git remote add upstream https://github.com/storytold/vectorcraft
git fetch upstream && git merge upstream/main
```

## Giấy phép

Giữ nguyên `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`. Logo ArtCraft là nhãn hiệu: không dùng cho
bản phát cho nhân viên.
