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
| `dancard.resize` | Resize từng object thành đúng Rộng × Cao theo khung transform của nó (card nghiêng co giãn theo trục card), giữ tâm. |
| `dancard.rasterize` | Nút **Raster** đầu panel: CMYK, 500 ppi, nền trong suốt, khử răng cưa Art, không lề, không thêm clipping mask. Ảnh đúng vùng nhìn thấy (clip group: phần trong mask; chọn object con trong clip group thì raster cả clip group). Card đặt nghiêng: ảnh nghiêng theo khung, không dư góc. Ảnh phủ kín lưu **TIFF CMYK giữ đúng mực** (chữ K 100 vẫn chỉ mực K); có chỗ trong suốt thì lưu PNG có kênh trong suốt. Tài liệu RGB: báo đổi sang CMYK. |
| Object › Rasterize… | Hộp thoại đầy đủ: Color Model, Resolution (Screen/Medium/High/Use Document Raster Effects Resolution/Other), Background, Anti-aliasing (None/Art Optimized/Type Optimized), Create Clipping Mask, Add … Around Object, thêm ô DanCard *Theo khung transform* (`followAngle`). Tài liệu CMYK: ảnh phủ kín lưu TIFF CMYK. |
| `dancard.layout` + tab **Dàn card** | Dàn card như `dcDan`: 9.2×5.6 (20 chỗ, nhận card đôi 18.4×5.6), 9×5.5 (20), 9.2×5.2 (21), 8.7×5.5 (22), 10.2×15.2 (6), voucher 15.2×7.2 (8, ghép thêm 3 card 9.2×5.6). 1 cột = in 1 mặt, 2 cột = mỗi hàng [trước][sau]. Mỗi mẫu clip đúng khổ (clip group có sẵn thì giữ), Expand Appearance, nhân bản vào các ô; mặt trước AB1, mặt sau AB2 đối xứng. Artboard + pon lấy từ file pon chung (tự đọc `dan_card_config.txt` của DanCard trên Illustrator nếu có, hoặc bấm Chọn…). Sai khổ thì tự sửa như DanCard (raster theo nền đúng khổ), không được thì báo lỗi và không đổi gì. Cardoffset thêm ghi chú. Cả lần dàn là một bước hoàn tác. |
| `dancard.cutMarks` + tab **Dấu cắt** | Dấu cắt 4 góc cho các bài đang chọn (group đã dàn có từ 2 clip group: mỗi clip group một bài), cùng quy tắc `dcThemDauCatTuDong`: nét ra vùng trống, sát mép artboard thì lật vào trong, không đè bài khác, không vẽ trùng khi bấm lại; layer "Dau cat tu dong", nét K 100 % 1 pt. |
| Panel: tab **Đổi tên** | Giao diện của `dancard.rename` (tiền tố, kiểu đánh, bắt đầu, đệm số 0, lặp nhãn, hướng đếm). |

## Mặc định riêng của bản này

App đổi các tùy chọn sau một lần khi mở (Preferences vẫn đổi lại được; Reset trả về đúng mặc định này):

| Tùy chọn | Ở đâu trong Preferences | Mặc định |
|---|---|---|
| Đơn vị General | Units › General | Centimeters (Stroke và Type giữ Points) |
| Mọi file mở ra đều dùng đơn vị General | Units | Bật: file .ai/PDF/SVG/… mở ra đều đo bằng cm |
| Giữ Ctrl: tạm dùng Direct Selection với mọi tool | Selection & Anchor Display › Selection | Bật: kể cả đang cầm Selection tool; nhả Ctrl là về tool cũ |
| Hiện điểm neo khi chọn bằng Selection tool | Selection & Anchor Display › Anchor Points… | Tắt: Selection và Group Selection chỉ hiện đường viền; Direct Selection (hay giữ Ctrl) vẫn hiện điểm neo |

VectorCraft gốc không đổi gì: ba tùy chọn mới mặc định tắt trong `Prefs::default()`; chỉ app desktop áp
`Prefs::apply_dancard_defaults` khi khởi động (`prefs_dialog::restore`), ghi phiên bản đã áp vào `dancardDefaults`.

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
