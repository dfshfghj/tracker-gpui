# Tracker GPUI

GPUI 版的 Tracker 视频工作区原型：左侧选择工作目录并显示视频文件树，中间是
带标签页的视频组件（视频 + 传输控制 + 进度条），右侧是属性和数据表。

视频层由 **FFmpeg 9** 真实解码，按 Tracker/XuggleVideo 的模型工作：启动时建立
packet 索引，之后按帧请求、顺序解码或 seek 关键帧，不再让解码线程追着墙钟跑。

- 依赖：`gpui-kit` / `gpui-component` 0.7.3（内部 `gpui-pre` 0.3.9）
- 解码：`ffmpeg-the-third` 6.0 / FFmpeg 9（Windows x64 shared；macOS Homebrew shared）
- 视频：FFmpeg 解码 → swscale BGRA → `RenderImage`；轨迹/手柄用图元绘制
- 工作区：目录树、多标签、视频左侧竖向工具栏、右侧可折叠属性/数据面板
- 分析布局：图与数据表上下分区、独立滚动，拖动分界线调整比例；分区可最大化，每张图及表格可移至独立窗口，关闭后自动放回。独立窗口跟随当前视频和轨道。
- 自定义数据变量：按轨道定义公式，自动推导量纲，并同步加入绘图和数据表；详见 [使用说明](CUSTOM_VARIABLES.md)
- 视觉（可选）：OpenCV 4.12（`vision` feature），用于运动目标检测；后续用于标定/去畸变、ArUco 刚体识别

## 构建（Windows / macOS）

Windows 使用仓库外的 FFmpeg 9 shared 预编译包（默认放在
`third_party/ffmpeg-n9.0-latest-win64-lgpl-shared-9.0`），通过脚本设置
`FFMPEG_DIR` 和 `LIBCLANG_PATH`：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 run
```

macOS 使用 Homebrew 的共享 FFmpeg，由 `pkg-config` 自动发现；脚本会同时检查
`pkg-config`、Homebrew 前缀和可选的 LLVM/libclang：

```bash
brew install ffmpeg pkg-config
./scripts/build-macos.sh run
```

- 需要 FFmpeg 9 shared（Windows 包与 `ffmpeg-the-third` 6.0 对应）。macOS 可用
  `pkg-config --modversion libavcodec` 检查版本；使用自定义前缀时设置
  `FFMPEG_DIR=/path/to/prefix ./scripts/build-macos.sh run`。
- `.cargo/config.toml` 不再硬编码 Windows 路径，避免 macOS 构建被全局环境变量
  破坏。
- `build.rs` 只在 Windows 目标复制 `av*/sw*` DLL；macOS 直接链接 Homebrew dylib。
- macOS 自动主题使用 `defaults read -g AppleInterfaceStyle`。
- AutoTracker 窗口在 Windows 上是主窗口的 owned window（Win32
  `GWLP_HWNDPARENT`）；macOS 上使用 GPUI `WindowKind::Floating`
  （`NSFloatingWindowLevel`），保持位于主窗口上方。

### OpenCV 与自动检测（vision feature）

自动检测编译在可选的 `vision` feature 中。Windows 使用 OpenCV 4.12 官方预编译包：

```powershell
# 下载 opencv-4.12.0-windows.exe 后解压到 third_party/opencv
7z x -y -othird_party/opencv opencv-4.12.0-windows.exe
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 run
```

脚本检测到 `third_party/opencv/opencv/build/x64/vc16/lib/opencv_world4120.lib`
后会自动追加 `--features vision`；`build.rs` 会把 `opencv_world4120.dll` 复制到
`target/<profile>/`。OpenCV 在其他位置时，设置 `OPENCV_INCLUDE_PATHS`、
`OPENCV_LINK_PATHS`、`OPENCV_LINK_LIBS` 后再执行 `cargo build --features vision`。

CI 不再下载官方预编译包，而是用 `scripts/build-opencv-minimal.ps1` /
`build-opencv-minimal.sh` 从源码编译裁剪版 OpenCV 4.12（core、imgproc、imgcodecs、
video、objdetect、calib3d、features2d、flann，world 动态库），并缓存 install
prefix；Windows 和 macOS 都以 `cargo build --release --all-features` 构建。
macOS 会把裁剪版 OpenCV dylib 放进 `.app/Contents/Frameworks` 并参与签名。

检测流程：MOG2 背景差分 + 帧差 → 形态学滤波 → 轮廓/质心 → 最近邻 + 速度预测关联；
按轨迹长度、亮度、目标面积和画面中心位置选出主运动目标，批量写入新的质点轨道。
适用于固定机位的球、滑块、摆、颗粒等；移动机位/强遮挡场景后续接 AI 检测。
工具栏的扫描框按钮和 Tracks 菜单中的“自动检测运动目标”都可触发。

测试钩子：`PROBE_VIDEO=... PROBE_AUTODETECT=1 PROBE_AUTODETECT_PREVIEW=...`
同步检测并输出预览图；`PROBE_AUTODETECT_UI=1` 走异步 UI 路径。

### 刚体轨道（ArUco / ORB / 运动轮廓）

刚体轨道复用 `Track`：质心仍存为普通点，额外保存每帧的 `angle`、特征尺寸
`scale` 和标记/内点数量；工程文件 v3 增加 `rigid_body` 节点和参考轮廓。

- **ArUco 单/多标记**：工具栏的方块标记按钮或 Tracks → “ArUco 刚体打标”。
  使用 `DICT_4X4_50`；第一帧检测到的标记定义刚体坐标系，同帧可见的其他标记
  自动映射进同一坐标系，之后用所有可见标记角点做 RANSAC 相似变换。
  标记需要在该帧清晰可见；单标记和同一参考帧内的多标记都可直接使用。
- **无标记 ORB**：工具栏的框选按钮或 Tracks → “无标记刚体（框选）”，在视频上
  拖一个矩形。ROI 内 ORB 特征作为参考，后续帧用 BFMatcher + ratio test +
  `estimateAffinePartial2D`(RANSAC) 跟踪，并用预测区域掩膜排除背景；内点不足或
  尺度/角度跳变会跳过该帧。适合有纹理的刚体；白球、纯色/光滑物体请用标记。
  当前固定使用参考帧特征，长序列可能有轻微漂移；后续可加参考帧刷新。
- **运动轮廓刚体**：工具栏的轮廓按钮或 Tracks → “运动轮廓刚体（框选）”。
  Rust 默认使用 KNN 运动分割，在参考帧附近学习 50 帧的固定 canonical mask，
  之后用位姿变换后的掩膜 IoU 匹配；角度在 θ/θ+π 两个真实峰上用弱的连续性
  代价选分支，避免对称杆的 180° 跳变。Tracks 菜单可勾选
  “纯运动掩膜（不学习固定形状）”，直接使用每帧运动轮廓（点跟踪，
  遮挡时可能跟到夹具）。阴影不做特殊处理（可能被算进轮廓）。方法对比、
  角度误差与不确定度的本地实验记录见 `docs/motion-segmentation.md`
  （该目录未纳入版本控制）。
- 属性面板显示刚体角度、特征尺寸和质量（内点/匹配度/标记数）；绘图可在 Y 轴
  选择“角度”。
- 合成测试：`cargo run --example synth_aruco --features vision` 生成
  `target/aruco_single.mp4` 与 `target/aruco_multi.mp4`；`cargo run --example
  synth_motion --features vision` 生成带柔和阴影、中途停顿的深色长条
  `target/motion_rod.mp4`（均为 MPEG-4 编码）。
- 测试钩子：`PROBE_RIGID=1`；`PROBE_ORB=1 PROBE_ORB_ROI=x,y,w,h
  [PROBE_ORB_FRAME=n]`；`PROBE_MOTION=1 PROBE_MOTION_ROI=x,y,w,h
  [PROBE_MOTION_FRAME=n]`。

### FFmpeg 裁剪（GitHub Actions）

当前完整 Windows 共享构建中 `avcodec-63.dll` 约 87MB，加上 `avformat/avutil/
swscale/swresample` 合计约 113MB；其中编码器、muxer、filter、网络协议和大量
未使用 codec 对本项目没有意义。仓库准备了最小化构建：

- 配置：`scripts/configure-ffmpeg-minimal.sh`
  - 只保留 file 协议、mov/matroska/avi/flv/asf/mpeg/mpegts/mpegvideo/ogg demuxer；
  - 只保留 h264/hevc/mpeg4/mpeg1-2/mjpeg/prores/theora/vp8/vp9/av1/flv/h263/
    wmv/vc1/msmpeg4/dv/ffv1/huffyuv/utvideo/dnxhd/rawvideo 解码器；
  - 保留 h264/hevc annexb 和 mpeg4 unpack bitstream filter；
  - 关闭 encoders/muxers/filters/avdevice/avfilter/programs/network/hwaccels/
    gpl/nonfree。
- 构建：`.github/workflows/build.yml`（每次 push 自动运行）
  - Windows x86_64：Ubuntu 上 MinGW 交叉编译，生成 DLL/头文件，并用
    `scripts/make-msvc-import-libs.py` 生成 MSVC `.lib`；Windows app job
    再下载 OpenCV 4.12、以 `--features vision` 编译 release 应用，并把
    `opencv_world4120.dll` 与许可证一起打包。
  - macOS：Apple Silicon 与 Intel 分别构建，dylib 安装名为 `@rpath`，并在同
    job 内打包成 `Tracker GPUI.app`；当前 macOS 构建尚未启用 OpenCV/vision。
  - **FFmpeg 缓存**：`prefix/` 以 FFmpeg 版本 + 裁剪脚本 hash +
    `FFMPEG_CACHE_VERSION` 为 key；配置不变时直接复用，只有裁剪配置/版本变化
    才重新编译。**Rust 缓存**：`Swatinem/rust-cache` 缓存 Cargo registry 与 target。
  - 每次 push 都产出应用 artifacts：`tracker-gpui-windows-x86_64`、
    `tracker-gpui-macos-arm64`、`tracker-gpui-macos-x86_64`。Windows 的
    `ffmpeg-windows-x86_64` 开发包同时作为 app job 的构建输入，每次运行都会上传；
    macOS 开发包仅在 cache miss 时上传。**不创建 GitHub Release**。
- 使用 FFmpeg 开发包：下载对应 `ffmpeg-*` artifact 解压到任意目录，把该目录设为 `FFMPEG_DIR` 再构建。
  macOS 运行时需要把 `$FFMPEG_DIR/lib` 加入 rpath 或
  `DYLD_FALLBACK_LIBRARY_PATH`；正式分发时应把 dylib 放进 App bundle 的
  Frameworks。
- release profile 使用 thin LTO、`codegen-units=1`、strip symbols；
  Windows release 设为 GUI subsystem，不弹控制台。
- 该工作流尚未在 GitHub 上实跑，首次运行可能需要按 Actions 日志调整交叉编译/
  导入库细节；本地不编译 FFmpeg。

## 代码结构

`main.rs` 只保留核心类型定义（`State`/`DocState`、工具与选择枚举、`Probe`）和启动逻辑，
其余按 UI 职责拆分：

| 模块 | 职责 |
|---|---|
| `state.rs` | `State`/`DocState` 的行为：文档、帧、编辑历史、AutoTracker 轮询、保存状态 |
| `app.rs` | `Probe::new`、测试钩子、打开/保存项目与视频 |
| `render.rs` | 主窗口布局、键盘/鼠标事件、左右面板与底部状态栏 |
| `workspace.rs` | 文件树、标签栏、视频传输控制、折叠边条 |
| `canvas.rs` | 视频画布叠加层：坐标轴、标尺、测量工具、轨迹与跟踪遮罩 |
| `plot_view.rs` | 右侧绘图分区与曲线绘制 |
| `menus.rs` / `modals.rs` | 菜单栏、右键弹出菜单、标定/跟踪/操作说明对话框 |
| `widgets.rs` / `icons.rs` / `theme.rs` | 通用控件、内联 SVG 图标、浅色/深色主题 |
| `coords.rs` / `track.rs` / `tracking.rs` / `plot.rs` / `project.rs` / `video.rs` | 与 UI 无关的数据与算法层 |

## 先看结论

| 项目 | 结果 |
|---|---|
| 1080p 30fps 播放 | **320 帧 / 10.4 s ≈ 30.8 fps**，渲染 60 fps，全程 **1 次 seek** |
| 随机跳转（关键帧对齐） | **约 45–85 ms** |
| 后退一帧（跨 GOP） | 约 390 ms（回到关键帧再解码；与 Java 关键帧回退模型一致） |
| 解码 + 转 BGRA | **约 3–12 ms/帧**（16 帧线程，1080p，debug） |
| 纹理回收 | 已验证；每帧 `drop_image` 退役旧图，atlas 不再无界增长 |

> 旧版 MF 探针实测过 1080p 8 秒 **0 帧接受 / 204 次 seek**。根因是解码线程
> 无节制向前跑、UI 只保留最新 16 帧、按墙钟目标每帧 seek，以及每帧新建纹理
> 却从不调用 `drop_image`。现在这两处都已按 Java 模型重写。

## 工作区

- **菜单栏**：File（新建/打开项目/打开文件夹/保存/另存为/最近项目/关闭标签）、
  Edit（撤销/重做）、View（适应窗口、主题、显示坐标系、显示轨迹、步号、
  轨迹线）、Tracks（轨道管理/自动步进/AutoTracker/新建测量轨）、Coords（标定）、
  Help（操作说明/关于）。
- **左侧**：目录名右侧的刷新图标重新扫描工作区，旁边的 `‹` 收起工作区；
  目录通过 File → 打开文件夹选择。扫描深度 4，跳过隐藏目录、`target`、
  `node_modules`、`vendor`、`third_party`，只列出视频扩展名；文件夹可展开，
  视频单行显示，超出宽度用省略号。收起后保留一条 24px 窄条，点击箭头恢复。
- **右侧栏**：顶部“检查器”标题带 `›` 收起按钮；收起后右侧保留窄条，点击箭头恢复。
  View 菜单的“显示工作区/显示属性栏”也可切换，菜单项带 `[x]` 状态。
- **中间**：每个视频一个标签页，可同时打开多个、点击切换、点 `×` 关闭；
  文件名过长时标签有最大宽度并省略。标签栏可用滚轮横向滚动。切换标签会暂停
  其他视频。未保存的标签显示 `*`。
- **视频组件**：视频画布 + 播放进度条 + 传输按钮。进度条可点击/拖动跳转；
  首帧/逐帧/播放/末帧按钮均为自绘 SVG 图标并带 tooltip；打开新视频时显示索引进度。
- **绘图**：右侧“绘图+数据”页签中，按当前质点轨道绘制数据曲线（默认 2 幅，
  x-t/y-t）和完整数据表。坐标轴标签直接标在横轴/纵轴旁（横轴右下、纵轴顶部），
  不再使用 “A vs B” 标题，绘图边距收紧。可切换数据变量、1/2 幅、点/线、同步 X；
  图表始终自动缩放占满区域，不拦截滚轮（滚轮用于滚动右侧栏）。点击图上的
  数据点跳到对应帧。数据表默认列为 t/x/y，可用复选框增删 vx/vy；点击行选择，
  支持全选/清除/复制（复制选中行，未选择时复制全部，Ctrl+C 同效）。绘图和数据
  表只针对质点轨道，当前轨道是测量轨时显示提示。绘图配置随 `.tgp` 保存。
- **右侧**：顶部页签在“对象属性”和“绘图+数据”之间切换。对象属性页含文件信息、
  轨道列表、选中对象属性；绘图页含曲线和当前帧数据表（frame / t / x / y / vx / vy）。
  各分区标题可点击折叠/展开，内容区纵向滚动；测量轨激活时绘图页显示提示。
- **编辑历史**：每个标签独立 Undo/Redo；打点、删点、标定、轨道属性和一次完整
  AutoTracker 运行都可撤销。
- **坐标系统**：`src/coords.rs` 对齐 OSP `ImageCoordSystem`，支持逐帧 origin/scale/angle、
  图像↔世界坐标、固定原点/角度、锁定坐标。比例只由定标尺控制（不再提供“固定
  比例”）。锁定坐标会阻止原点/角度手柄改动，但不影响选中对象、也不锁定定标尺；
  锁定状态下仍可拖动标尺端点或编辑长度。Coords 菜单可切换固定/锁定、对齐到
  标定尺、统一当前帧坐标、重置。
- **标定**：支持多把定标尺，每把有独立世界长度；所有标尺共享比例，拖动任意
  一把的端点会按各自长度自动调整其他标尺的端点，但不会旋转坐标轴。坐标原点
  和角度完全由坐标轴手柄独立控制。标尺/测量尺绘制主次刻度、端点刻线，主刻度
  带长度标签、小刻度不带，两端不画圆点。点击工具栏标尺按钮即新建标尺；选中
  标尺后在右侧“对象属性”查看长度、显示/隐藏并编辑长度。
- **轨道**：`src/track.rs` 的 `Track` + `TrackKind::PointMass`，每个 track 有稳定 id、
  名称、颜色、可见/锁定、step size、Autostep 和 footprint（菱形/三角形/方框/圆/十字，
  默认菱形）。Tracks 菜单可新建/删除轨道、清空数据、改步长/标记形状；右侧显示每个
  点的世界坐标与速度。点绘制对齐原 Tracker：默认显示当前帧往前 15 帧的长轨迹，
  每个可见 step 在点左上方显示帧号，当前帧点外加 12px 圆圈高亮；按住 Shift
  进入标记状态时只显示当前帧的点，释放后恢复。View 菜单可切换轨迹、步号和
  轨迹线（默认关闭轨迹线）。
- **AutoTracker**：`src/tracking.rs` 的 `TrackingBackend` trait + OSP 模板匹配
  baseline。从当前帧主动轨迹点取模板，按 step size 向后逐帧搜索；独立 FFmpeg
  decoder 在后台读帧，不抢 UI 播放。支持亚像素匹配、速度/加速度预测、模板
  evolve/tether、良好/可能/丢失阈值、暂停/继续、单步搜索和从当前帧重跟踪；
  失败或结尾自动停止，一次运行只产生一个 undo 记录。AutoTracker 现在打开为
  独立 OS 窗口，设置为主窗口的 owned window（始终位于主窗口上方但不阻塞主
  窗口）。覆盖层按原版区分：关键帧显示目标十字、实线掩膜和角点，非关键帧显示
  点线匹配区域；搜索区域始终为虚线，角点为实心方块。
- **Autostep**：每个轨道独立开关；手动新建点后自动前进 `step_size` 帧（A 键或
  Header/Tracks 菜单切换）。
- **项目**：自有 `.tgp` JSON 格式，保存视频路径、轨道（含步长/可见/锁定/形状）、
  标定、当前帧和视图状态；关闭有未保存修改的标签会提示保存/放弃/取消。
  `PROBE_LOAD/PROBE_SAVE` 可脚本化验证。
- **视频工具栏**：视频区域左侧的竖向图标栏。默认是移动/选择工具，任意工具下
  按住 Shift 都会显示十字光标，`Shift+单击`放置点，`T`/`Esc` 清除工具选择回到
  移动状态。`标定`组左键单击直接新建一把定标尺，右键选择“设定坐标原点/原点
  偏移”。`测量`组右键选择测量尺/量角器/圆拟合。移动按钮旁是质点轨道按钮，
  点击列出当前所有质点轨道并切换打点目标，菜单顶部可直接新建质点。之后是 Autostep、AutoTracker 和
  坐标系显示开关；AutoTracker 按钮打开独立设置窗口。所有按钮悬停显示
  tooltip，图标按原 Tracker 的位图重绘为内嵌 SVG。
- **对象选择**：点击坐标轴、定标尺、测量尺或轨迹点即可选中，坐标锁定后仍可
  选中对象；选中对象在右侧“对象属性”中显示属性。布尔属性（显示/锁定/固定
  原点/固定角度）使用复选框；原点、角度、标尺长度等数值点击后可直接键入，
  Enter 提交、Esc 取消。拖动对象主体可平移，拖动标尺/测量尺端点可调整几何。
  `Delete`/`Backspace` 删除选中的标尺、测量轨或轨迹（坐标轴不可删除）；未选中
  对象时删除当前帧的点。点击空白取消选择。平移视频改为 `Ctrl+滚轮` 或
  `Alt+滚轮`，左键拖动不再平移视频。
- **主题**：View → 跟随系统主题 / 浅色模式 / 深色模式；跟随系统时每 5 秒检查
  Windows 应用主题并自动刷新颜色。选择会保存到用户偏好。
- **坐标系与原点**：显示坐标轴后，可直接拖动原点标记设置原点，拖动 x 轴上的
  方形手柄旋转坐标轴；同时保留“设定坐标原点”和“原点偏移”单击/拖动工具。
  坐标轴按当前坐标系统显示 x/y 轴、方向箭头、主次刻度和主刻度坐标标签。
- **测量工具**：测量尺（长度/角度）、量角器（夹角/两臂长度）、圆拟合（三点
  外接圆的圆心/半径）；拖动白色手柄调整，结果显示在右侧“测量”分区。测量轨
  也出现在轨道列表中，支持清空/删除和项目保存。

## 跟踪模型

- 帧号直接作为 step 键：`PointMass` 内部是 `BTreeMap<frame, PointMassStep>`，
  天然稀疏、按帧有序；`step_size` 表示自动步进/自动跟踪的帧间隔。
- 世界坐标位置/速度/加速度由 `Track::world_position/velocity/acceleration` 计算；
  速度用相邻轨迹点的中心差分，因此数据表在稀疏步长下仍是世界单位/秒。
- 模板匹配参数默认对齐 OSP：mask 18×18、search 40×40、goodMatch 4、
  possibleMatch 1、evolve 20%、tether 5%。AutoTracker 独立窗口可修改并用于
  下一次运行，支持搜索策略 Look-Ahead/Follow/Fixed、停止条件、仅 X 轴、
  暂停/继续、单步搜索和从当前帧重跟踪。
- 当前 backend 只做平移跟踪（不做旋转/尺度变化）；TAPNext++/SAM2/CSRT 等后端
  可通过 `TrackingBackend` 接口在后续阶段接入。

`.trk/.trz` **不兼容**，本项目使用自己的 `.tgp` 格式（版本化 JSON，视频以路径引用）。

## 视频层架构

`src/video.rs` 对应 Java 的 `XuggleVideo` + `StepperClipControl`：

1. **打开时建索引**：扫描容器里的视频 packet，按 PTS 排序，记录每帧时间戳和
   关键帧标记；`frame_times` 用于 VFR/变速播放，`keyframes` 用于跳转。
2. **按需解码**：UI 每次只请求一个目标帧。目标在前方且距离不大时顺序解码；
   后退或跨 GOP 时 `avformat_seek_file` 到关键帧再向前解码到目标。
3. **最新请求覆盖**：请求槽只保留最新目标；解码过程中发现新目标会中止当前
   解码，避免旧 seek 堆积。
4. **一帧预取 + 4 帧缓存**：播放时提前解下一帧，按 `frame_times` 到点显示；
   解码慢时逐帧慢放，不跳帧。最近 4 帧缓存用于快速前后微调。
5. **显示与纹理**：swscale 输出 BGRA，UI 直接交给 `RenderImage`。替换帧图时
   旧纹理进入 3 帧退役队列，之后由 `Window::drop_image` 释放；关闭标签页时
   其纹理也统一交给渲染线程回收。

## 构建

项目通过 `rust-toolchain.toml` 固定使用 Rust stable 1.99.1，Windows/macOS CI
也使用相同版本。需要安装 rustup、FFmpeg 9 shared 开发包，以及 `libclang`（`ffmpeg-sys-the-third`
的 bindgen 需要）。平台差异由脚本处理，`.cargo/config.toml` 不再写死机器路径。

**Windows**

```powershell
# FFmpeg 解压到 third_party/ffmpeg-n9.0-latest-win64-lgpl-shared-9.0
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 run
```

**macOS**

```bash
brew install ffmpeg pkg-config
./scripts/build-macos.sh run
```

`build.rs` 只会把实际使用的 `avcodec/avformat/avutil/swscale/swresample` DLL
复制到 `target/<profile>/`，不再复制 avdevice/avfilter。使用 CI 下载的 FFmpeg
artifact 时，先解压并把该目录设为 `FFMPEG_DIR`。

`gpui-kit` 0.7.3 及其依赖直接从 crates.io 获取，版本由 `Cargo.lock` 锁定。
旧版 Rust 1.91.1 所需的三个兼容补丁已移除；`slice::as_array`、
`hint::cold_path`、`AtomicU64::try_update` 均已在当前工具链稳定，项目不再维护
这些依赖的 vendor 副本。首次编译（含 gpui 依赖树）约 10 分钟；之后应用增量
约 10–20 秒。

安装 rustup 后，在项目目录运行 `cargo` 会自动安装固定的工具链及
Clippy/rustfmt。验证命令（Windows）：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 check --locked
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 test --locked --bin tracker-gpui
```

### 脚本化测试

以下环境变量示例需在已配置 FFMPEG 的构建会话中运行；Windows 用
`powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 run`、
macOS 用 `./scripts/build-macos.sh run` 代替 `cargo run`。

```bash
PROBE_DIR="D:/dfshfghj/tracker/examples" cargo run
PROBE_OPEN="D:/videos/a.mp4;D:/videos/b.mp4" PROBE_PLAY=1 cargo run
PROBE_VIDEO="D:/videos/a.mp4" PROBE_TRACE=1 PROBE_SEEK="30,300,299,600,0,150" cargo run
PROBE_OPEN="D:/videos/a.mp4;D:/videos/b.mp4" PROBE_SAVE="D:/tmp/test.tgp" cargo run
PROBE_LOAD="D:/tmp/test.tgp" PROBE_PLAY=1 cargo run
PROBE_MENU=file cargo run
PROBE_CAL_DIALOG=1 cargo run
PROBE_TRACKER_DIALOG=1 cargo run
PROBE_VIDEO="D:/tmp/mover.mp4" PROBE_AUTOTRACK="94,114" PROBE_TRACK_TRACE=1 cargo run
PROBE_VIDEO="D:/tmp/mover.mp4" PROBE_AUTOTRACK="94,114" PROBE_SAVE="D:/tmp/a.tgp" cargo run
PROBE_LOAD="D:/tmp/a.tgp" PROBE_INSPECT=1 cargo run
PROBE_VIDEO="D:/tmp/mover.mp4" PROBE_MEASURE=1 PROBE_SAVE="D:/tmp/m.tgp" cargo run
PROBE_VIDEO="D:/tmp/mover.mp4" PROBE_THEME=light cargo run
PROBE_VIDEO="D:/tmp/mover.mp4" PROBE_MEASURE=1 PROBE_PLOT=1 cargo run
```

- `PROBE_DIR`：启动时设置工作目录并扫描文件树。
- `PROBE_OPEN`：按真实加载路径打开视频，分号分隔多个文件，会生成多个标签页。
- `PROBE_VIDEO`：同步打开单个视频（绕过加载进度，适合性能测试）。
- `PROBE_LOAD`：启动即加载 `.tgp` 项目。
- `PROBE_SAVE`：加载完成后保存 `.tgp` 项目（测试用）。
- `PROBE_MENU`：启动即展开 File/Edit/View/Tracks/Coords/Help 菜单（截图/调试用）。
- `PROBE_CAL_DIALOG`：启动即打开标定设置对话框。
- `PROBE_TRACKER_DIALOG`：启动即打开独立 AutoTracker 窗口。
- `PROBE_CAL_TEST`：自动应用 0.5 cm 标定并打印 scale/origin（回归测试用）。
- `PROBE_EDIT`：自动打点并执行 Undo/Redo，打印 dirty 状态（回归测试用）。
- `PROBE_AUTOTRACK="x,y"`：在帧 0 标记该图像坐标并启动 AutoTracker，完成后打印
  采样点/状态并退出；配合 `PROBE_SAVE` 保存、`PROBE_AUTOTRACK_UNDO=1` 验证单次撤销。
- `PROBE_TRACK_TRACE=1`：打印 AutoTracker 每帧的预测位置、匹配位置、置信度和质量。
- `PROBE_INSPECT=1`：加载项目后打印轨道元数据、测量轨和首尾点（配合 `PROBE_LOAD`）。
- `PROBE_TRAIL=1`：生成 16 个手打点并停在帧 16，用于验证长轨迹、帧号标签和当前点圆圈。
- `PROBE_TRACKS_MENU=1`：创建多个质点轨和一条测量轨，展开工具栏质点轨道菜单（回归测试用）。
- `PROBE_OWNER_CHECK=1`：打印 AutoTracker 窗口的 native owner HWND 校验结果。
- `PROBE_MEASURE=1`：创建测量尺/量角器/圆拟合各一个并打开坐标轴，打印几何结果；
  配合 `PROBE_SAVE` 可验证测量轨持久化。
- `PROBE_THEME=light|dark|auto`：覆盖主题偏好（测试/截图用）。
- `PROBE_PLOT=1`：启动后展开右侧绘图分区，打印当前轨道和绘图配置（回归测试用）。
- `PROBE_WINDOW_SIZE=1200x700`：覆盖默认窗口大小（小屏幕/截图用）。
- `PROBE_PLAY`：加载完成后播放当前标签页。
- `PROBE_SEEK`：加载后按 700ms 间隔跳到指定帧，用于回归测试。
- `PROBE_TRACE=1`：打印索引、解码耗时、请求/seek/UI 接受帧、渲染 FPS。
- 测量 FPS 时请让窗口保持前台；Windows 会节流被遮挡的窗口（实测可从 60 fps
  降到 ~23 fps）。

## 操作

| 操作 | 效果 |
|---|---|
| File 菜单 | 新建/打开/保存/另存为/最近项目/关闭标签 |
| Edit 菜单 / Ctrl+Z | 撤销 |
| Ctrl+Shift+Z / Ctrl+Y | 重做 |
| 左侧点击视频 | 在新标签页打开（已打开则切到该标签页） |
| 左侧刷新图标 | 重新扫描当前目录 |
| 标签页 × | 关闭视频（有修改时提示保存） |
| T / Esc | 回到移动/选择工具；打点需 Shift+单击 |
| 视频左侧工具栏 标定 | 左键新建定标尺，右键打开坐标原点工具 |
| 点击坐标轴/标尺/测量尺/轨迹点 | 选中对象，右侧显示对象属性 |
| 拖动对象主体 | 平移选中的坐标轴/标尺/测量尺 |
| 拖动标尺/测量尺端点 | 调整长度/几何；其他标尺按各自长度自动跟随 |
| 拖动坐标轴原点/方形手柄 | 移动坐标原点/旋转坐标轴（需显示坐标系） |
| 测量组右键 | 测量尺/量角器/圆拟合 |
| 测量工具拖动 | 拖白色手柄调整测量几何；测量尺首次点击后拖动确定终点 |
| 视频左侧工具栏 质点轨道 | 点击列出质点轨道并切换当前打点目标 |
| 视频左侧工具栏 / A | 切换当前轨道 Autostep；新建点后自动前进 step size 帧 |
| 工具栏 AutoTracker / Tracks → AutoTracker… | 打开独立 AutoTracker 窗口；窗口内搜索/暂停/单步/修正 |
| View → 主题 | 跟随系统/浅色/深色 |
| View → 显示坐标系 | 开关坐标轴和刻度 |
| View → 显示轨迹/轨迹长度 | 开关长轨迹并切换 15 帧/4 帧/全部 |
| View → 显示步号 | 开关每个可见 step 的帧号标签 |
| View → 显示轨迹线 | 开关连接所有点的轨迹线（默认关闭） |
| 按住 Shift | 当前轨道只显示当前帧的点，隐藏之前的轨迹和标签 |
| 右侧栏 对象属性/绘图+数据 | 切换检查器页签；各页签独立内容 |
| 右侧 绘图 分区 | 展开/折叠；图表随右侧栏滚动 |
| 绘图分区 x/y 芯片 | 选择横轴/纵轴数据变量 |
| 绘图分区 1 图/2 图 | 切换绘图数量；“点/线/同步 X”使用复选框，同步 X 只对 2 图有效 |
| 数据表 列复选框 | 默认 t/x/y，可勾选 vx/vy |
| 数据表 点击行 | 选择/取消选择该行数据 |
| 数据表 全选/清除/复制 | 选择全部行、清除选择、复制选中行（无选择时复制全部） |
| Ctrl+C | 复制数据表当前选中行/全部可见列 |
| 绘图图上单击 | 跳到最近的数据点所在帧 |
| 标签栏滚轮 | 左右滚动标签页 |
| 左侧标题栏 `‹` / 检查器 `›` | 收起对应侧栏 |
| 折叠后的窄条箭头 | 重新展开侧栏 |
| View → 显示工作区/显示属性栏 | 切换左右侧栏（带 `[x]` 状态） |
| Help → 操作说明 | 查看快捷键与鼠标操作 |
| AutoTracker 窗口 | 修改 mask/search/阈值/evolve/tether、搜索策略与停止条件 |
| Tracks → 标记形状/步长 | 切换 footprint、调整自动步进/跟踪间隔 |
| 右侧对象属性 → 数值 | 点击编辑原点/角度/标尺长度，Enter 提交、Esc 取消 |
| 右侧对象属性 → 复选框 | 切换显示/锁定/固定原点/固定角度（定标尺不受锁定影响） |
| Delete / Backspace | 删除选中的标尺/测量轨/轨迹；坐标轴不可删除 |
| Coords → 固定/锁定 | 固定原点/固定角度、锁定坐标（比例由定标尺控制） |
| Coords → 对齐到标定尺 | 用主标尺重新设置坐标系统的比例/角度（不改原点） |
| 空格 | 播放 / 暂停 |
| ← / → | 逐帧（Shift 为 10 帧） |
| 进度条点击/拖动 | 跳到对应帧 |
| 滚轮 | 以光标为中心放大/缩小 |
| Ctrl+滚轮 / Alt+滚轮 | 平移视频 |
| 点击空白 | 取消对象选择 |
| Shift+左键点画面 | 在当前帧打点（避免误触） |
| 拖白色方块 | 微调当前帧的点 |
| F | 适应窗口 |
| Delete / Backspace（无选中对象） | 删除当前帧的点 |

## 测量

Windows 11 / i7-13700H / debug profile / FFmpeg 9 LGPL shared：

```
duet.mp4 1920×1080 30.03fps 609 帧
播放 10.4s：320 帧（30.8 fps），渲染 60 fps，1 次 seek
随机 seek：300 → 46ms；600 → 68ms；0 → 84ms；150 → 45ms
后退 300 → 299：387ms（关键帧 270，解 30 帧）
解码+转 BGRA：3–12ms/帧（16 线程）
```

截图：`docs/shot-menu.png`（工作区/File 菜单）、`docs/shot-coords-menu.png`（坐标菜单）、
`docs/shot-calibration.png`（标定设置）、`docs/shot-trunc.png`（长文件名截断）。

## 踩坑记录

1. **`request_animation_frame()` 不能在事件回调里调用。**
   它内部会调 `window.current_view()`，后者断言只能发生在 request_layout / prepaint /
   paint 阶段。在 `on_key_down` 里调用会直接 panic。事件里只 `cx.notify()`；
   连续动画在 `render()` 里请求下一帧。

2. **空轨道会越界。**
   打开视频后到索引完成前，标签页存在但 `tracks` 为空；所有 `tracks[active]`
   必须用 `.get()`，绘制手柄、点击打点、数据表、右侧轨道都要能处理空轨道。

3. **视频帧不能用“新 ImageId + 永不 drop”。**
   `RenderImage::new` 每次分配新 ID，`paint_image` 把它插进 sprite atlas；
   不调用 `drop_image` 会按 8.3 MB/帧增长。现在用 3 帧退役队列回收。

4. **播放时钟不能直接追墙钟。**
   解码慢时不能每帧 `seek` 去追；应该按 `frame_times` 判断“下一帧到点没有”，
   没到点就等，到了才请求/显示。解码慢就慢放，和 Tracker 的 Java 实现一致。

5. **FFmpeg 导入库要 MSVC 版本。**
   BtbN 的 `*-shared` 包同时带 `.lib` 和 `.dll.a`；MSVC 用 `.lib`。DLL 必须和 exe
   同目录（`build.rs` 已处理），否则运行时报 `STATUS_DLL_NOT_FOUND`。

6. **树和标签页必须单行截断。**
   长文件名会换行撑高行/标签；给名称容器 `min_w(0) + truncate()`，标签再加
   `max_w`，否则布局会被长文件名破坏。

## 尚未做

- 相机标定/去畸变、平面透视矫正、AprilTag 字典（当前 ArUco 固定 4×4_50）。
- 无标记多刚体、自动候选框、长序列参考帧刷新（当前 ORB 固定参考帧）。
- 运动轮廓的阴影/同色背景/大遮挡处理；模板回退目前只做平移（角度/尺度保持）。
- AI 目标检测（RTMDet/YOLOX + ByteTrack/OC-SORT）与移动机位检测。
- 光流、PTV/PIV、碰撞/事件检测、多相机 3D 三角化。
- macOS 的 OpenCV 动态库打包与 `vision` feature、公证。
- 硬件解码（FFmpeg d3d11va）；当前是 16 线程软解，1080p 30fps 已够用。
- 视频滤镜栈、音频、摄像头录制、视频导出。
- Tabset、标签右键菜单/重排、完整偏好设置对话框、关闭应用时的未保存确认。
- 目录扫描目前同步执行；超大目录可以移到后台线程并显示进度。

详细清单见 [TODO.md](TODO.md)。

## 许可

本项目以 GNU General Public License v3.0 或更高版本发布，详见 [LICENSE](LICENSE)。
Windows 发行包内的 OpenCV 以 Apache-2.0 许可使用，FFmpeg 以 LGPL 2.1+ 使用，
对应许可证随包附带。
