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
| `dancard.clipToSize` | Nút **Clip** như DanCard: clip theo kích thước đúng tâm phần nhìn thấy (bỏ object ẩn, guide, path không tô), clip group có sẵn thì giữ khung, rồi Expand Appearance; mask theo khung transform của object (card xoay hay đặt nghiêng vẫn clip đúng trục, tâm đo theo trục card; `followAngle: false` để dùng khung thẳng). |
| `dancard.resize` | Resize từng object thành đúng Rộng × Cao theo khung transform của nó (card nghiêng co giãn theo trục card), giữ tâm. |
| `dancard.rasterize` | Nút **Raster** đầu panel: CMYK, 500 ppi, nền trong suốt, khử răng cưa Art, không lề, không thêm clipping mask. Ảnh đúng vùng nhìn thấy (clip group: phần trong mask; chọn object con trong clip group thì raster cả clip group). Card đặt nghiêng: ảnh nghiêng theo khung, không dư góc. Ảnh phủ kín lưu **TIFF CMYK giữ đúng mực** (chữ K 100 vẫn chỉ mực K); có chỗ trong suốt thì lưu PNG có kênh trong suốt. Tài liệu RGB: báo đổi sang CMYK. |
| Object › Rasterize… | Hộp thoại đầy đủ: Color Model, Resolution (Screen/Medium/High/Use Document Raster Effects Resolution/Other), Background, Anti-aliasing (None/Art Optimized/Type Optimized), Create Clipping Mask, Add … Around Object, thêm ô DanCard *Theo khung transform* (`followAngle`). Tài liệu CMYK: ảnh phủ kín lưu TIFF CMYK. |
| `dancard.layout` + tab **Dàn card** | Dàn card như `dcDan`: 9.2×5.6 (20 chỗ, nhận card đôi 18.4×5.6), 9×5.5 (20), 9.2×5.2 (21), 8.7×5.5 (22), 10.2×15.2 (6), voucher 15.2×7.2 (8, ghép thêm 3 card 9.2×5.6). 1 cột = in 1 mặt, 2 cột = mỗi hàng [trước][sau]. Mỗi mẫu clip đúng khổ (clip group có sẵn thì giữ), Expand Appearance, nhân bản vào các ô; mặt trước AB1, mặt sau AB2 đối xứng. Artboard + pon lấy từ file pon chung (tự đọc `dan_card_config.txt` của DanCard trên Illustrator nếu có, hoặc bấm Chọn…). Mẫu được đo và dựng thẳng theo khung transform (card xoay trong app hay đặt nghiêng trong file .ai vẫn đúng khổ). Sai khổ thì tự sửa bằng clip, giữ vector, như DanCard tự sửa (có nền gần đúng khổ thì clip theo nền; không thì clip đúng khổ ở tâm khi object phủ kín khổ đó); không được thì báo lỗi và không đổi gì. Cardoffset thêm ghi chú. Cả lần dàn là một bước hoàn tác. |
| `dancard.cutMarks` + tab **Dấu cắt** | Dấu cắt 4 góc cho các bài đang chọn (group đã dàn có từ 2 clip group: mỗi clip group một bài), cùng quy tắc `dcThemDauCatTuDong`: nét ra vùng trống, sát mép artboard thì lật vào trong, không đè bài khác, không vẽ trùng khi bấm lại; layer "Dau cat tu dong", nét K 100 % 1 pt. |
| Panel: tab **Đổi tên** | Giao diện của `dancard.rename` (tiền tố, kiểu đánh, bắt đầu, đệm số 0, lặp nhãn, hướng đếm). |
| `dancard.ktsLayout` + **Dàn tối ưu › Dàn KTS** | Như `dcDanToiUu` v22: dàn tối ưu không PON, lề 3 mm, khe 0, con sát nhau (hàng/cột thường rồi con xoay 90°, chẻ guillotine 2 tầng, canh giữa). Bài đo và nhân bản theo khung transform. Dàn 2 mặt: chọn theo hàng trái = trước, phải = sau; mặt sau đối xứng ngang. Nhiều mẫu vào một tờ: nhiều khổ thì con lớn trước, con nhỏ lấp chỗ trống, mẫu đã dàn bị xoá khỏi nguồn. Số vị trí không chia đều cho số mẫu thì panel hỏi **Dừng lại / Tiếp tục**. Panel hiện "n con / tờ". |
| `dancard.dieLayout` + **Dàn tối ưu › Dàn bế** | Như Dàn bế v2.16.20 (`dan_be_nester.js` viết lại bằng Rust): chọn từng cặp Khuôn (trái) → Bài (phải), 2 mặt thì bộ ba Khuôn → trước → sau; không chọn gì thì lấy theo layer Khuôn bế / Bài. Khuôn đặt nghiêng được dựng thẳng theo khung transform (Bài bỏ cùng góc). Xếp theo đường bế thật (xoay 0/90/180/270°, lưới 0,25 mm), khe giữa đường bế, lề, né PON; vẽ 4 chấm PON tròn 5 mm, layer PON / Khuôn / Bài; tờ mới từ góc trên trái canvas như bản gốc. Panel hiện "n con / tờ". Bộ test gốc `tests/dan_be_*.test.js` (lõi dàn, đọc khuôn, vẽ tờ, hai mặt, canvas, panel) đã chuyển sang Rust (`dancard_be_goc_tests.rs`) với cùng dữ liệu mẫu, cùng mốc số con (lấy số bản JS đo được nếu cao hơn) và cùng các phép kiểm tra hình học độc lập; chạy lại 94 lần dàn của bộ test gốc, bản Rust bằng hoặc hơn bản JS ở mọi lần. |
| `dancard.tuTro` + **Dàn tối ưu › Dàn Offset** | Như `dcDanTuTro` v11: tự trở, nửa trái mặt trước, nửa phải đối xứng qua tim giấy, in xong lật ngang tờ; mỗi mặt đo theo khung transform. Cùng khổ: tự thử xoay, tự bóp tối đa "Bóp tối đa" mm khi nhờ vậy dàn thêm được con; khổ lẫn lộn: mỗi cặp một chỗ. Lặp 1 cặp cho đầy tờ. PON giấy 4 góc. Cặp đã dàn bị xoá, cặp chưa vừa giữ lại. |
| `dancard.learnTemplate` / `dancard.applyTemplate` + tab **Dàn theo mẫu** | Học mẫu 1 mặt hoặc 2 mặt (mặt sau đối xứng) từ bố cục dàn tay: vị trí, khung và góc nghiêng theo khung transform của từng con, hướng artwork 0/90/180/270° so với con chuẩn (đọc từ ma trận ảnh/chữ, không được thì so ảnh). Cần chọn con chuẩn thì panel hiện nút **Xác nhận con chuẩn**. Áp mẫu: mỗi nguồn dựng thẳng theo khung transform, giữ vector, xoay, co đúng khung và nghiêng theo từng vị trí, mỗi lô một artboard đúng khổ file pon (nhớ file pon mặt trước/mặt sau). |
| `dancard.saveTemplatePdf` | Lưu PDF đã dàn: từng artboard 1 PDF hoặc mặt trước + mặt sau 1 PDF, tên "file 1 - <nội dung>.pdf", không ghi đè file có sẵn. |
| `dancard.copyNotes` | Copy ghi chú đang chọn sang các artboard sau (bài 2 mặt: artboard 1, 3, 5…; 1 mặt: mọi artboard), cùng khoảng cách tới góc artboard, thêm tiền tố F1, F2… nếu nhập. |
| `dancard.saddleStitch` + tab **CTL Offset › Đóng ghim giữa** | Như `dcRunSignature8`: số trang chia hết cho 4; có bìa thì 4 trang ngoài thành tờ tự trở 4 trang; ruột tự trở 4/8 (A5 thêm 16) rồi các tờ AB 16 trang (A5 32). Trang dựng thẳng theo khung transform, giữ vector. Tờ giấy cố định (85,8 × 63,8, 85,8 × 62,5, 64,8 × 41,8, 42,8 × 31,3 cm), bóp ruột 1 mm mỗi tờ, PON giấy và PON cắt trên hai layer khóa, ghi chú RUỘT N ở mặt A. Bìa A4 cần rộng ≤ 20,9 cm như bản gốc. |
| `dancard.perfectBind` + tab **CTL Offset › Keo gáy** | Như `dcRunKeoGay`: tuần tự, không bóp, không khe; A4 16 trang / tờ AB, A5 32 trang (2 tay 16), phần dư tự trở TT16/TT8/TT4. |
| `dancard.saveCtlPackage` | Nút **Lưu AI + JPG + ZIP** theo từng khổ vừa dàn: mỗi tờ một file .ai (nội dung PDF, Illustrator mở được), một JPG CMYK 300 ppi (cả mặt A + B) và một ZIP nén deflate chứa hai file đó; không ghi đè. |
| `dancard.catalogue` + tab **Catalogue** | Như `dcRunCatalogueAuto`: tờ PON 43 × 32,5 cm, trang dựng thẳng theo khung transform (giữ vector), ưu tiên 2 cuốn / tờ, xoay cuốn khi cần, ghép cặp đóng gáy giữa, ruột bóp 0,25 mm mỗi tờ, PON chữ L trên layer khóa, ghi chú bìa/ruột. |
| `dancard.autoSavePdf` + tab **Auto Save** | Như `dcAutoSavePDF`: mỗi con (mỗi số hộp) một PDF, 1 mặt 1 trang, 2 mặt 2 trang, trang đúng khung clip (mặt xoay/nghiêng được dựng thẳng theo khung transform); tên `<mã> - <STT> - <số hộp> hop[ - ghi chú][ - 1 mat] - <cán màng> - <ngày>.pdf`; không sửa tài liệu, không ghi đè. |
| `dancard.clipCard` | Nút Clip 9,2 × 5,6 tự xoay (`dcClipCard926`): ngang/dọc theo clip cha gần nhất hoặc chính object; neo tâm theo clip cha (chọn object con thì clip lại cả clip group), không có thì theo nền đúng khổ trong group trông như một trang, không có nữa thì theo khung của object; mỗi card một lần; mask theo khung transform. |
| `dancard.importVariables` + tab **Variable** | Nhập file CSV/TXT thành Variables: mỗi cột một biến (cột `#` là biến ẩn/hiện: true/on hiện, false/off/trống ẩn, chữ khác hiện), mỗi dòng một bản ghi, tự gắn object trùng tên (hoặc ghi chú Attributes); giữ đúng dấu tiếng Việt (chuẩn hoá NFC, file Excel Windows-1258 cũng đọc được). Duyệt bản ghi trong bảng Variables. Viết lại theo đặc tả, không chép mã Variable Importer. |

### Khác DanCard

- DanCard trên Illustrator không đọc được góc xoay nên đo bằng khung thẳng của trang và raster để
  sửa khổ. Ở đây mọi lệnh dàn (Dàn card, KTS, Dàn Offset, Dàn bế, Dàn theo mẫu, CTL Offset,
  Catalogue, Auto Save, Clip) đo và đặt theo **khung transform** (như bảng Transform hiện), dựng thẳng bài
  trước khi dàn và **giữ nguyên vector**, không tự raster. Chỉ nút **Raster** mới raster. (Bước
  Expand Appearance của Dàn card và nút Clip, như DanCard, vẫn biến hiệu ứng raster như đổ bóng, làm
  mờ, phát sáng thành ảnh, như Illustrator.)
- Dựng thẳng chỉ bỏ **phần nghiêng lẻ** của khung (góc đưa về -45°…45°): bài xoay đúng 90° / 180°
  giữ nguyên hướng như đang thấy trên trang, đúng như DanCard đo, nên số con, thứ tự trang và cách
  ghép cặp không đổi. Rộng × Cao của Resize và Clip là ngang × dọc như đang thấy trên trang.
- Mọi lần dàn là **một bước hoàn tác** (Ctrl+Z bỏ cả lần dàn); lỗi giữa chừng không để lại gì.
- Hộp thoại của DanCard thành tham số hoặc câu hỏi ngay trên panel (Dàn nhiều loại, Xác nhận con chuẩn, ghi chú CTL).
- Tờ mới của Dàn KTS, Dàn Offset đặt vào chỗ trống bên phải những gì đang có; Dàn bế, Dàn theo mẫu và CTL Offset xếp từ góc trên trái canvas (±7200 pt quanh gốc) như DanCard. VectorCraft không có tuỳ chọn Large Canvas: tờ hay lô Dàn bế quá canvas thường thì tự dùng vùng rộng gấp 10 như tài liệu Large Canvas, thay cho báo lỗi. Như DanCard, các artboard cũ bị xoá khung (bài giữ nguyên).
- Dàn bế đếm phép thử thay cho đồng hồ 3 giây: mọi máy cho đúng kết quả bộ test đã kiểm (không phụ thuộc máy nhanh chậm như bản gốc). Thời gian thật chỉ là chốt an toàn khi máy chậm hơn hẳn (quá 2 lần ngân sách).
- Dàn bế tối đa 50 000 con một tờ (chốt an toàn bộ nhớ; bộ test gốc dùng tới 13 440 khuôn tí hon). Guide trong khuôn không được tính là đường bế. Khuôn nhóm cùng nhãn chữ được đặt theo đúng khung đường bế đã dàn (bản gốc đặt theo khung cả nhóm).
- Tab Variable chưa có biến ảnh (`@`) và biểu đồ (`%`): cột đó bị bỏ qua kèm cảnh báo.
- Rộng × Cao của Resize/Clip tối đa 1000 cm (số lớn hơn là gõ nhầm).
- Dàn card tự sửa sai khổ khi không có nền: object đứng dọc thì khung clip cũng đứng dọc (DanCard luôn clip nằm ngang nên báo sai khổ), rồi được xoay về khổ đích như card đứng dọc đúng khổ.

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

## Cập nhật khi VectorCraft ra bản mới

Nhánh `dancard` = VectorCraft gốc (`storytold/vectorcraft`, nhánh `main`) + các commit DanCard,
luôn **gộp (merge)** bản gốc vào, không rebase, để GitHub Desktop trên máy không phải ép push.
Code DanCard nằm trong file riêng (`cmd/dancard*.rs`, `panels/dancard*.rs`), nên mỗi lần gộp
thường chỉ cần một lệnh.

**Cách nhanh nhất:** GitHub › tab Actions › **DanCard đồng bộ VectorCraft** › Run workflow. Workflow
lấy bản mới, gộp vào `dancard`, chạy test DanCard. Qua thì tự đẩy lên `dancard` và build bản Windows
(**DanCard Windows x64**). Đụng nhau thì không đổi gì, trang tóm tắt của lần chạy liệt kê các file
cần sửa. Khi đó nhờ Claude sửa: mở phiên mới, nói "gộp VectorCraft mới vào dancard".

- **Cần làm một lần:** đặt `dancard` làm nhánh mặc định của fork (Settings › General › Default
  branch). GitHub chỉ hiện workflow này ở tab Actions (nút Run workflow) và chỉ chạy lịch sáng thứ
  Hai 08:00 (giờ Việt Nam) khi file workflow nằm trên nhánh mặc định; nhánh `main` của fork chỉ có
  workflow của bản gốc.
- **Nên làm:** tạo token riêng cho việc đẩy (GitHub › Settings › Developer settings › Fine-grained
  tokens: chỉ repo này, quyền **Contents** và **Workflows** đều Read and write), lưu ở Settings ›
  Secrets and variables › Actions tên `DANCARD_SYNC_TOKEN`. Không có token thì workflow vẫn chạy,
  nhưng lần bản gốc sửa file trong `.github/workflows` thì GitHub không cho đẩy (trang tóm tắt sẽ
  báo); có token thì lần đẩy còn tự kích hoạt build Windows.
- Sau khi workflow đẩy lên, GitHub Desktop trên máy cần **Fetch / Pull** trước khi commit tiếp.

**Phiên Claude làm tay** (khi workflow báo đụng nhau, hoặc muốn kiểm tra kỹ hơn):

```sh
bash .github/dancard/sync.sh             # gộp + fmt + test DanCard + quét lệnh
CHECK=none bash .github/dancard/sync.sh  # chỉ gộp
```

Script thoát mã 2 khi đụng nhau (đã huỷ gộp, in danh sách file), mã 3 khi gộp được nhưng kiểm
tra hỏng (commit gộp còn đó để sửa), mã 4 khi git không gộp được vì lý do khác. Kiểm tra gồm fmt,
test DanCard (engine + giao diện, có bộ test gốc Dàn bế), quét mọi lệnh với tham số rác, clippy
(cảnh báo trong file DanCard là lỗi) và phân tầng crate. Sửa xong thì đẩy lên như thường lệ.

### Các chỗ DanCard sửa vào file của VectorCraft

Chỉ những file này có thể đụng nhau khi gộp; mọi thứ khác là file riêng của DanCard.

| File | DanCard thêm / sửa |
|---|---|
| `crates/engine/src/lib.rs` | `Prefs`: `force_document_units`, `ctrl_direct_selection`, `selection_tool_anchors`, `dancard_defaults`; `replace_document` ép đơn vị General |
| `crates/engine/src/tooling.rs` | giữ Ctrl → Direct Selection (`lent_tool`, `ctrl_tool`) |
| `crates/engine/src/cmd/mod.rs` | `mod dancard;` và đăng ký `dancard::specs()` |
| `crates/engine/src/cmd/effectcmd.rs` | `expand_node` thành `pub(crate)` |
| `crates/engine/src/cmd/fileio/load.rs` | file mở ra dùng đơn vị General |
| `crates/engine/src/cmd/menucmds.rs` | Object › Rasterize dùng `dancard::render_raster`, tùy chọn `followAngle` |
| `crates/engine/src/cmd/prefscmds.rs` | 3 dòng `PREF_SPECS`; Reset trả về mặc định DanCard |
| `crates/render/src/lib.rs` | `render_inks` (ảnh CMYK giữ đúng mực) |
| `crates/ui-egui/src/canvas.rs` | tool đang hiện khi giữ Ctrl; ẩn điểm neo với Selection |
| `crates/ui-egui/src/dialogs/mod.rs`, `dialogs/rasterize.rs` | hộp thoại Object › Rasterize đầy đủ (`rasterize.rs` là file mới) |
| `crates/ui-egui/src/i18n/tests.rs` | bỏ qua chữ tiếng Việt của DanCard khi kiểm tra bản dịch |
| `crates/ui-egui/src/menus.rs` | Window › Công cụ bình |
| `crates/ui-egui/src/panels/mod.rs` | panel Công cụ bình |
| `crates/ui-egui/src/prefs_dialog.rs` | áp mặc định DanCard một lần khi khởi động |
| `crates/ui-egui/src/state.rs` | `UiState.dancard` (giá trị đã nhập trên panel) |

Khi sửa chỗ đụng nhau: giữ phần mới của VectorCraft, thêm lại đúng phần DanCard ghi ở bảng trên.
Thêm chỗ chạm mới vào file gốc thì ghi vào bảng này.

## Giấy phép

Giữ nguyên `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`. Logo ArtCraft là nhãn hiệu: không dùng cho
bản phát cho nhân viên.
