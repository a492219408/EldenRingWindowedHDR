# Elden Ring Native Windowed HDR

这是一个由 ModEngine3 提前加载的 Rust DLL，用于在《艾尔登法环》App Ver. 1.17 的
窗口态 HDR。0.1.0 至 0.4.0 逐层确认了 10 位交换链、真正 HDR 菜单、内部请求和依赖独占
全屏的“实际 HDR 状态”门控。0.5.0 又把 PQ 色彩空间提交同步到对应 HDR 帧的 `Present`
之前；`20260830-215204-v050-borderless-hdr-pq-sync` 的首台 NVIDIA 实机测试中，开启后得到
正常 HDR，关闭后恢复正常 SDR，日志中的 PQ 与恢复调用也都成功。

0.6.0 进一步处理了启动持久化和普通窗口化：新模式会在严格窗口态候选检查通过后覆盖游戏的
公共 HDR 可用性判断，让游戏继续使用自身保存的 HDR 开关，而不是让 MOD 解析或改写
`ER0000.sl2/.co2`。2026-08-31 的六次实测确认：无边框和普通窗口化均能正常开启 HDR、
恢复 SDR，并能准确保留开启/关闭状态。0.6.1 是最终审计补丁，不改变上述成功路径。1.0.0
将正式名称改为 `Elden Ring Native Windowed HDR`，把已验证的 `windowed_hdr` 设为发布默认值，
并整理面向玩家的双语 TXT 文档、Nexus 素材和 GitHub Actions；核心 HDR 状态机未改变。

这仍不是不受条件限制的通用 HDR MOD。现有运行时证据来自一台 NVIDIA 测试机；维护者已
额外确认 Windows HDR 关闭时选项会保持不可用，以及窗口可在两台均已开启 HDR 的显示器间
移动或跨屏显示而不破坏 HDR。AMD、Intel、HDR/SDR 混合显示器、Windows HDR 热切换、
休眠恢复和其他 DXGI Hook 共存仍待验证。

## 当前能做什么

- 通过 EXE 导入表捕获 `CreateDXGIFactory` / `CreateDXGIFactory1`；
- 对 DXGI 工厂和交换链安装对象级影子虚表，并保持 Overlay 的 Hook 调用链；
- 记录交换链创建、`GetDesc` / `GetDesc1`、`Present` / `Present1`、
  `SetFullscreenState`、`ResizeBuffers` / `ResizeBuffers1`、`ResizeTarget`、
  `CheckColorSpaceSupport`、`SetColorSpace1` 和 `SetHDRMetaData`；
- 查询并记录 `IDXGIOutput6::GetDesc1` 的当前色彩空间、位深和亮度；
- 记录 AMD AGS 5.0.5 的 `agsSetDisplayMode` 参数与返回码；
- 观测真正 HDR 行的原始灰显判断，并记录最终返回给 UI 的结果；
- 记录真正 HDR 字段从 `MENU_OPTION_DATA +0x15` 提交到实时图形配置 `+0x1B` 的变化；
- 观测 `FUN_141E9F4D0` 如何从 `IDXGISwapChain::GetFullscreenState` 计算游戏内部“实际 HDR
  状态”；
- 在只解灰模式下仍完全保留该实际状态结果；
- 在另一个显式实验模式下，经 10 位格式、无边框、flip-model、PQ 输出和 PRESENT 支持等
  条件共同验证后，只模拟该内部状态结果，不修改 DXGI 的真实全屏状态；
- 在 0.5.0 的独立显式模式下，把 PQ 设置推迟到对应 HDR 帧的 `Present` 前，并在 HDR 关闭
  后于 SDR 帧 `Present` 前恢复此前观察到的颜色空间；
- 在 0.6.0 的 `windowed_hdr` 模式下，只对满足同一严格候选检查的无边框/普通窗口交换链
  覆盖公共 HDR 可用性判断，避免设置页初始化把游戏已载入的 HDR 请求归一化为关闭；
- 启动恢复时以游戏后端自己的已确认请求位为准，不创建第二份 HDR 状态，也不打开、解析或
  修改游戏存档；
- 只接受已分析的 `eldenring.exe` 指纹，版本不符时在安装 Hook 前安全终止初始化。

目标 EXE 的固定指纹为：

```text
大小：87,024,720 字节
SHA-256：D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134
```

## 运行模式

`EldenRingWindowedHDR.ini` 的发布默认配置为：

```ini
[HDR]
mode = windowed_hdr
```

`windowed_hdr` 是正常用户功能模式。它继承 `emulate_hdr_and_set_pq` 的内部状态与 Present
同步 PQ 状态机，但不再强行改写 HDR 行的最终灰显结果。它改为 Hook 真实公共可用性函数
`FUN_140953A10`：原生全屏结果始终透传；原生结果为假时，只有活动交换链满足 10 位、窗口态、
flip-model、PQ 输出和 `PRESENT` 支持才返回可用。该函数的两个已确认直接调用者同时负责菜单
灰显和设置页初始化。0.6.0 六组实测已确认该方案能保留游戏自己的持久状态，并自然覆盖
无边框与普通窗口化。

`observe` 不修改游戏传给 DXGI、AGS 或 HDR 菜单的任何参数，仅用于门控和状态转换诊断。

`unlock_hdr_menu` 先调用 App Ver. 1.17 真正 HDR 行的原灰显谓词，
记录 `original_grayed`，再把返回 UI 的 `effective_grayed` 改为 `false`。它不会：

- 直接修改 HDR 配置字段；
- 主动调用或替换 `SetColorSpace1`；
- 强制修改交换链格式或 HDR 元数据；
- 伪造 AMD AGS 调用。

0.3.0 实测已完成这个目标：游戏自己的 setter 确实把实时配置 `+0x1B` 改为 `1`，但无边框
画面仍是正常 SDR。该模式继续保留为后端状态的只读对照。

`emulate_hdr_fullscreen_state` 同样解锁菜单，并在游戏请求 HDR 后检查具体交换链和输出。
只有全部安全条件满足时，才把 `FUN_141E9F4D0` 返回给游戏渲染器的内部状态从 `false`
改为 `true`。它不会：

- 修改 `IDXGISwapChain::GetFullscreenState` 对游戏其他部分的真实结果；
- 主动调用 `SetColorSpace1` 或切换独占全屏；
- 修改交换链格式、HDR metadata、Windows HDR 或 AGS 状态。

0.4.0 实测中，该模式成功得到 `effective_actual=true`，但画面整体发灰、不是正确 HDR；
关闭后内部状态和 SDR 画面均能恢复。它因此继续保留为“不提交颜色空间”的诊断对照。

`emulate_hdr_and_set_pq` 继承上述全部安全检查。后端 Hook 只记录颜色空间请求；交换链
`Present` Hook 在对应 HDR 帧提交前重新验证交换链和输出，再调用 `SetColorSpace1(PQ)`；
关闭 HDR 时则在对应 SDR 帧提交前恢复进入实验前最后观察到的颜色空间。它还具有以下保护：

- 任一次受管颜色空间切换失败后锁存失败，不逐帧重试；
- 检测到冲突的外部 `SetColorSpace1` 时放弃争夺所有权；
- 不修改交换链格式、HDR metadata、Windows HDR、真实全屏状态或 AGS。

0.6.0 至 1.0.0 不把 HDR 开关映射成 MOD 配置项，也不读写 `.sl2/.co2`。六组实测已确认游戏
自身的状态可以在公共可用性修正后恢复，因此无需增加 MOD 状态文件；直接修改存档不在
计划内。

`unlock_hdr_menu`、`emulate_hdr_fullscreen_state` 和 `emulate_hdr_and_set_pq` 只保留为
`docs/testing.zh-CN.md` 所需的诊断模式。无论选择何种模式，只要检测到版本、RTTI、相邻
虚表项、调用函数字节或 DXGI 前提不符，DLL 都会拒绝对应干预并记录原因。

`force_pq_if_hdr10` 已退役。若旧 INI 仍请求它，DLL 会写入 `SAFETY` 日志并自动退回
`observe`，绝不会再次把 SDR 画面强行标记为 PQ。

## 构建与打包

本机若由 mise 管理 Cargo，脚本会使用 `mise exec -- cargo`；否则使用 PATH 中的 Cargo：

```powershell
.\scripts\build.ps1
.\scripts\package.ps1 -SkipBuild
```

生成目录、ZIP 和 SHA-256 校验文件位于 `dist\EldenRingWindowedHDR-1.0.0`。发布 DLL 位于：

```text
target\x86_64-pc-windows-msvc\release\EldenRingWindowedHDR.dll
```

正式 ZIP 只包含运行所需的 `.me3`、DLL、INI、英文/中文 TXT README、许可证和第三方声明；
源码仓库中的 `docs` 与 `scripts` 不进入玩家发布包。

## ModEngine3 加载

打包目录包含 `EldenRingWindowedHDR.me3`，其中明确设置：

- `start_online = false`；
- `load_early = true`；
- `initializer = { function = "elden_ring_windowed_hdr_init" }`。

保持 `.me3`、`natives\EldenRingWindowedHDR.dll` 和同目录 INI 的相对位置不变，然后通过
ModEngine3 启动。不要绕过 EAC 后进入官方匹配；本 MOD 只用于离线运行。

DLL 每次启动都会截断并重写同目录的 `EldenRingWindowedHDR.log`，因此不需要手动删除；
只需在再次启动游戏前保存上一份日志。完整测试顺序和收集命令见 `docs/testing.zh-CN.md`。

## 首轮实测结论与当前边界

六次 0.1.0 实测均创建 `3840x2160`、`R10G10B10A2_UNORM`、3 缓冲、
`FLIP_DISCARD` 的同一类交换链。全屏与无边框的已见差异主要是
`SetFullscreenState(true)`；不是 8 位与 10 位格式之差。Windows 输出在所有测试中均报告
10 bpc 与 PQ，但这只描述输出/Advanced Color 状态，不能证明游戏像素已经是 PQ。

`force_pq_if_hdr10` 两次成功调用 `SetColorSpace1(PQ)` 后出现明显异常色彩，直接证明当时
内部着色器仍在输出 SDR。全屏 HDR 的两次发灰和重启后恢复，在已记录的 DXGI 描述中没有
差异，而且用户确认不加载本 MOD 也会出现；目前归为游戏/驱动已有异常，而不是观测 Hook
造成。

0.2.0 的四组实测都成功安装旧 Hook，却没有一次 `HDR menu gate` 回调；静态复核确认旧目标
属于 `02_046_BrightnessSetting` 亮度校准页。0.3.0 的四次实测确认了正确门控、菜单解锁和
HDR 配置 `+0x1B` 的完整 `0→1→0` 链，但无边框开启后画面与关闭时一样，仍为 SDR。

继续反编译确认，实时配置更新会调用后端 setter；渲染器随后又以
`IDXGISwapChain::GetFullscreenState`、输出身份和能力位计算 `+0x13C`“实际 HDR 状态”。
无边框在这一步被压回 `false`。0.4.0 实测已确认：严格模拟该结果能触发下游状态和明显
画面变化，但由于无边框路径没有 `SetColorSpace1`，最终画面发灰而非正确 HDR。当前最强
假设是 HDR/PQ 像素仍被 DWM 按默认 SDR 色彩空间解释；0.5.0 的实机结果已验证 Present
同步 PQ 能消除该失配。0.6.0 六次实测又确认了无边框/普通窗口化开关、开启/关闭持久化及
启动自动恢复。维护者后续又确认 Windows HDR 关闭时选项安全保持不可用，两台 HDR 显示器
均开启时，窗口跨屏显示以及用 `Win + Shift + Left/Right` 在两屏间移动都能保持正常 HDR。
当前未验证项转为跨硬件、HDR/SDR 混合显示器、Windows HDR 热切换、休眠恢复和其他 Hook
共存。详细证据和测试顺序见
`docs/feasibility-analysis.zh-CN.md`、`docs/testing.zh-CN.md` 与
`docs/final-audit.zh-CN.md`。
