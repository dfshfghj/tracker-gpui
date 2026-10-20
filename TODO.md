# Tracker GPUI 重写 TODO

更新：2026-10-20
目标：在 GPUI 探针上按原 Tracker 的功能逐步对齐；本阶段以功能对齐为主，不追求额外增强。

## 原则

- Rust 核心与 GPUI UI 分离；核心模型不依赖 UI。
- 所有修改走 Command/Event 总线，Undo/Redo 从第一天可用。
- 为未来 Python 脚本/插件预留接口，但本阶段不实现 Python。
- 不做 JS/EJS 扩展；原 EJS 数据模型后续用原生表达式或 Python 替代。
- 视频功能本阶段不扩展，只保持现有的 FFmpeg 播放/逐帧/seek/帧访问可用。
- 每个阶段有可验收的闭环，避免只做 UI 不做数据。

## 当前已落地

- [x] FFmpeg 解码：索引、按需解码、seek、1080p30、纹理回收
- [x] 工作区骨架：目录树、视频多标签、右侧属性/数据表
- [x] 视频组件：播放/暂停/逐帧/进度条 seek、加载进度
- [x] 基础打点、标定尺、缩放/平移
- [x] 空轨道保护和长文件名/Tab 截断
- [x] Track/PointMass 模型、轨道菜单、footprint、Autostep
- [x] AutoTracker baseline（OSP TemplateMatcher）+ `TrackingBackend` 接口
- [x] 浅色/深色/跟随系统主题；播放控制 SVG 图标
- [x] 视频左侧工具栏、标定/测量分组右键选项、右侧分区折叠
- [x] 坐标轴显示、设定原点/拖动原点偏移
- [x] 测量尺、量角器、圆拟合（基础版，多 handle 测量轨）
- [x] Plot 绘图：数据变量、1/2 图、点/线、始终自动缩放、点击跳帧、
      同步 X、`.tgp` 保存绘图配置；已迁移到右侧可折叠分区
- [x] 右侧栏纵向滚动；视频加载后自动适应窗口
- [x] 打点改为 Shift+单击，弹窗/菜单关闭不会误触视频；移除工具栏打点按钮（T/Esc 回到打点）
- [x] AutoTracker 跟踪时在视频上显示 mask 圆框和 search 区域角框
- [x] 多定标尺（独立长度、联动端点）、标尺主次刻度/端点刻线/主刻度标签
- [x] 坐标轴原点与角度手柄拖动；坐标轴与标定尺完全解耦（标尺只影响比例）
- [x] 绘图侧栏始终自动缩放、不拦截滚轮；标签栏滚轮横向滚动；滚轮缩放方向修正
- [x] Shift 在任意工具下可打点，并切换为十字光标
- [x] 坐标轴/标尺/测量尺作为可选中对象：点击选中、右侧对象属性、拖动平移、显示/隐藏、长度编辑
- [x] 右侧移除常驻的测量/坐标/标定分区；标尺长度/切换菜单移除
- [x] 视频左键拖动平移移除，改为 Ctrl/Alt+滚轮平移
- [x] 轴线刻度间距加大，标尺末尾刻度与末端标签避免重叠
- [x] 修复标签栏滚轮 panic（`max_offset().x` 为正时 clamp 上下界倒置，先钳到 ≤0）
- [x] 锁定坐标不再阻止选中对象；定标尺不再被坐标锁锁定，比例不再提供“固定”
- [x] 右侧布尔属性改复选框；原点/角度/标尺长度点击内联编辑（Enter 提交/Esc 取消）
- [x] Delete/Backspace 删除选中标尺/测量轨/轨迹，坐标轴受保护；无选中时删当前帧点
- [x] 左右侧栏收起/展开：面板头部按钮、折叠窄条、View 菜单同步（PROBE_COLLAPSE 冒烟）
- [x] `main.rs` 模块化：拆出 state/app/render/workspace/canvas/plot_view/menus/modals/widgets/icons/theme，根文件仅保留核心类型与启动
- [x] AutoTracker 改为独立 OS 窗口，并设置为主窗口 owned window（Win32 GWLP_HWNDPARENT，实测 owner 正确）
- [x] 点绘制对齐原版：默认 15 帧长轨迹、Shift 只显示当前步、每步帧号标签、当前点 12px 圆圈、默认菱形 footprint、轨迹线单独开关
- [x] AutoTracker 覆盖层对齐：关键帧目标十字/实线掩膜/角点，非关键帧点线匹配，虚线搜索框；移除覆盖层自带帧号
- [x] 工具栏新增质点轨道按钮：点击列出全部质点轨道并切换当前打点目标
- [x] 绘图和数据表仅针对质点轨道，测量轨激活时隐藏
- [x] 右侧栏拆成“对象属性”和“绘图+数据”两个可切换页签
- [x] 绘图边距收紧、坐标轴标签放到轴旁（去掉 vs 标题）
- [x] 数据表显示全部点，列复选框（默认 t/x/y）、行选择、全选/清除/复制与 Ctrl+C
- [x] 跨平台构建：Windows 脚本设置 FFMPEG_DIR/LIBCLANG，macOS Homebrew pkg-config；移除全局硬编码
- [x] AutoTracker 窗口置顶：Windows-owned HWND；macOS GPUI Floating 层级
- [x] macOS 自动主题检测（AppleInterfaceStyle）
- [x] 最小 FFmpeg 配置、Windows MSVC 导入库生成脚本与 GitHub Actions 工作流已准备
- [x] 包名/二进制从 tracker-gpui-probe 改为 tracker-gpui
- [x] release profile：thin LTO、codegen-units=1、strip；Windows release GUI subsystem
- [x] CI 每次 push 构建 Windows/macOS release 应用 artifact；FFmpeg/Rust 双缓存，仅 cache miss 上传 FFmpeg 开发包，不创建 Release
- [ ] 在 GitHub Actions 实跑构建，验证 Windows MSVC/macOS dylib 与 .app 产物

---

## 阶段 1：工作区外壳（基本完成）

### 1.1 文档与标签

- [x] 多标签：打开视频、切换、关闭
- [x] 标签脏标记（未保存）
- [x] 关闭标签时的保存/放弃/取消确认
- [x] 最近打开的视频/工作目录持久化
- [ ] 标签右键菜单（关闭、关闭其他、复制路径）
- [ ] Tab 重排（可选）
- [ ] Tabset 打开/保存（后续）

### 1.2 菜单 / 工具栏 / 状态栏

- [x] 菜单栏：File / Edit / View / Tracks / Coords / Help
- [x] 绘图位于右侧可折叠分区（取消中心视图切换）
- [x] 菜单项按当前文档状态动态启用/禁用（基础）
- [x] 状态栏：fps / draw / points / frame / time / track / tool / zoom / 跟踪状态
- [x] 错误提示/确认对话框统一封装（Modal）
- [x] Tracks 菜单（阶段 3）
- [x] 视频左侧竖向工具栏 + 标定/测量右键分组
- [x] 播放控制全部改为自绘 SVG 图标 + tooltip
- [x] 右侧属性/轨道/对象属性/数据表/绘图分区折叠
- [x] 右侧栏纵向滚动；视频加载后 fit；打点 Shift+单击；关闭弹窗/菜单的点击不穿透到视频
- [ ] Video / Window 菜单（随对应功能阶段补齐）
- [ ] Checkbox/Radio 菜单项（当前用 [x] 文本代替）
- [ ] 快捷键表与菜单显示同步（Help 操作说明已列出基础快捷键）

### 1.3 目录树

- [x] 选择工作目录、递归扫描视频、展开/折叠、打开视频
- [x] 刷新目录
- [x] 记住上次工作目录
- [ ] 键盘上下导航与 Enter 打开
- [ ] 文件变更监听（可选，`notify`）

### 1.4 配置与最近文件

- [x] 配置持久化到用户目录
- [x] 最近项目/目录列表
- [ ] 最小偏好设置对话框（默认工作目录、主题、字体、自动保存）
- [ ] 语言/单位/数字格式的配置入口（具体实现可在阶段 2/4）

### 1.5 命令与 Undo 骨架

- [x] `EditCommand` 枚举：打点、删点、设标定、重置标定
- [x] 命令执行入口 `apply_edit_command`（UI 和未来 Python 共用）
- [x] Undo/Redo 栈（快照）
- [x] 文档 dirty 状态与保存联动
- [ ] `Event` 枚举/事件总线（放到 Track/Step 阶段）
- [ ] 轨道增删、保存等命令化
- [ ] 拖拽手势统一成命令（当前用 transient command + 鼠标抬起 `commit_edit`）

### 1.6 项目生命周期

- [x] New / Open / Save / Save As / Close（标签）
- [x] 自有项目格式 `.tgp`（JSON，版本化）
- [x] 最小保存/加载闭环：视频路径、轨道、标定、当前帧、视图状态
- [x] 多标签项目保存/加载
- [ ] 关闭应用时的未保存确认
- [ ] Tabset

**阶段 1 验收**：选目录 → 打开两个视频 → 切换/关闭标签 → 菜单可用 → 改标定/打点 → Undo/Redo → 保存 `.tgp` → 重开恢复 → 无 panic。
2026-10-20 已实测：两标签保存/加载、dirty/undo/redo、File 菜单、无 panic。`.trk/.trz` 不兼容，使用自有 `.tgp`。

---

## 阶段 2：坐标系统（基本完成）

### 2.1 图像坐标系

- [x] 每帧 origin / scale / angle（`src/coords.rs`，对齐 OSP ImageCoordSystem）
- [x] 图像坐标 ↔ 世界坐标转换
- [x] 固定原点 / 固定角度 / 锁定坐标（Coords 菜单；比例由定标尺控制，不提供固定比例）
- [ ] 坐标系统与视频 Resize/Rotate/Perspective 滤镜联动（视频滤镜尚未实现）

### 2.2 标定工具

- [x] 标定尺：多标尺、各自实际长度；只计算比例/角度，坐标原点独立
- [x] 标尺主次刻度、端点刻线、主刻度 label；坐标轴原点/角度手柄拖动
- [x] 标定步骤；逐帧或固定标定（`fixed_coordinates`）
- [x] 标定可视化、拖拽、Undo
- [x] 标定设置对话框（长度/单位/小数位）
- [x] 单位与数字格式用于右侧数据表
- [ ] ReferenceFrame 基础设施（每轨道参考系，依赖 Track/Step）

### 2.3 依赖 Track/Step 的坐标工具

> 这些工具本身是轨道类型，放到阶段 3 开头实现。

- [x] 坐标轴显示（x/y 轴、箭头、主次刻度、主刻度标签；View/工具栏开关）
- [x] 直接拖动坐标轴原点与角度手柄（固定/锁定遵循 ImageCoordSystem）
- [x] 设定坐标原点（单击）
- [x] 原点偏移（拖动原点）
- [x] 多定标尺：各自世界长度、共享比例/角度、拖动一把时其他端点自动跟随；坐标轴与标尺解耦
- [ ] CoordAxes 作为可保存的独立轨道（当前为每个文档的 axes_visible 视图状态）
- [ ] ReferenceFrame 绑定到具体轨道
- [ ] 透视标定 / PerspectiveTrack（P2，较复杂）

**阶段 2 验收**：标定一段视频，世界坐标正确；保存/重开后保持；锁定/固定坐标行为正确。
2026-10-20 已实测：多标尺映射与联动、逐帧/固定、锁定、Coords 菜单、`.tgp` 保存/加载、单元测试通过。
2026-10-20 复核：固定原点/角度逐帧生效；锁定只限制原点/角度编辑，标尺仍可调；对象选择与内联编辑在锁定下可用。

---

## 阶段 3：Track/Step + PointMass + AutoTracker（基本完成）

### 3.1 Track/Step 模型重构（最高优先）

- [x] `Track` + `TrackKind::PointMass` + `PointMassStep` 数据结构
- [x] 帧号 ↔ step 映射（目前以帧号为键）、per-track stepSize
- [ ] 起始/结束帧、插值步长
- [ ] 通用数据列抽象（目前 x/y/vx/vy 在 UI/模型方法中计算）
- [x] Footprint、颜色、可见/锁定、选中、删除轨道
- [ ] Track 克隆、复制/粘贴（XML/JSON 对象）
- [x] 与 Undo/Command 集成（轨道属性修改提交快照）
- [x] 为未来 `TrackingBackend` 保留接口
- [x] 替换当前 `Track { points }` 临时模型

### 3.2 PointMass

- [x] 位置、速度、加速度数据列（右侧属性 + 数据表 vx/vy）
- [x] 标记/拖拽/删除当前帧点
- [x] 自动填充/步进时的数据更新
- [x] 数据表接入（基础；绘图未做）
- [x] 与标定/世界坐标联动

### 3.3 AutoTracker（baseline）

- [x] 移植 OSP TemplateMatcher：模板、椭圆 mask、搜索区域、RGB 平方差评分
- [x] 初始关键帧；LOOK_AHEAD 速度/加速度预测
- [ ] 多关键帧/重定位与 FIXED/FOLLOW 搜索区域策略
- [x] 模板演化（evolveAlpha）、tether、抛物线亚像素
- [x] 自动追踪、停止、丢失/错误处理
- [ ] 暂停/续跟；人工修正后从当前帧重新跟踪
- [x] 进度显示与单次 Undo
- [x] 抽取 `TrackingBackend` trait
- [x] `TemplateMatchBackend` 作为第一个实现
- [ ] TAPNext++/SAM2/CSRT 等未来后端（阶段 5 接 Python 或原生库）

### 3.4 自动步进 / 标记

- [x] 自动步进：手动新建点后按当前轨道 step size 前进（A 键/菜单/Header）
- [ ] AutoMark：播放时自动标记并暂停/继续
- [ ] 独立标记条时间轴
- [x] 当前帧点高亮（footprint）、删除点
- [x] 与 AutoTracker、Undo 联动；播放控制联动仅自动暂停

**阶段 3 验收**：在样例视频上手动打点 → AutoTracker 跟踪 → 自动步进 → 人工修正 → Undo → 保存/重开；结果与原 Java 版对比误差可接受。

2026-10-20 已验证：640×480/90 帧合成移动目标，种子 (94,114)，AutoTracker 跟踪 89 点；
f10/f30/f60/f89 位置约 (135,165)/(215,113)/(335,113)/(451,101)，与真实目标误差约 1 px；
一次 Undo 后回到仅种子点；保存/重开后 90 点及首尾位置一致。尚未做暂停续跟与轨迹平滑对比。

---

## 阶段 4：剩余 Tracker 功能（后续）

### 4.1 其他轨道/测量

- [ ] CenterOfMass
- [ ] Vector / VectorSum
- [ ] LineProfile
- [ ] RGBRegion
- [x] TapeMeasure（基础：长度/角度、像素长度、世界单位、拖拽、保存）
- [x] Protractor（基础：0–180° 夹角、两臂长度、拖拽、保存）
- [x] CircleFitter（基础：3 手柄外接圆、圆心/半径、世界单位、保存）
- [ ] 测量轨的专用数据表/绘图、自动跟踪测量对象、更多 step 数据列
- [ ] PerspectiveTrack、ReferenceFrame
- [ ] 轨道属性、footprints、数据构建器

### 4.2 分析

- [ ] Butterworth / Savitzky-Golay / Moving Average
- [ ] 一阶/二阶导数
- [ ] 统计、数据列函数
- [ ] 数据导出（CSV/文本，分隔符/格式）

### 4.3 视图

- [x] Plot 基础（x/y 变量选择、1–2 幅、点/线、始终自动缩放、点击跳帧、同步 X；位于右侧栏，不拦截滚轮）
- [ ] Plot 视图完整对齐：3 幅、右键菜单、坐标范围对话框、零轴线、复制图片/打印
- [ ] Plot 多数据集/guest tracks、数据构建器联动、测量轨多变量曲线
- [ ] World 视图
- [ ] Page/Notes 视图（先 Markdown/文本，HTML/PDF 另议）
- [ ] 数据构建器/分析对话框

### 4.4 文件与导出

- [ ] `.trk` 完整兼容/升级
- [ ] `.trz` 项目包
- [ ] `.tabset`
- [ ] 图片序列导入/导出
- [ ] 视频导出（带叠加）
- [ ] 缩略图/打印
- [ ] 最近文件、URL、缓存、资源库

### 4.5 视频（本阶段暂缓）

- [ ] 视频滤镜栈（ghost/negative/grayscale/resize/rotate/perspective 等）
- [ ] 摄像头采集/录制
- [ ] 视频诊断/属性

---

## 阶段 5：Python 扩展（以后）

- 本阶段不实现，只保留架构入口。

- [ ] `tracker-core` 的 Command/Event API 稳定化
- [ ] `TrackingBackend` trait 稳定化
- [ ] FrameProvider / 共享内存接口
- [ ] Python sidecar（JSON-RPC + shared memory/Arrow）
- [ ] `tracker` Python 客户端包
- [ ] 插件 manifest、生命周期、依赖管理（uv/venv）
- [ ] 插件菜单/命令/事件
- [ ] 超时、取消、崩溃重启
- [ ] 图像处理插件：numpy/opencv-python 帧访问

**架构约束（现在就要遵守）**

- 所有修改通过命令；Python 不直接改 Rust 内存。
- 核心模型不依赖 GPUI；DTO 可序列化。
- Track/Step 有稳定 ID；Undo 可覆盖脚本操作。
- 帧访问接口为共享内存/批处理预留，不把 `Arc<RenderImage>` 暴露给脚本。
- 核心不依赖 Python；Python 缺失时应用完整可用。
- 不做 JS/EJS；EJS 数据模型后续由原生表达式或 Python 插件替代。

---

## 建议的模块拆分

当前探针是单 crate 单二进制；在阶段 1/3 之间建议拆成：

```
tracker-core   模型、坐标、Track/Step、AutoTracker、Command/Undo、IO
tracker-video  FFmpeg 解码、帧服务
tracker-ui     GPUI 工作区、视频组件、表格/绘图/对话框
tracker-script 未来的 Python host（暂不建）
```

先做 `src/core/` 模块化也可以，等核心稳定后再拆 crate。

## 已确认

1. `.trk/.trz` **不兼容**，使用自有 `.tgp`（JSON，版本化）。
2. 项目保存/加载放在阶段 1；已完成最小闭环和多标签保存/加载。

## 仍需确认 / 默认方案

3. 坐标系是否按原 Tracker 支持逐帧不同标定？（建议：是，对齐 ImageCoordSystem）
4. AutoTracker baseline 是否严格移植 OSP TemplateMatcher 的评分/搜索区域/模板演化行为？（建议：是，作为 baseline）
5. Page/Notes 和打印是否属于后续必需功能？（当前默认：阶段 4，先 Markdown/文本）

