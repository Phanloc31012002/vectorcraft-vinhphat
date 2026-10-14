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

- **Khớp bảng Transform**: rộng, cao, góc và tâm X/Y theo thước bằng đúng số bảng Transform hiện,
  với điểm tham chiếu ở tâm. Clip group đo theo khung mask, group lồng bao nhiêu tầng cũng đúng,
  không phải dò clip cha/con hay group cha/con trong tool nữa. Use Preview Bounds bật thì đo cả viền.
- **Góc xoay**:
  - `transform`: object được xoay ngay trong VectorCraft, nên lấy đúng góc VectorCraft lưu.
  - `detected`: bảng Transform cho khung thẳng, nhưng hình thật là chữ nhật đặt nghiêng (file .ai/PDF
    không lưu góc). Khi đó trả khung chữ nhật nhỏ nhất ôm hình, kèm `transformBox` là số bảng Transform
    hiện để đối chiếu. Chỉ nhận góc khi cạnh thẳng của hình phủ ít nhất 80% chu vi khung (card, tem,
    khung clip). Dòng chữ hay nhóm object rời rạc thì giữ khung thẳng.
  - `page`: khung thẳng, bằng đúng bảng Transform.
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
