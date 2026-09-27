# Remote Desktop Platform - Kế Hoạch Phát Triển & Quản Lý Dự Án (Master Plan)

> **Mục tiêu**: Xây dựng ứng dụng Remote Desktop đa nền tảng (macOS, Windows, Linux) tương tự AnyDesk / RustDesk bằng Rust + Tauri v2 + React.
> **Kiến trúc**: Clean Architecture (Hexagonal Ports & Adapters) với 9 crates mô-đun hóa cao, dễ bảo trì, độ trễ thấp (< 200ms).

---

## I. KIẾN TRÚC HỆ THỐNG (SYSTEM ARCHITECTURE)

```
┌────────────────────────────────────────────────────────────────────────┐
│                          rd-desktop (Tauri v2)                         │
│   ┌───────────────────────────────┐  ┌──────────────────────────────┐  │
│   │       React Frontend UI       │  │     Tauri Rust Backend       │  │
│   │  - Share Screen (Host Panel)  │  │  - Session Manager           │  │
│   │  - Connect (Viewer Canvas)    │  │  - Host Capture Loop Task    │  │
│   │  - Security & Permission Box  │  │  - Viewer Receiver Task      │  │
│   │  - Settings & Remote Toolbar  │  │  - Input Forwarder           │  │
│   └───────────────┬───────────────┘  └──────────────┬───────────────┘  │
└───────────────────┼─────────────────────────────────┼──────────────────┘
                    │ IPC                             │
┌───────────────────┴─────────────────────────────────┴──────────────────┐
│                             Core Crates                                │
│                                                                        │
│   [rd-core]      Domain Models, Traits (Ports), Protocol Messages      │
│   [rd-codec]     JPEG & H.264 Encoder / Decoder                        │
│   [rd-transport] WebRTC DataChannel (P2P) + QUIC Fallback              │
│   [rd-platform]  OS Abstraction (macOS ScreenCaptureKit/CGEvent,       │
│                  Windows DXGI/SendInput, Linux PipeWire/X11)           │
│   [rd-client]    RemoteSession API & State Management                  │
└────────────────────────────────────┬───────────────────────────────────┘
                                     │ Network
┌────────────────────────────────────┴───────────────────────────────────┐
│                    Signaling & Infrastructure                          │
│                                                                        │
│   [rd-signaling] Axum WebSocket Server (Port 3030)                     │
│                  - 6/9-Digit Peer ID Routing                           │
│                  - SDP Offer / Answer Exchange                         │
│                  - ICE Candidate Exchange                              │
│   [STUN / TURN]  Google STUN (stun.l.google.com:19302) + Coturn        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## II. LỘ TRÌNH TRIỂN KHAI THEO GIAI ĐOẠN (ROADMAP & MILESTONES)

### 📌 Milestone 1: Hoàn thiện Luồng P2P Streaming & Remote Control Hai Chiều (MVP Core) ✅
*Mục tiêu: Máy Host stream màn hình thật sang Viewer qua WebRTC, Viewer hiển thị mượt mà và điều khiển được chuột/bàn phím của Host.*

- [x] **1.1. Host Streaming Worker**:
  - [x] Khởi tạo luồng nền khi bấm `start_host` trong `rd-desktop`.
  - [x] Lấy frame từ `rd-platform::create_screen_capture()`.
  - [x] Nén qua `rd-codec::JpegEncoder` (tối ưu xử lý BGRA/RGBA trực tiếp sang RGB).
  - [x] Gửi qua `WebRTCTransport::send_msg(ProtocolMessage::ScreenFrame)`.
- [x] **1.2. Host Remote Input Executor**:
  - [x] Lắng nghe `ProtocolMessage::InputEvent` từ DataChannel của `WebRTCTransport`.
  - [x] Cải tiến `MacOSInputInjector`: Lưu vết vị trí chuột (`last_x`, `last_y`) để click chính xác toạ độ.
  - [x] Bổ sung xử lý cuộn chuột `MouseScroll` bằng `CGEvent::new_scroll_event` với feature `highsierra`.
- [x] **1.3. Viewer Receiver & Canvas Renderer**:
  - [x] Trong `connect_peer`, khởi chạy tác vụ nền nhận `ScreenFrame` từ `WebRTCTransport`.
  - [x] Giải nén JPEG bằng `rd-codec::JpegDecoder` trả về RGBA trực tiếp.
  - [x] Blit dữ liệu frame lên Canvas React bằng native `ImageData` (0ms JS loop overhead).
- [x] **1.4. Chuẩn hoá toạ độ chuột & sự kiện điều khiển**:
  - [x] Map toạ độ chuột theo tỷ lệ thực tế giữa Canvas Viewer và màn hình Host.
  - [x] Bắt sự kiện chuột trái, chuột phải, cuộn chuột và chặn context menu trình duyệt trên Canvas.

---

### 📌 Milestone 2: Cơ Chế Bảo Mật & Trải Nghiệm Chuẩn AnyDesk (Security & UX) ✅
*Mục tiêu: Đảm bảo bảo mật khi kết nối, hỗ trợ cả 2 chế độ: Có người trực (Accept Dialog) và Không người trực (Password).*

- [x] **2.1. Quản lý trạng thái kết nối**:
  - [x] Trạng thái phiên kết nối: Ready, Sharing, Connecting, Connected, Disconnected.
  - [x] One-click Copy Peer ID với visual feedback.
- [x] **2.2. Hộp thoại xin phép kết nối (Incoming Connection Prompt)**:
  - [x] Khi có Viewer gửi yêu cầu kết nối, Host hiển thị Popup: *"Connection Request từ thiết bị [ID]"*.
  - [x] Tùy chọn cấp quyền chi tiết: Cho phép hoặc không cho phép điều khiển chuột/phím (`allow_input`).
  - [x] Nút **Chấp nhận (Accept)** và **Từ chối (Decline)** kèm cơ chế handshake qua WebRTC DataChannel (`SessionRequest` -> `SessionCreated` / `SessionEnd`).
- [x] **2.3. Chế độ truy cập không người trực (Unattended Access)**:
  - [x] Cài đặt mật khẩu truy cập từ xa lưu trữ an toàn dưới dạng mã băm SHA-256 (`sha2`).
  - [x] Viewer hỗ trợ nhập mật khẩu để kết nối tự động không cần người ngồi tại Host bấm Accept.
- [x] **2.4. Thanh công cụ phiên điều khiển & Chất lượng stream**:
  - [x] Nút ngắt kết nối nhanh (Disconnect Session).
  - [x] Nút Fullscreen toggle cho viewport điều khiển.
  - [x] Bộ chọn chất lượng luồng thời gian thực: Speed (Q=50) / Balanced (Q=70) / Best (Q=85).
  - [x] Chỉ số hiển thị FPS thời gian thực trên màn hình Viewer.
- [x] **2.5. Chuyển tiếp bàn phím (Keyboard Forwarding)**:
  - [x] Bắt sự kiện `KeyDown` và `KeyUp` khi Canvas nhận focus.
  - [x] Bảng map đầy đủ từ mã phím chuẩn W3C (`e.code`) sang mã phím ảo của OS.
  - [x] Mô phỏng phím bấm vật lý thời gian thực qua `CGEvent::new_keyboard_event`.

---

### 📌 Milestone 3: Tối Ưu Hóa Hiệu Năng & Codec (Performance & Codec)
*Mục tiêu: Giảm thiểu tiêu thụ CPU/RAM và băng thông mạng (< 2-3 Mbps cho 1080p 60fps).*

- [x] **3.1. Phân mảnh gói tin (SCTP Packet Chunking) & Frame Reassembly** ✅:
  - [x] Định nghĩa `ProtocolMessage::FrameChunk` với `chunk_index`, `total_chunks`, `sequence`, `data`.
  - [x] Phân chia gói tin tự động khi frame JPEG vượt quá 50,000 bytes trên Host side để đảm bảo nằm gọn trong buffer MTU WebRTC SCTP.
  - [x] Bộ gom mảnh ghép khung hình (Frame Assembler) phía Viewer với cấu trúc `PendingChunkedFrame`, tự động loại bỏ chunk của frame cũ và reassemble hoàn chỉnh trước khi decode.
- [x] **3.2. Chuyển đổi màu sắc tối ưu (Zero-Copy Color Conversion)** ✅:
  - [x] Tối ưu hóa pipeline xử lý BGRA/RGBA trực tiếp sang RGB trong `JpegEncoder` không qua trung gian `DynamicImage`.
  - [x] Giảm tải CPU đáng kể và triệt tiêu độ trễ render trên Canvas React bằng SIMD `ImageData`.
- [ ] **3.3. Tích hợp H.264 / AV1 Codec**:
  - [ ] Tích hợp phần cứng: Apple VideoToolbox (macOS), NVENC / Media Foundation (Windows).
  - [ ] Hỗ trợ stream qua WebRTC Media Stream trực tiếp hoặc DataChannel.

---

### 📌 Milestone 4: Mở Rộng Đa Nền Tảng (Windows & Linux) ✅
*Mục tiêu: Đạt 100% tính năng Screen Capture và Input Injection trên cả 3 nền tảng macOS, Windows và Linux.*

- [x] **4.1. Windows Platform (Đã kiểm tra & verify biên dịch)** ✅:
  - [x] Triển khai `WindowsScreenCapture`: Chụp màn hình Desktop bằng Win32 GDI (`CreateCompatibleDC`, `CreateCompatibleBitmap`, `BitBlt`, `GetDIBits` với top-down DIB BGRA format) trong background thread `spawn_blocking`.
  - [x] Triển khai `WindowsInputInjector`: Mô phỏng đầy đủ thao tác chuột và bàn phím bằng Win32 `SendInput` API (`MOUSEEVENTF_ABSOLUTE` với chuẩn hóa toạ độ 0..65535, `MOUSEEVENTF_LEFTDOWN/UP`, `MOUSEEVENTF_RIGHTDOWN/UP`, `MOUSEEVENTF_MIDDLEDOWN/UP`, `MOUSEEVENTF_WHEEL/HWHEEL`, `KEYEVENTF_KEYUP`).
  - [x] Kiểm thử biên dịch thành công 100% qua target `aarch64-pc-windows-msvc`.
- [x] **4.2. Linux Platform (Đã kiểm tra & verify biên dịch)** ✅:
  - [x] Triển khai `LinuxScreenCapture`: Kết nối X server qua `x11rb`, chụp màn hình root window bằng `ZPixmap` (`get_image`) định dạng BGRA và liệt kê đa màn hình từ `roots`.
  - [x] Triển khai `LinuxInputInjector`: Mô phỏng chuột và bàn phím bằng giao thức X11 XTest (`xtest_fake_input`) hỗ trợ di chuyển chuột (`MOTION_NOTIFY_EVENT`), bấm phím/chuột (`BUTTON_PRESS/RELEASE`, `KEY_PRESS/RELEASE`), cuộn chuột bánh xe (Button 4/5/6/7) và `conn.flush()`.
---

### 📌 Milestone 5: Tính Năng Nâng Cao Chuẩn AnyDesk (File Transfer, Clipboard, Audio) ✅
*Mục tiêu: Đạt đầy đủ trải nghiệm AnyDesk chuyên nghiệp với truyền file hai chiều, đồng bộ khay nhớ tạm và stream âm thanh.*

- [x] **5.1. Đồng bộ Clipboard hai chiều (Bidirectional Clipboard Sharing)** ✅:
  - [x] Triển khai `ClipboardManager` trong `rd-platform` sử dụng `arboard`, chạy ngầm giám sát thay đổi khay nhớ tạm mỗi 400ms.
  - [x] Gửi thông điệp `ProtocolMessage::ClipboardSync` qua WebRTC DataChannel.
  - [x] Cơ chế ghi nhận `last_synced` chống phản xạ vô tận (anti-reflection echo loop).
  - [x] Hiển thị huy hiệu thông báo thời gian thực trên giao diện React khi clipboard được đồng bộ.
- [x] **5.2. Truyền file hai chiều (Bidirectional File Transfer)** ✅:
  - [x] Thiết kế giao thức truyền file: `FileTransferRequest`, `FileChunk` (phân mảnh 32KB), `FileTransferComplete`.
  - [x] Lệnh Tauri `send_file` hỗ trợ gửi file từ cả Host và Viewer.
  - [x] Tự động lưu file nhận được vào thư mục `Downloads` chuẩn của OS (`USERPROFILE/Downloads` trên Windows, `~/Downloads` trên macOS/Linux).
  - [x] Thanh tiến trình truyền file (Progress Bar) động và Toast thông báo hoàn thành trên UI React.
- [x] **5.3. Truyền tải âm thanh độ trễ thấp (Low-Latency Audio Streaming)** ✅:
  - [x] Triển khai `AudioCaptureService` trong `rd-platform` sử dụng thư viện chuẩn `cpal` trên thread chuyên biệt.
  - [x] Thu âm thanh hệ thống/micro, mã hóa thành luồng 16-bit PCM (Little-Endian, 48kHz Stereo) đóng gói vào `ProtocolMessage::AudioFrame`.
  - [x] Phía Viewer giải mã và phát âm thanh trực tiếp bằng Web Audio API (`AudioContext`) độ trễ cực thấp.
  - [x] Nút Mute / Unmute âm thanh tích hợp trực tiếp trên thanh công cụ điều khiển nổi (Floating Toolbar).

---

## III. QUY TẮC THIẾT KẾ CODE ĐỂ DỄ DÀNG BẢO TRÌ (CLEAN CODE & MAINTAINABILITY)

1. **Tuân thủ Clean Architecture**:
   - `rd-core` không phụ thuộc vào bất kỳ thư viện nền tảng hay giao diện nào.
   - Thêm tính năng mới (ví dụ codec H.264, platform Android) chỉ cần tạo implementation mới cho trait trong `rd-codec` hoặc `rd-platform`.
2. **Quản lý bất đồng bộ (Async & Thread Safety)**:
   - Các tác vụ I/O nặng và capture loop chạy trên Tokio Tasks hoặc thread riêng biệt (`spawn_blocking` cho CGEvent/DXGI).
   - Truyền dữ liệu qua bounded channels (`tokio::sync::mpsc`) để tránh tràn RAM khi mạng chậm.
3. **Quản lý lỗi tập trung**:
   - Sử dụng `thiserror` cho các lỗi domain/port và `anyhow` cho ứng dụng runtime/CLI.
   - Luôn trả về thông báo lỗi rõ ràng cho người dùng ở tầng UI thay vì crash app.
