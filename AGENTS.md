# AGENTS.md

本文件为在本仓库中工作的 AI Agent 提供长期上下文，作用域覆盖整个仓库。若子目录中有
更具体的 `AGENTS.md`，以距离目标文件最近的一份为准；用户的当前指令始终优先。

## 项目目标与当前阶段

本项目研究并计划实现《艾尔登法环》在“无边框窗口化”和普通“窗口化”模式下使用游戏原生 HDR 渲染链。
预期产物是一个以 Rust 编写、由 ModEngine3 提前加载的 64 位进程内 DLL，不修改磁盘上的
`eldenring.exe`、显卡驱动设置或存档。

0.1.0 动态观测器已经完成首轮真实游戏/HDR 显示器测试；实测证明全屏 SDR、全屏 HDR 与
无边框 SDR 均为 10 位交换链，而单独设置 PQ 会造成 SDR/PQ 编码失配。0.2.0 的四组实测证明
当时选错了亮度校准页的 lambda，所谓菜单解锁从未执行。0.3.0 的四次实测已确认真正 HDR
行可解锁、setter 能把实时配置 `+0x1B` 做 `0→1→0`，但无边框开启期间画面仍是正常 SDR。
0.4.0 三组实测已确认全屏原生状态、无边框第二层门控和严格模拟路径；模拟成功后画面发灰，
关闭后恢复正常 SDR，且没有任何颜色空间提交。0.5.0 的
`20260830-215204-v050-borderless-hdr-pq-sync` 已确认 Present 前 PQ 能得到正常无边框 HDR，
关闭后恢复正常 SDR。0.6.0 的六次实测又确认公共 HDR 可用性覆盖能保留游戏自身的开启/
关闭状态，并覆盖普通窗口化；无边框与普通窗口化均得到正常 HDR/SDR。0.6.1 完成失败路径、
未知 Hook 冲突和打包证据保护的最终审计补丁，正常 HDR 状态机未改变。1.0.0 以
`Elden Ring Native Windowed HDR` 为正式名称，将 `windowed_hdr` 设为发布默认值，并精简
玩家发布包、补充双语 TXT README、Nexus 素材和 GitHub Actions；核心状态机仍未改变。先阅读
`docs/feasibility-analysis.zh-CN.md`、`docs/version-compatibility.zh-CN.md`、
`README.zh-CN.md` 和 `docs/testing.zh-CN.md`，不得把当前单台 NVIDIA 验证误写成跨硬件
通用可用。1.0.0 发布前又加入严格的跨版本目标解析：App Ver. 1.16.2 与 1.17 的磁盘 EXE
均已通过静态双版本审计，并各以两次真实游戏启动完成动态解析、HDR/SDR 与持久化回归；不得
把这台 NVIDIA 测试机上的双版本结果写成跨硬件通用支持。

## 固定目标与外部资源

已知分析目标有两个：

- App Ver. 1.16.2 / 文件版本 `2.6.2.0`：`86,998,096` 字节，SHA-256
  `34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB`；
- App Ver. 1.17 / 文件版本 `2.7.0.0`：`87,024,720` 字节，SHA-256
  `D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134`；
- PE：Windows x86-64，映像基址 `0x140000000`

维护者当前提供的仓库外资源：

- 游戏只读备份：
  `I:\backup\艾尔登法环\有DLC-1.17-1.17\steamapps\common\ELDEN RING\Game\eldenring.exe`
  以及同一根目录下的 1.16.2 备份；同版本“有 DLC/无 DLC”副本已确认字节相同；
- Ghidra：`D:\DevTools\Ghidra`
- 可参考的 Rust Hook 项目：`D:\Projects\RustroverProjects\UnlockTheFps`
- ModEngine3 源码：`D:\Projects\RustroverProjects\me3`

这些路径只是当前机器的工作上下文。开始分析前先确认路径和文件哈希，不要因为版本字符串
相同就复用地址或签名。

## 已确认的关键事实

- 游戏是 D3D12 程序，导入 `CreateDXGIFactory`、`CreateDXGIFactory1`，并导入 AGS 5.0.5
  的 `agsInit`、`agsDeInit`、`agsSetDisplayMode`。
- `HDRSetting` 是真实配置键；全屏与无边框宽高使用不同配置键。这只能证明配置路径存在，
  不能证明修改单个布尔值足以启用 HDR。
- `0x141EB0640` 所在函数按显示设备名查找 AGS 显示器，清零一个 `0x68` 字节的
  `AGSDisplaySettings`，在关闭时写 `mode = 0`，开启时写 `mode = 2`，随后调用
  `agsSetDisplayMode`。
- AGS 5.0.5 官方头文件把 `0` 定义为 SDR、`1` 定义为 scRGB、`2` 定义为 PQ；PQ 要求
  `10:10:10:2 UNORM` 交换链和 PQ 输出着色器。由此可知游戏原生 HDR 至少涉及渲染编码、
  交换链格式和显示状态，绝非纯 UI 开关。
- Windows Advanced Color 支持由 DWM 合成的窗口化 HDR。平台本身不要求独占全屏，但
  交换链必须满足格式、颜色空间、呈现模型和显示能力等条件。
- 六组 0.1.0 日志中的交换链均为 `3840x2160`、`R10G10B10A2_UNORM`、3 缓冲、
  `FLIP_DISCARD`；无边框与全屏的已见差异不是交换链位深。
- `force_pq_if_hdr10` 在无边框 SDR 状态两次成功设置 PQ 后产生明显色彩异常，证明输出描述
  为 PQ 不等于游戏像素已是 PQ；不得恢复该自动路径。
- 四组 0.2.0 日志都显示旧 Hook 安装成功，但打开 HDR 设置页时调用次数始终为零；结合菜单
  构造顺序，已确认 `FUN_140808A70` / `02_046_BrightnessSetting` 属于 HDR/SDR 亮度校准页，
  不是“高动态范围成像”开关。
- 真正 HDR 行由 `FUN_14095D540` 构造。其值位于菜单页 `+0x1CE3`；setter
  `FUN_140962350` 把该值写到 `MENU_OPTION_DATA +0x15`，调用菜单页虚函数 `+0xA0`，再调用
  `FUN_14067B150`。`FUN_14025C780` 随后把源 `+0x15` 复制到实时图形配置 `+0x1B`。
- 真正 HDR 灰显谓词虚表为 `0x142B152C8`，调用槽 `0x142B152D8` 指向
  `FUN_140962B30`。它返回 `!FUN_140953A10()`；后者同时检查一个值为 `1` 的渲染/显示模式
  条件及显示能力查询结果。0.3.0 已实测该条件在全屏为真、无边框为假。
- `FUN_140680420` 在图形配置脏标志出现时把实时配置 `+0x1B` 送入
  `FUN_1419ECB00`；后者把请求写入渲染记录 `+0x13D`，并通过 `FUN_141E9F4B0` 更新后端
  `+0x30`。绘制更新又把 `FUN_141E99780` 的返回值写入记录 `+0x13C`，这是实际 HDR 状态。
- `FUN_141E99780` 转入 `FUN_141E9F4D0`。该函数经交换链虚表 `+0x58` 调用
  `IDXGISwapChain::GetFullscreenState`，并要求能力位、输出匹配和独占全屏同时成立；这就是
  无边框配置已为 HDR、渲染仍为 SDR 的第二层门控。
- 0.5.0 实机已确认：上述内部状态成立后，在对应 `Present` 前设置
  `RGB_FULL_G2084_NONE_P2020` 能得到正常 HDR；关闭时恢复 `G22/P709` 后得到正常 SDR。
- 0.6.0 六组实机已确认：无边框开/关回归、无边框 HDR=true/false 跨启动、普通窗口化
  HDR=true 跨启动全部成立；普通窗口化交换链为 10 位 flip-model，启动恢复无需打开设置页
  或加载角色。
- 1.0.0 四次跨版本实机回归已确认：1.16.2 与 1.17 都能唯一解析并交叉核对全部目标；两版
  无边框 `HDR→SDR→HDR`、启动自动恢复与 SDR 恢复均成功，1.17 还在普通窗口化
  `ResizeBuffers` 后重新提交 PQ。四份日志没有兼容失败、`SAFETY`、非零 HRESULT、设备移除
  或冲突，维护者确认视觉全部正常；同时加载 UnlockTheFps 与 Alt+Tab 在这组限定组合中通过。
- 维护者追加实测已确认：Windows HDR 关闭时，即使 `mode = windowed_hdr`，HDR 选项也保持
  不可选择；两台显示器均开启 HDR 时，普通窗口可以跨两屏显示，也可以通过
  `Win + Shift + Left/Right` 在两屏间移动，HDR 画面仍正常。该轮未提供新的 DLL 日志，
  因而属于用户操作与视觉确认；HDR/SDR 混合显示器、运行中热切换和断连仍待验证。
- `FUN_140953A10` 只有两个已确认直接调用者：真正 HDR 行谓词 `FUN_140962B30` 和设置页
  初始化 `FUN_14093D730`。后者在可用性为假时会把局部 HDR 值同步为实际状态并经
  `FUN_14025C780` 写回；这是窗口态已保存 HDR 被压回关闭的直接解释。
- 用户观察到 `.sl2/.co2` 会随相关启动/设置流程变化，但文件时间戳不足以证明具体字段位置；
  不得解析或修改存档来实现持久化。

## 当前实现状态

- crate 为 Rust 2024 `cdylib`，发布 DLL 名为 `EldenRingWindowedHDR.dll`。
- ModEngine3 配置位于 `packaging/EldenRingWindowedHDR.me3`，使用 `load_early = true`、
  显式 initializer，并保持 `start_online = false`。
- `mode = observe` 对 DXGI/AGS 和真正 HDR 行灰显谓词透明转发，并记录源 `+0x15` 到实时
  配置 `+0x1B` 的应用过程；1.0.0 起它只作为诊断模式保留。
- `mode = unlock_hdr_menu` 仍先调用真正 HDR 行的原灰显谓词，然后只把其返回值改为
  `false`。它不主动调用 `SetColorSpace1`，实际设置仍由游戏自己的 setter 和应用链处理。
- `mode = emulate_hdr_fullscreen_state` 同时解锁菜单，并观测 `FUN_141E9F4D0` 的原结果；仅当
  实时配置、后端请求、10 位无边框 flip-model、PQ/10 bpc 输出和
  `CheckColorSpaceSupport(PRESENT)` 全部成立时，才把这一内部结果改为真。它不改变 DXGI 的
  真实全屏状态，也不主动设置 PQ。
- 0.4.0 实测中，上述模式得到 `effective_actual=true`、`override=true`，并在关闭时可靠恢复，
  但开启画面发灰、不是正确 HDR。结合整个过程没有 `SetColorSpace1`，当前最强推断是内部
  HDR/PQ 路径已改变像素，而无边框交换链仍以默认 SDR 标签交给 DWM。
- `mode = emulate_hdr_and_set_pq` 继承全部严格检查；只记录后端请求，在对应 HDR 帧
  `Present` 前重新验证具体交换链/输出后提交 PQ，并在 HDR 关闭后的 SDR 帧 `Present` 前
  恢复此前观察到的颜色空间。
  失败会按交换链锁存且不逐帧重试；冲突的外部 `SetColorSpace1` 会使实验放弃所有权。
- 0.5.0 实机已验证 `emulate_hdr_and_set_pq` 的正常 HDR、正常 SDR 恢复和成功 HRESULT。
- `mode = windowed_hdr` 继承上述状态机，但不直接强制菜单结果；它 Hook 公共可用性函数，
  仅在活动交换链通过 10 位、窗口态、flip-model、PQ 输出和 `PRESENT` 严格检查时把原生
  false 覆盖为 true。它不区分无边框与普通窗口化，不创建 MOD 持久化布尔值，也不读写
  `.sl2/.co2`。启动时实时配置尚为 unknown 时，只信任后端自身的已确认 HDR 请求位。
- 0.6.0 已实机验证 `windowed_hdr` 的无边框/普通窗口化正常 HDR、正常 SDR 恢复及游戏自身
  持久化。1.0.0 的发布 INI 与缺省配置均默认 `windowed_hdr`；版本/输出/DXGI 前提不满足时
  仍安全拒绝干预。
- 旧 `mode = force_pq_if_hdr10` 会记录 `SAFETY` 并降级为 `observe`。
- `src/game_compat.rs` 只扫描主模块可执行 PE 节，以多段签名唯一解析公共可用性、配置复制
  和后端状态函数；要求公共函数恰有两个直接调用者，并以调用关系定位灰显谓词，再从代码
  RIP-relative `lea` 反向验证唯一 RTTI/COL/六项虚表。安全 Cookie 从已验证指令动态解析，
  必须落在可读非执行节。
- 已知 1.16.2/1.17 指纹会继续逐项核对预期 RVA；未知哈希只有在整组签名、调用关系、RTTI、
  虚表、字段语义和内存边界全部通过时才按“结构兼容但未实机验证”放行。失败时不安装游戏
  内部 HDR Hook、不接管 PQ，只保留 DXGI/AGS 诊断并把具体原因写入日志。
- 已知 1.16.2/1.17 的上述运行时解析器均已在真实游戏中验证；未来未知哈希即使结构放行，
  仍必须保持“未实机验证”标记，直到完成相同回归。
- 菜单、后端和公共可用性 Hook 在安装前还会复核解析时捕获的虚表邻项与函数首部；行为
  修改模式遇到未知前置 Hook 或任一 DXGI 前提失败时安全拒绝。安装逻辑位于
  `src/game_hdr.rs`。
- `scripts/collect-logs.ps1` 用于保存每次启动会被覆盖的 DLL 日志及系统/GPU/EXE 指纹。
- `scripts/package.ps1` 若发现同版本包目录含 `test-results` 会拒绝覆盖，禁止绕过该保护删除
  实机证据。1.0.0 正式 ZIP 不再包含开发用 `docs` 和 `scripts`，只包含运行文件、双语 TXT
  README、许可证和第三方声明；脚本同时生成 `.sha256`。最终审计与证据边界见
  `docs/final-audit.zh-CN.md`。
- GitHub Actions 的 CI 与 Release 工作流固定使用 Rust 1.98.0，避免浮动 `stable` 新增 Clippy
  lint 后让未改动的发布无预警失败。`src/sha256.rs` 已兼容 1.98 的
  `chunks_exact_to_as_chunks` lint；该机械改写不改变摘要算法或 HDR/DXGI 状态机。

文档中的虚拟地址只对其明确标注的哈希有效。地址用于解释和已知版本交叉核对，不得在发布
代码中作为无结构校验的裸常量。

## 开始工作前

1. 运行 `git status --short`，保留用户已有修改。
2. 阅读本文件、`docs/feasibility-analysis.zh-CN.md` 和
   `docs/version-compatibility.zh-CN.md`。
3. 涉及 DXGI Hook 时，阅读 `UnlockTheFps` 的 `AGENTS.md`、`src/dxgi.rs`、`src/windows.rs`
   和 `src/lib.rs`；复用其已验证思路时保留 Overlay 链式 Hook 与内存边界检查。
4. 涉及加载时序时，阅读 ModEngine3 的 `schemas/mod-profile.zh.md` 和
   `crates/mod-host/src/host.rs`，不要凭旧版 ModEngine2 行为猜测。
5. 优先使用 `rg` / `rg --files`。编译器、运行时或包管理器可能由 mise 管理时，先用
   `mise current`、`mise which <tool>` 检查，并优先以 `mise exec -- <command>` 调用；
   若该工具不由 mise 管理，再使用 PATH 中的版本。

## 安全边界

- 仅离线使用 ModEngine3，保持 `start_online = false`，不要建议绕过 EAC 后进入官方匹配。
- 不修改或覆盖游戏备份，不向 Git 提交游戏二进制、AGS DLL、着色器提取物、Ghidra
  数据库、崩溃转储或完整运行日志。
- 不自动修改 Windows 全局 HDR、注册表、显示器模式或显卡驱动配置。若系统 HDR 未开启，
  DLL 应记录原因并安全退回原行为。
- 所有签名必须只扫描主模块的可执行 PE 节；要求唯一命中，并在不匹配时安全失败。
- 不把静态分析当成运行时验证。无法启动真实游戏时，明确列出未验证项。

## 推荐实现顺序

### 阶段 A：DXGI/AGS 基线（已完成）

- 通过 EXE IAT Hook 捕获 `CreateDXGIFactory` / `CreateDXGIFactory1`。
- 对工厂和交换链使用对象级影子虚表，记录所有交换链创建路径、`QueryInterface`、
  `ResizeBuffers`、`SetFullscreenState`、`CheckColorSpaceSupport`、`SetColorSpace1`、
  `SetHDRMetaData` 与 `Present`。
- 记录 `DXGI_SWAP_CHAIN_DESC` / `DESC1` 的格式、缓冲数、SwapEffect、Flags、窗口状态和
  当前输出；记录 `IDXGIOutput6::GetDesc1` 的颜色空间及亮度能力。
- Hook EXE 对 `agsSetDisplayMode` 的导入，只记录设备索引、显示索引、结构内容和返回值。
- 已采集“全屏 SDR、全屏 HDR、无边框 SDR”及复测；结论见可行性文档。

### 阶段 B：内部 HDR 门控实验（已完成诊断）

- 0.2.0 四组日志已确认旧 Hook 回调从未执行；不得再根据旧地址推断 HDR 门控。
- 0.3.0 已实测确认无边框/全屏的 `original_grayed`、菜单解锁和
  `source+0x15` / `destination+0x1B` 转换；无边框画面仍为 SDR。
- 0.4.0 `observe` 已确认全屏原生 HDR 为 `native_actual=true`；`unlock_hdr_menu` 已确认无边框
  请求为 `backend_requested_hdr=true`、`native_actual=false`。
- 0.4.0 `emulate_hdr_fullscreen_state` 已确认候选为 10 位无边框 flip-model、PQ/10 bpc 输出、
  `PRESENT=true` 且 `override=true`。视觉上进入发灰状态，关闭后可靠恢复 SDR。
- 该视觉变化与静态写入链共同证明下游 HDR 状态已受影响；0.5.0 随后用 Present 同步
  `SetColorSpace1` 验证了颜色空间标签假设，并得到正常 HDR/SDR。
- `SetHDRMetaData` 不是启用 HDR 的开关，只在确认游戏全屏基线确实使用且数值可靠时复现。
- AMD 路径在交换链或显示模式变化后复现游戏自己的 AGS 调用；非 AMD 路径不得伪造 AGS。

### 阶段 C：持久化、普通窗口化与用户功能（1.0.0 正式发布验收已完成）

- 已按 `docs/testing.zh-CN.md` 实测 0.6.0 `windowed_hdr` 的无边框开/关持久化。
- 已验证普通窗口化满足同一严格候选并得到正常 HDR/SDR；未因模式名称放宽条件。
- 已验证 Windows HDR 关闭时安全拒绝，以及两台均开启 HDR 的显示器间窗口跨屏/移动；
  新增证据没有日志，只能标记为维护者操作与视觉确认。
- 已在 1.16.2 与 1.17 上完成最终动态解析器短回归；四次均同时加载 UnlockTheFps，Alt+Tab
  未出现问题。该结果只覆盖此明确组合，不能外推到其他 Overlay。
- 继续以游戏自身序列化状态为唯一事实来源；不增加 MOD 状态文件，任何情况下都不直接
  改写 `.sl2/.co2`。
- 待验证 HDR/SDR 混合显示器、显示器断连、HDR 开启期间切换独占全屏、Windows HDR
  热切换、休眠恢复、AMD/Intel 及其他 Overlay/MOD 组合。

## Rust 与 DLL 约束

- 使用 Rust 2024 edition、`cdylib`、`x86_64-pc-windows-msvc`，发布配置建议
  `panic = "abort"`。
- crate 根启用 `#![deny(unsafe_op_in_unsafe_fn)]`。每个 `unsafe` 块附近说明 ABI、指针、
  虚表长度、生命周期或页保护的前置条件。
- `DllMain` 只做加载器锁下必要的最少工作：保存模块句柄、关闭线程通知、触发轻量初始化。
  禁止在其中进行文件 I/O、复杂扫描、等待或 COM 调用。
- 导出一个明确的 `extern "C" fn() -> bool` 初始化函数供 ModEngine3 调用，并在 `.me3`
  中同时使用 `load_early = true` 和 `initializer = { function = "..." }`，保证工厂创建前安装
  观测 Hook。
- Hook 必须保存并链式调用前一个函数指针，兼容先后加载的 Overlay 和其他 MOD；对象销毁
  时正确处理 `Release` 与影子表生命周期。
- 日志不得在每帧无界写入；对稳定状态去重，仅在转换、错误或能力变化时记录。

普通改动至少执行：

```powershell
mise exec -- cargo fmt --all -- --check
mise exec -- cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
mise exec -- cargo test --locked --target x86_64-pc-windows-msvc
mise exec -- cargo build --locked --release --target x86_64-pc-windows-msvc
```

若 `mise which cargo` 明确表明 Cargo 不由 mise 管理，则去掉 `mise exec --`，使用同样的
Cargo 参数。不能把“编译和单元测试通过”写成“游戏内 HDR 已验证”。

## 实机验收下限

- Windows HDR 已开启且 `IDXGIOutput6::GetDesc1` 报告 Advanced Color。
- 无边框状态下交换链为 `R10G10B10A2_UNORM`，颜色空间为
  `RGB_FULL_G2084_NONE_P2020`，并能稳定 Present。
- 与全屏 HDR 基线相比没有灰雾、过曝、黑位抬升、色域裁切或重复色调映射。
- Alt+Tab、改变分辨率、切换显示器、休眠恢复和 Windows HDR 状态变化均能安全恢复或
  明确回退。
- 至少覆盖 AMD 与 NVIDIA；AMD 额外核对 `agsSetDisplayMode(mode = 2)`，NVIDIA 不依赖
  AGS。记录 GPU、驱动、Windows 版本、显示器、连接方式和游戏文件哈希。
- 与 ModEngine3、常用 Overlay 和 UnlockTheFps 同时加载不崩溃、不破坏 Hook 链。

## 文档与提交要求

- 简体中文使用全角标点，API、符号、地址、路径和配置键保持原文。
- 逆向结论标注“已确认”“推断”或“待实机验证”，不要掩盖证据边界。
- 改变实现方案或验证状态时同步更新 `docs/feasibility-analysis.zh-CN.md`。
- 提交保持聚焦，不顺带格式化或覆盖用户无关修改，不使用破坏性 Git 命令恢复工作树。
- 最终交付必须说明执行过的检查、未执行的真实游戏测试及产物绝对路径。
