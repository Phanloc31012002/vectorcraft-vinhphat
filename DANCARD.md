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
| `dancard.ktsLayout` + **Dàn tối ưu › Dàn KTS** | Như `dcDanToiUu` v22: dàn tối ưu không PON, lề 3 mm, khe 0, con sát nhau (hàng/cột thường rồi con xoay 90°, chẻ guillotine 2 tầng, canh giữa). Dàn 2 mặt: chọn theo hàng trái = trước, phải = sau; mặt sau đối xứng ngang. Nhiều mẫu vào một tờ: nhiều khổ thì con lớn trước, con nhỏ lấp chỗ trống, mẫu đã dàn bị xoá khỏi nguồn. Số vị trí không chia đều cho số mẫu thì panel hỏi **Dừng lại / Tiếp tục**. Panel hiện "n con / tờ". |
| `dancard.dieLayout` + **Dàn tối ưu › Dàn bế** | Như Dàn bế v2.16.20 (`dan_be_nester.js` viết lại bằng Rust): chọn từng cặp Khuôn (trái) → Bài (phải), 2 mặt thì bộ ba Khuôn → trước → sau; không chọn gì thì lấy theo layer Khuôn bế / Bài. Xếp theo đường bế thật (xoay 0/90/180/270°, lưới 0,25 mm), khe giữa đường bế, lề, né PON; vẽ 4 chấm PON tròn 5 mm, layer PON / Khuôn / Bài. Ngân sách tìm kiếm 3 giây như bản gốc. |
| `dancard.tuTro` + **Dàn tối ưu › Dàn Offset** | Như `dcDanTuTro` v11: tự trở, nửa trái mặt trước, nửa phải đối xứng qua tim giấy, in xong lật ngang tờ. Cùng khổ: tự thử xoay, tự bóp tối đa "Bóp tối đa" mm khi nhờ vậy dàn thêm được con; khổ lẫn lộn: mỗi cặp một chỗ. Lặp 1 cặp cho đầy tờ. PON giấy 4 góc. Cặp đã dàn bị xoá, cặp chưa vừa giữ lại. |
| `dancard.learnTemplate` / `dancard.applyTemplate` + tab **Dàn theo mẫu** | Học mẫu 1 mặt hoặc 2 mặt (mặt sau đối xứng) từ bố cục dàn tay: vị trí, khung, góc 0/90/180/270° so với con chuẩn (đọc từ ma trận ảnh/chữ, không được thì so ảnh). Cần chọn con chuẩn thì panel hiện nút **Xác nhận con chuẩn**. Áp mẫu: mỗi nguồn raster 500 ppi, xoay và co đúng khung từng vị trí, mỗi lô một artboard đúng khổ file pon (nhớ file pon mặt trước/mặt sau). |
| `dancard.saveTemplatePdf` | Lưu PDF đã dàn: từng artboard 1 PDF hoặc mặt trước + mặt sau 1 PDF, tên "file 1 - <nội dung>.pdf", không ghi đè file có sẵn. |
| `dancard.copyNotes` | Copy ghi chú đang chọn sang các artboard sau (bài 2 mặt: artboard 1, 3, 5…; 1 mặt: mọi artboard), cùng khoảng cách tới góc artboard, thêm tiền tố F1, F2… nếu nhập. |
| `dancard.saddleStitch` + tab **CTL Offset › Đóng ghim giữa** | Như `dcRunSignature8`: số trang chia hết cho 4; có bìa thì 4 trang ngoài thành tờ tự trở 4 trang; ruột tự trở 4/8 (A5 thêm 16) rồi các tờ AB 16 trang (A5 32). Tờ giấy cố định (85,8 × 63,8, 85,8 × 62,5, 64,8 × 41,8, 42,8 × 31,3 cm), bóp ruột 1 mm mỗi tờ, PON giấy và PON cắt trên hai layer khóa, ghi chú RUỘT N ở mặt A. Bìa A4 cần rộng ≤ 20,9 cm như bản gốc. |
| `dancard.perfectBind` + tab **CTL Offset › Keo gáy** | Như `dcRunKeoGay`: tuần tự, không bóp, không khe; A4 16 trang / tờ AB, A5 32 trang (2 tay 16), phần dư tự trở TT16/TT8/TT4. |
| `dancard.saveCtlPackage` | Nút **Lưu AI + JPG + ZIP** theo từng khổ vừa dàn: mỗi tờ một file .ai (nội dung PDF, Illustrator mở được), một JPG CMYK 300 ppi (cả mặt A + B) và một ZIP nén deflate chứa hai file đó; không ghi đè. |
| `dancard.catalogue` + tab **Catalogue** | Như `dcRunCatalogueAuto`: tờ PON 43 × 32,5 cm, ưu tiên 2 cuốn / tờ, xoay cuốn khi cần, ghép cặp đóng gáy giữa, ruột bóp 0,25 mm mỗi tờ, PON chữ L trên layer khóa, ghi chú bìa/ruột. |
| `dancard.autoSavePdf` + tab **Auto Save** | Như `dcAutoSavePDF`: mỗi con (mỗi số hộp) một PDF, 1 mặt 1 trang, 2 mặt 2 trang, trang đúng khung clip; tên `<mã> - <STT> - <số hộp> hop[ - ghi chú][ - 1 mat] - <cán màng> - <ngày>.pdf`; không sửa tài liệu, không ghi đè. |
| `dancard.clipCard` | Nút Clip 9,2 × 5,6 tự xoay theo object (`dcClipCard926`). |
| `dancard.importVariables` + tab **Variable** | Nhập file CSV/TXT thành Variables: mỗi cột một biến (cột `#` là biến ẩn/hiện: true/on hiện, false/off/trống ẩn, chữ khác hiện), mỗi dòng một bản ghi, tự gắn object trùng tên (hoặc ghi chú Attributes); giữ đúng dấu tiếng Việt (chuẩn hoá NFC, file Excel Windows-1258 cũng đọc được). Duyệt bản ghi trong bảng Variables. Viết lại theo đặc tả, không chép mã Variable Importer. |

### Khác DanCard

- Mọi lần dàn là **một bước hoàn tác** (Ctrl+Z bỏ cả lần dàn); lỗi giữa chừng không để lại gì.
- Hộp thoại của DanCard thành tham số hoặc câu hỏi ngay trên panel (Dàn nhiều loại, Xác nhận con chuẩn, ghi chú CTL).
- Tờ mới của Dàn KTS, Dàn bế, Dàn Offset đặt vào chỗ trống bên phải những gì đang có; Dàn theo mẫu và CTL Offset xếp từ góc trên trái canvas (±7200 pt quanh gốc) như DanCard. Như DanCard, các artboard cũ bị xoá khung (bài giữ nguyên).
- Dàn bế đếm phép thử thay cho đồng hồ, và bản Windows còn chặn thêm bằng thời gian thật, nên không bao giờ chạy quá ngân sách 3 giây (cộng bước canh khe) của bản gốc.
- Tab Variable chưa có biến ảnh (`@`) và biểu đồ (`%`): cột đó bị bỏ qua kèm cảnh báo.
- Rộng × Cao của Resize/Clip tối đa 1000 cm (số lớn hơn là gõ nhầm).

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
