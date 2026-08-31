# 《艾尔登法环》窗口态原生 HDR 可行性分析

分析日期：2026-08-31  
实机目标：App Ver. 1.16.2 / `eldenring.exe` 2.6.2.0 与 App Ver. 1.17 / `eldenring.exe` 2.7.0.0
阶段：0.6.0 已在首台 NVIDIA 实机通过无边框/普通窗口化 HDR、SDR 恢复与开关持久化；
0.6.1 完成最终审计补丁；1.0.0 已完成双版本动态解析器实机验收

## 结论

结论仍是“有条件可行”，但首轮实测已经排除“交换链在无边框下退化为 8 位”这一假设，
并直接否定“只设置 PQ 色彩空间”这一实现。

Windows 的 Advanced Color 架构允许 DWM 合成窗口化/无边框 HDR，因此“必须独占全屏”
不是操作系统或 DXGI 的硬限制。微软给出的窗口化 HDR 路径要求使用可参与 DWM Advanced
Color 的 flip-model 交换链，并使用以下匹配之一：

- `R16G16B16A16_FLOAT` + scRGB；
- `R10G10B10A2_UNORM` + `RGB_FULL_G2084_NONE_P2020`（HDR10/PQ）。

对本游戏的静态分析证明原生 HDR 至少涉及内部 HDR 渲染状态、PQ 编码和 AMD 显示状态；
动态日志又证明全屏 SDR、全屏 HDR、无边框 SDR 都已经使用 10 位 flip-model 交换链。
0.3.0 进一步证明，无边框下菜单和 `HDRSetting` setter 都能工作，但渲染画面仍为 SDR。
0.4.0 已严格模拟游戏以独占全屏派生的“实际 HDR 状态”：日志得到
`effective_actual=true`，画面从正常 SDR 变为发灰，关闭 HDR 后两者均恢复；整个过程没有
`SetColorSpace1`。0.5.0 随后在同一内部状态成立后，把 PQ 提交同步到下一次 `Present`
之前；实机日志确认开启 PQ 和关闭恢复均成功，用户确认开启后为正常 HDR、关闭后为正常
SDR。因此当前 NVIDIA/显示器环境下的核心无边框 HDR 路径已经成立。

0.2.0 的四组实测推翻了当时的菜单目标判断：旧 Hook 安装成功，但打开 HDR 设置页时从未
执行，解锁模式下选项也仍然灰显。复核菜单构造顺序后确认旧目标属于 HDR/SDR 亮度校准页。
0.3.0 改为截获真正 HDR 行的灰显谓词，并记录真正 HDR 字段的提交。四次实测确认：全屏与
无边框的原生门控语义正确、无边框 UI 可解锁、实时配置 `+0x1B` 能完整 `0→1→0`，但开启
期间画面与关闭时一样，仍是色彩正常的 SDR。继续反编译已确认第二层门控来自
`IDXGISwapChain::GetFullscreenState`；0.4.0 对这一层的实测证明覆盖确实进入下游状态，但没有
得到正确画面。0.5.0 的单变量实验已确认缺少的正是对应时序的窗口化 PQ 色彩空间提交。
0.6.0 六次实测随后确认游戏自己的 HDR 开关可以在两种窗口态跨启动保留，普通窗口化也能
通过相同严格候选并得到正常 HDR/SDR。维护者随后又确认：Windows HDR 关闭时功能会安全
保持不可用；两台均开启 HDR 的显示器可以跨屏显示窗口，也可以用
`Win + Shift + Left/Right` 移动而保持正常 HDR。当前问题已转为补足异常恢复、跨硬件和
其他 Hook 共存验证，而不是继续寻找核心启用路径。1.0.0 最终四次回归又在 App Ver.
1.16.2 与 1.17 上真实执行了动态解析器：两版的唯一目标、Hook、窗口态 HDR/SDR 与启动
持久化均通过，所有受管颜色空间调用成功，视觉观察正常。

## 2026-08-30 实现进度

已完成 Rust `cdylib`。0.1.0 至 0.6.1 的开发包默认使用 `observe`，该模式不改变 DXGI、AGS
或菜单谓词返回值，能够观测工厂与
交换链创建、窗口状态、Resize、Present、颜色空间能力与设置、HDR 元数据、
`IDXGIOutput6::GetDesc1` 和 `agsSetDisplayMode`。对象级影子虚表和 Overlay 链式调用机制
改编自 `UnlockTheFps`，其压力测试已适配并通过。

0.1.0 曾提供 `force_pq_if_hdr10`，实测在无边框 SDR 内部状态下两次成功执行
`SetColorSpace1(PQ)`，随即出现明显色彩异常。该模式已经完全移除；旧 INI 请求会自动安全
退回 `observe`。0.3.0 的 `unlock_hdr_menu` 不主动调用任何 DXGI HDR 设置。0.4.0 新增
`emulate_hdr_fullscreen_state`：只在实时配置、后端请求、10 位无边框 flip-model、PQ 输出和
`CheckColorSpaceSupport(PRESENT)` 全部通过时，改变游戏内部实际状态查询的返回值；不修改
DXGI 的真实全屏状态，也不主动提交 PQ 色彩空间。0.4.0 实测已确认该覆盖和关闭回退均生效，
但开启时画面发灰。

0.5.0 新增 `emulate_hdr_and_set_pq`。它继承全部前置检查，先让后端查询把实际状态交给绘制
系统，再把 PQ 请求安排到对应 HDR 帧的 `Present` 前；执行时会重新验证交换链、输出和
`PRESENT` 支持。关闭时则在对应 SDR 帧 `Present` 前恢复进入实验前最后观察到的颜色空间。
失败按交换链锁存，不逐帧重试；若外部 `SetColorSpace1` 与状态机冲突，则放弃颜色空间
所有权。`20260830-215204-v050-borderless-hdr-pq-sync` 已在首台 NVIDIA 实机验证：开启
与恢复的 HRESULT 均成功，视觉结果分别为正常 HDR 与正常 SDR。

0.6.0 新增 `windowed_hdr`。它不解析或修改 `.sl2/.co2`，而是在严格窗口态 DXGI 候选检查
通过后覆盖公共 HDR 可用性函数 `FUN_140953A10`。该函数同时控制真正 HDR 行灰显和设置页
初始化时的“不可用则同步回实际关闭状态”分支，因此能让游戏继续使用自身保存的 HDR
值。候选条件只要求真实窗口态而不区分无边框/普通窗口化，所以两者共用同一路径。启动时
菜单配置尚为未知的短窗口内，仅当游戏后端自身的已确认 HDR 请求位为真时才允许启用。
0.6.0 六组实测已确认无边框/普通窗口化的开启、关闭、开启持久化和关闭持久化。0.6.1
只收紧失败 HRESULT 的诊断读取、未知菜单 Hook 冲突处理和打包证据保护，不改变已验证的
正常 HDR 状态机路径。

1.0.0 采用正式名称 `Elden Ring Native Windowed HDR`，crate、DLL、INI、日志和 `.me3`
前缀统一为 `EldenRingWindowedHDR`。发布 INI 与缺省配置改为 `windowed_hdr`；`observe`
继续作为诊断模式保留。玩家 ZIP 只包含运行文件、双语 TXT README、许可证和第三方声明，
不包含源码仓库中的 `docs` 与 `scripts`。这些发布层改动不改变 0.6.0 已验证的 HDR 状态机。

运行时仍记录目标 EXE 的大小和 SHA-256，但不再把完整哈希作为唯一放行条件。它只扫描
主模块可执行 PE 节，要求四个内部目标唯一，并交叉校验调用关系、RTTI/COL、六项虚表、
安全 Cookie 和关键字段复制；已知 1.16.2/1.17 还会逐项核对预期 RVA。未知哈希全部通过时
按“结构兼容但未实机验证”尝试运行，任何歧义则只保留 DXGI/AGS 诊断、拒绝内部 HDR 与 PQ
干预。ModEngine3 配置使用提前加载与显式 initializer；每次启动的日志写在 DLL 同目录，
具体兼容性证据见 `docs/version-compatibility.zh-CN.md`，测试矩阵见
`docs/testing.zh-CN.md`。

## 2026-08-30 首轮实测结果

测试环境为 Windows 11 10.0.26200、GeForce RTX 4090 D（驱动 `32.0.16.1074`）及同机 AMD
集显；目标输出为 `\\.\DISPLAY2`。输出在六组日志中均报告 10 bpc、
`RGB_FULL_G2084_NONE_P2020` 和 `0.010..420.000 nits`。

| 日志 | 游戏状态 | 交换链 | 独占状态 | 视觉结果 |
| --- | --- | --- | --- | --- |
| `20260830-164659-fullscreen-sdr` | 全屏 SDR | 10 位、3 缓冲、flip discard | 启动后切到 `true` | SDR 基线 |
| `20260830-165140-fullscreen-hdr1` | 同次启动由全屏 SDR 开启 HDR | 同上 | `true` | 发灰，非正常 HDR |
| `20260830-165418-fullscreen-hdr2` | 全屏 HDR | 同上 | `true` | 发灰，非正常 HDR |
| `20260830-170914-fullscreen-hdr3` | 重启后全屏 HDR | 同上 | `true` | 正常 HDR |
| `20260830-171429-borderless-sdr` | 无边框 SDR | 同上 | 始终 `false` | 正常 SDR |
| `20260830-171624-borderless-force-pq` | 无边框 SDR + 强制 PQ 标签 | 同上 | 始终 `false` | 明显色彩格式/空间失配 |

所有六组交换链均为 `3840x2160`、`R10G10B10A2_UNORM`、3 缓冲、`FLIP_DISCARD`、
Flags `0x2`。因此：

- **已确认：**无边框不是因为交换链格式为 8 位而禁用 HDR；
- **已确认：**`IDXGIOutput6::GetDesc1.ColorSpace = PQ` 只说明 Windows 输出/Advanced Color
  状态，不能证明交换链内容或游戏着色器已经输出 PQ；
- **已确认：**在游戏 HDR 仍关闭时单独调用 `SetColorSpace1(PQ)` 会把 SDR 编码按 PQ 解释，
  产生用户观察到的异常色彩；
- **已确认：**已收集的六次启动均没有游戏自身的 `CheckColorSpaceSupport`、
  `SetColorSpace1`、`SetHDRMetaData` 或 `agsSetDisplayMode` 调用；
- **已确认：**`fullscreen-hdr1` 是实际执行“全屏 HDR 关闭 → 开启”的同次启动日志；其中
  仍没有上述 DXGI/AGS HDR 调用。对当前 NVIDIA 路径而言，HDR 开启不是通过已 Hook 的
  `SetColorSpace1`、`SetHDRMetaData` 或 AGS IAT 完成；
- **已补充但证据不足：**0.2.0 的 `menu-only1` 记录了全屏到无边框的 DXGI 转换；由于当时
  未 Hook 真正 HDR 字段，它仍不能说明 HDR 关闭回调的内部时序；
- **推断：**两次全屏 HDR 发灰与重启后恢复在已观测 DXGI 状态上没有差异，且用户确认不
  加载 MOD 也会发生，当前更符合游戏/驱动已有的持久状态异常。

## 2026-08-30 第二轮实测与目标纠正

| 日志 | 模式与操作 | 旧 Hook 调用 | UI 结果 |
| --- | --- | --- | --- |
| `20260830-182333-v020-observe-borderless-gate` | 无边框、打开 HDR 页 | 0 次 | 灰显 |
| `20260830-182922-v020-observe-fullscreen-hdr-transition` | 全屏、HDR 开→关→开 | 0 次 | 原生可操作 |
| `20260830-183137-v020-unlock-borderless-menu-only1` | 切到无边框、打开 HDR 页 | 0 次 | 仍灰显 |
| `20260830-183349-v020-unlock-borderless-menu-only2` | 无边框、再次打开 HDR 页 | 0 次 | 仍灰显 |

四组日志均显示 `hdr_menu_gate=true`，但均没有 `HDR menu gate:` 运行时记录。因此失败点不是
“代理捕获值仍被第二层门控覆盖”，而是 Hook 对象根本没有在该页面执行。全屏转换日志仍未
捕获 `CheckColorSpaceSupport`、`SetColorSpace1`、`SetHDRMetaData` 或 `agsSetDisplayMode`；
这与 0.1.0 结果一致。

日志文件由 `Logger::new` 以 `truncate(true)` 打开，因此无需手动删除；必须做的是在下一次
启动覆盖前完成收集。

## 2026-08-30 第三轮实测与第二层门控

| 日志 | 模式与操作 | 关键运行时结果 | 视觉结果 |
| --- | --- | --- | --- |
| `20260830-195328-v030-observe-borderless-gate` | 无边框，只读门控 | `original_grayed=true` | HDR 灰显 |
| `20260830-195616-v030-observe-fullscreen-hdr-transition` | 全屏，HDR 开→关→开 | `original_grayed=false`；`+0x1B` 为 `0→1→0→1` | 原生切换基线 |
| `20260830-200011-v030-unlock-borderless-menu-only` | 无边框，只解灰 | `original_grayed=true`、`effective_grayed=false` | UI 可解锁 |
| `20260830-200213-v030-unlock-borderless-hdr-transition` | 无边框，HDR 开→关 | `+0x1B` 为 `0→1→0` | 开启期间仍为正常 SDR |

由此可作以下区分：

- **已确认：**真正 HDR 灰显谓词的 `native_eligible` 与独占全屏状态对应；
- **已确认：**0.3.0 的菜单 Hook 和真正 setter 均已生效，不存在“只是 UI 看起来可点”的
  问题；
- **已确认：**无边框下 `HDRSetting` 已进入实时配置，但在开启期间没有交换链颜色空间、
  metadata、AGS 或格式重建事件，画面也没有 PQ/SDR 失配，而是继续正常显示 SDR；
- **推断并经随后机器码确认：**实时配置之后仍有一层“实际 HDR 状态”，它把无边框请求
  压回 SDR。画面没有异常颜色也支持“HDR/PQ 着色路径从未开启”，而不是“已输出 PQ 但少
  一个色彩空间标签”。

## 2026-08-30 第四轮实测与窗口化颜色空间假设

| 日志 | 模式与操作 | 关键运行时结果 | 视觉结果 |
| --- | --- | --- | --- |
| `20260830-205735-v040-observe-fullscreen-backend-state` | 全屏原生 HDR 开→关 | `native_actual=true→false` | 符合预期 |
| `20260830-205944-v040-unlock-borderless-backend-state` | 无边框只解灰，HDR 开→关 | 请求为真时 `native_actual=false` | 正常 SDR |
| `20260830-210635-v040-emulate-borderless-hdr-transition` | 无边框内部状态模拟，HDR 开→关 | 候选全部通过；开启时 `effective_actual=true`、`override=true`，关闭时恢复 `false` | 开启后发灰、不是正确 HDR；关闭后恢复正常 SDR |

三份日志形成了完整对照：全屏原生路径能得到 `native_actual=true`；无边框只解灰时请求已
进入后端但实际状态仍为假；实验模式在同一类 10 位交换链上只改变内部查询结果后，状态和
画面都发生了对应变化。第 3 份日志还确认 `CheckColorSpaceSupport(PQ)` 的
`PRESENT=true`，但没有任何 `SetColorSpace1`、`SetHDRMetaData` 或 AGS 调用。

- **已确认：**内部查询覆盖、严格候选检查和关闭回退均按设计运行；
- **已确认：**`FUN_1419E7780` 会把该返回值写入记录 `+0x13C`，而游戏管理器只在该字段为
  真时提交后续 HDR 参数；实测画面变化与这条静态链一致；
- **已确认：**无边框状态仍缺少显式交换链颜色空间提交；
- **推断：**发灰最符合 PQ/HDR 路径像素仍被 DWM 按默认 G22/P709 解释，但仅靠肉眼不能
  证明最终像素已经是完整、正确的 PQ；
- **下一步（已完成）：**0.5.0 仅在上述状态成立后，于 `Present` 前提交 PQ，并在关闭后
  恢复此前观察到的颜色空间。

## 2026-08-30 第五轮实测与正常无边框 HDR

| 日志 | 模式与操作 | 关键运行时结果 | 视觉结果 |
| --- | --- | --- | --- |
| `20260830-215204-v050-borderless-hdr-pq-sync` | 无边框，内部状态 + Present 同步 PQ，HDR 开→关 | 开启时 `effective_actual=true`；`SetColorSpace1(PQ)` 成功；关闭时恢复 `G22/P709` 成功 | 开启后正常 HDR；关闭后正常 SDR |

日志时序确认：实时配置先由 `0→1`，后端下一次查询把 `effective_actual` 置真并登记 PQ，随后
紧邻的 `Present` 前再次通过候选检查并成功提交色彩空间；关闭时实时配置由 `1→0`，下一次
SDR `Present` 前恢复进入实验前记录的色彩空间。用户的画面判断与日志状态完全一致。

- **已确认：**当前 NVIDIA/显示器环境的无边框原生 HDR 渲染内容能经 DWM 正确显示；
- **已确认：**0.4.0 的发灰原因就是 HDR/PQ 内容缺少窗口化 PQ 色彩空间标签；
- **已确认：**关闭路径能恢复正常 SDR，未观察到颜色空间残留；
- **尚未确认：**该次人工观察模板未填写 Alt+Tab 结果，不能据此宣称 Alt+Tab 已验证；
- **后续进展：**0.6.0 已验证启动持久化和普通窗口化；显示器/Windows HDR 热切换及其他
  硬件仍待扩展。

## 2026-08-31 第六轮实测、持久化与普通窗口化

| 日志 | 模式与操作 | 关键运行时结果 | 视觉结果 |
| --- | --- | --- | --- |
| `20260831-054033-v060-borderless-toggle-regression` | 无边框，HDR 开→关 | 候选通过；`+0x1B` 为 `0→1→0`；PQ/SDR 两次切换成功 | 用户确认正常 HDR→正常 SDR |
| `20260831-054124-v060-borderless-persist-on-write` | 无边框，开启后直接退出 | PQ 成功并以 HDR 请求为真退出 | 用户确认正常 HDR |
| `20260831-054302-v060-borderless-persist-on-reload` | 无边框，读取开启状态后关闭 | 设置页打开前即以 `live_config_hdr=unknown`、后端请求为真恢复 PQ；关闭恢复 SDR | 用户确认自动恢复 HDR，随后正常 SDR |
| `20260831-054516-v060-borderless-persist-off-reload` | 无边框，读取关闭状态 | 后端请求和实际状态均为假，没有 PQ 开启；菜单仍可选择 | 用户确认正常 SDR |
| `20260831-054654-v060-windowed-persist-on-write` | 普通窗口化，开启后退出 | `2560x1440`、10 位 flip-model 候选通过；PQ 成功 | 用户确认正常 HDR |
| `20260831-054834-v060-windowed-persist-on-reload` | 普通窗口化，读取开启状态后关闭 | 设置页打开前恢复 PQ；关闭恢复 `G22/P709` | 用户确认自动恢复 HDR，随后正常 SDR |

六份日志均记录正确 EXE 大小和 SHA-256、`mode=windowed_hdr`、完整 Hook 安装和
`initialization completed successfully`，且没有 `SAFETY`、失败 HRESULT、`success=false`、
候选拒绝或设备移除。普通窗口化实测交换链为 `2560x1440`、
`R10G10B10A2_UNORM`、3 缓冲、`FLIP_DISCARD`、真实窗口态；输出为 10 bpc/PQ，
`PRESENT=true`。

- **已确认：**公共可用性覆盖早于设置页归一化并保留了游戏自己保存的 HDR=true；
- **已确认：**HDR=false 也能跨启动保留，MOD 不会在关闭状态下误提交 PQ；
- **已确认：**普通窗口化和无边框使用相同严格路径，不需要模式专用布尔值；
- **已确认：**启动自动恢复不需要打开设置页或加载角色；
- **证据边界：**六份 `observations.txt` 未填写，视觉结论来自维护者本轮统一确认；Alt+Tab、
  Overlay、显示器型号和连接方式不能从日志补出。

本轮收集时未传 `-GameExePath`，只使 `system.txt` 少了脚本冗余采集的 EXE 元数据。当时参与
测试的 0.6.0 DLL 自身在 Hook 前记录并强校验了 `87,024,720` 字节和固定 SHA-256；这与只读
备份复算结果一致，因此不影响既有测试有效性。后续跨版本解析改动发生在这轮实测之后。

## 2026-08-31 补充实测与发布准备

维护者在相同 NVIDIA 测试机上补充完成了以下操作测试：

- **已确认（操作与视觉）：**关闭目标显示器的 Windows HDR 后，即使配置为
  `mode = windowed_hdr`，游戏内 HDR 选项仍不可选择；这符合严格候选失败即透传的设计；
- **已确认（操作与视觉）：**两台 HDR 显示器都开启 Windows HDR 时，普通窗口可以一半
  位于显示器 1、一半位于显示器 2，HDR 画面仍正常；
- **已确认（操作与视觉）：**使用 `Win + Shift + Left/Right` 在两台 HDR 显示器间移动
  游戏窗口后，HDR 画面仍正常。

本轮没有对应的新 DLL 日志，因而只能证明维护者观察到的行为，不能据此确认所有输出切换
事件、HRESULT 或内部 revision。它也不覆盖一台 HDR 开启、另一台 HDR 关闭的混合桌面，
以及 Windows HDR 运行中热切换、显示器断连或设备重建。

## 目标文件指纹

| App Ver. | 文件版本 | 文件大小 | SHA-256 | 证据状态 |
| --- | --- | ---: | --- | --- |
| 1.16.2 | 2.6.2.0 | 86,998,096 | `34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB` | 静态解析与当前 NVIDIA 实机通过 |
| 1.17 | 2.7.0.0 | 87,024,720 | `D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134` | 静态与当前 NVIDIA 实机通过 |

两者均为 COFF x86-64、映像基址 `0x140000000`，游戏图形路径为 D3D12 / DXGI。同版本的
“有 DLC/无 DLC”备份字节完全相同。下文未另行标注的绝对地址来自 1.17；不能直接套用到
1.16.2 或未来版本。游戏更新后必须重新扫描并验证控制流，运行时解析成功也不能替代首次
人工与实机复核。

## 2026-08-31 跨版本静态审计

1.16.2 与 1.17 的四个 Hook 目标都保持相同 ABI、字段语义和关键控制流，但 RVA 使用四组
不同变化量，不能用统一地址增量兼容。当前实现改为：

- 公共 HDR 可用性使用忽略 RIP-relative/`call rel32` 位移的多段签名，并要求可执行节唯一；
- 扫描到该函数的全部直接调用，要求恰好两个，再以 19 字节逻辑反值形状定位 HDR 行谓词；
- 从可执行节的 RIP-relative `lea` 反查唯一虚表，并验证 MSVC COL、lambda RTTI 与 6 个
  可执行虚表项；
- 配置复制签名额外验证 `source+0x15 → destination+0x1B`；后端签名验证 `+0x32 bit 2`
  与交换链虚表 `+0x58`；
- 安全 Cookie 由已验证指令动态求址，必须位于可读非执行节；
- 已知指纹仍与各自预期 RVA 逐项比对，未知哈希只在整组条件全部成立时放行。

离线对两个真实磁盘 EXE 运行同一规则，均得到：可用性签名 1 个、直接调用者 2 个、灰显
谓词 1 个、虚表 1 个且有 4 个代码引用、配置复制 1 个、后端查询 1 个；解析地址全部匹配
Ghidra 结果。详细地址表、日志语义和未来版本维护步骤见
`docs/version-compatibility.zh-CN.md`。

## 2026-08-31 跨版本动态解析器实机验收

维护者用正式 1.0.0 发布 DLL 在两版游戏中各完成两次启动。1.17 日志目录为
`20260831-184648-v100-compat-117-borderless-resolve` 与
`20260831-184836-v100-compat-117-persist-windowed`；1.16.2 日志目录为
`20260831-190000-v100-compat-116-borderless-resolve` 与
`20260831-190104-v100-compat-116-persist-windowed`。

四份日志均在约 5.4 秒内唯一解析整组目标，已知版本 RVA 交叉核对全部一致，内部 HDR/PQ
Hook 汇总项全部为 `true`。两版首轮均记录两次 `enable_hdr_pq` 和一次
`restore_previous`；第二轮都在打开设置页前由游戏保存状态自动请求 HDR 并成功设置 PQ，
随后能恢复 SDR。1.17 切换普通窗口化时还记录 `ResizeTarget`、`ResizeBuffers`、候选 revision
更新与新代次 `Present` 前 PQ 重提交。所有调用均为 `HRESULT=0x00000000,
success=true`，没有兼容失败、安全回退、`SAFETY`、设备移除或 Hook 冲突。维护者确认四次
视觉观察全部正常，并说明 1.16.2 使用与 1.17 相同的测试方法。

本轮四次均同时加载 UnlockTheFps，OBS 以 P010/PQ 游戏源预览运行，Alt+Tab 未出现问题。
这补充了一个明确的共存样本，但不能外推为所有 Overlay、捕获软件或 DXGI MOD 都兼容。
原始 `observations.txt` 只有第一份部分填写；其余视觉判定来自维护者在交付日志时的明确说明，
因此日志可以证明内部时序和返回码，视觉正常仍属于维护者操作观察。

## 静态分析证据

### 配置与窗口模式

- ASCII 字符串 `HDRSetting` 位于 VA `0x142B14B90`，引用点为 `0x140954180` 与
  `0x140954313`。
- `FUN_1409542D0` 通过 `HDRSetting` 构造/读取一个图形配置项。
- `Resolution-BorderlessScreenWidth` 和 `Resolution-BorderlessScreenHeight` 位于
  `0x142C01530` 与 `0x142C01580`，由图形配置读写函数引用；同一配置路径还维护独占全屏
  尺寸。
- 二进制含有 `CSGraphicsConfig`、`GXSwapChainOperator` 和
  `GXSwapChainOperatorForMultiWindow` 的 RTTI 名称。

这些事实表明菜单条件、图形配置和交换链管理是分层的。受 Arxan 和错误的 non-return
推断影响，部分 Ghidra 反编译结果仍不完整；关键结论均用原始指令和虚表交叉验证。

### 0.2.0 错误目标

0.2.0 截获的 `FUN_140808A70` 最终通向 `FUN_1409542D0` 和 `HDRSetting`，因此当时被误判为
HDR 开关。结合真实 UI 调用次数与菜单构造顺序，现已确认它属于
`02_046_BrightnessSetting`，即根据 SDR/HDR 状态进入不同亮度校准页。旧虚表
`0x142AC1778`、调用槽 `0x142AC1788` 及其捕获的 `+0x1CE2` 字节不再用于解锁。

### 真正 HDR 行与 setter

`FUN_14095D540` 构造“声音与显示设置”页面。前四项对应血液显示、字幕、HUD 和教程提示；
随后单独构造的 bool 行才是真正 HDR 开关。机器码确认：

1. 初始 HDR 值从图形状态 `+0x13C` 复制到菜单页 `+0x1CE3`；
2. setter lambda 虚表为 `0x142B15290`，调用槽 `0x142B152A0` 指向
   `FUN_140962350`；
3. `FUN_140962350` 把菜单页 `+0x1CE3` 写入捕获的 `MENU_OPTION_DATA +0x15`，调用菜单页
   虚函数 `+0xA0`，然后跳转到 `FUN_14067B150`；
4. 设置应用函数 `FUN_14025C780` 的完整字段映射包含源 `+0x15` 到实时图形配置 `+0x1B`。

因此 `+0x15` / `+0x1B` 是由真正 setter 和复制函数共同证明的 HDR 字段，不再是根据相邻
设置顺序猜测的“候选字段”。0.3.0 在 `FUN_14025C780` 入口安装只读 inline observer，调用
原函数前后记录这一字段和其余实际变化。

### 真正 HDR 灰显谓词

真正 HDR 行传入另一个 bool lambda 作为灰显谓词：

1. 虚表起始于 `0x142B152C8`；MSVC RTTI Complete Object Locator 位于
   `0x143320AB8`；
2. 第三项 `0x142B152D8` 指向 `FUN_140962B30`；
3. `FUN_140962B30` 调用 `FUN_140953A10`，并以 `setz al` 返回其逻辑反值，即
   `original_grayed = !native_eligible`；
4. `FUN_140953A10` 先要求 `FUN_14078BC60()` 为真，再要求
   `FUN_141E88E90(&output, 1)` 返回成功且 `output[0] != 0`；
5. `FUN_14078BC60` 从全局渲染管理器调用 `FUN_140E91380`，只在返回值为 `1` 时为真。

**已确认：**上述控制流、地址和字段偏移。0.3.0 实测又确认 `native_eligible` 在全屏为
`true`、无边框为 `false`，因此返回值 `1` 的第一项条件在当前路径上确实与独占全屏对应；
第二项仍代表所选输出的 HDR 可用性。

0.3.0 只替换该特定 lambda 虚表的调用槽：`observe` 调用并返回原结果；
`unlock_hdr_menu` 仍先调用原函数用于诊断，再仅返回 `false`。安装前使用上述运行时解析并
复核 RTTI、五个相邻虚表项和 19 字节原函数体。若行为修改模式发现未知前置 Hook 或字节
变更，则拒绝覆盖并保留 DXGI/AGS 观测。

### HDR 持久化与公共可用性归一化

用户观察到：全屏 HDR 能跨启动保留，而强制无边框 HDR 在下一次启动后总是关闭；
`GraphicsConfig.xml` 不含 HDR 字段，同时未加载角色的启动/退出也可能改变 `.sl2/.co2`。
这支持“图形设置的 HDR 字段属于游戏统一序列化数据”的判断，但仅凭文件时间戳不能证明
具体字节位置，也可能包含会话元数据或校验更新。0.6.0 因此不解析或改写存档。

已有反编译给出了更直接的状态丢失解释：

1. `FUN_14025C550` 从序列化流读取 `0x140` 字节图形配置；它只在输出 HDR 能力查询失败或
   输出标志为假时把配置 `+0x1B` 清零，并随后标记图形状态待应用；
2. `FUN_14093D730` 创建设置页时，先把实时图形配置复制到局部菜单数据，再调用
   `FUN_140953A10`；
3. 若该公共可用性返回假，且局部 HDR 值与渲染记录的实际值 `+0x13C` 不同，它会把局部值
   改成实际值并调用 `FUN_14025C780` 写回实时配置；
4. 窗口态的原生实际值为假，所以已加载的 HDR=true 会在这里被归一化为 false，并进入游戏
   自己的后续保存路径。

2026-08-30 对 `0x140953A10` 的定向 Ghidra 导出进一步确认，它只有两个直接调用者：
`FUN_14093D730` 和真正 HDR 行谓词 `FUN_140962B30`。这使公共函数成为比“另存一个 MOD
布尔值”更小、语义更一致的干预点。

0.6.0 在完整 EXE 哈希通过后校验该函数 14 字节入口；当前版本则先以多段签名唯一解析，再
动态校验入口和 Cookie 目标。入口含 RIP-relative 安全 cookie 读取，不能直接复制到任意地址，
因此实现使用专用 trampoline：把 cookie 地址改写为绝对
加载，再跳回原函数。原生结果仍透传；只有 `windowed_hdr` 且活动交换链通过 10 位窗口态严格
候选检查时，原生 false 才变为 true。全屏原生 true 不改变。这样菜单和初始化使用同一个
有效资格结果，同时由游戏继续拥有 `.sl2/.co2` 中的持久数据。0.6.0 第六轮实测已确认
HDR=true 和 HDR=false 在无边框下均能跨启动保留，普通窗口化 HDR=true 也能在打开设置页
前自动恢复。

### 配置请求与“实际 HDR 状态”的第二层门控

0.3.0 视觉结果出现后，继续追踪实时配置 `+0x1B` 的读取点，确认完整下游链：

1. `FUN_140256560` 返回当前实时图形配置；
2. `FUN_14025C780` 更新 `+0x1B` 后，`FUN_14067B150` 把游戏管理器 `+0xA90` 标记为脏；
3. `FUN_140680420` 在下一次更新中读取该脏标志，并调用
   `FUN_1419ECB00(DAT_1447F33E0, 0, config[0x1B])`；
4. `FUN_1419ECB00` 把请求值写入首个 `0x170` 字节记录的 `+0x13D`，然后调用
   `FUN_141E9F4B0(backend, requested == 0)`；后者把反向的“禁用 HDR”值写到后端 `+0x30`；
5. 绘制系统更新时，`FUN_1419E7780` 对记录调用 `FUN_141E99780`，并把返回值写入记录
   `+0x13C`；`+0x13D` 因而是请求状态，`+0x13C` 是实际状态；
6. `FUN_141E99780` 只是把后端 `+0x38` 传给 `FUN_141E9F4D0`；
7. `FUN_141E9F4D0` 通过交换链虚表 `+0x58` 调用
   `IDXGISwapChain::GetFullscreenState`，并且只有能力位存在、输出身份匹配、模式匹配、
   `fullscreen != 0` 等条件同时满足时才返回 `true`。

第 7 点是无边框请求仍为 SDR 的直接机器码解释，不再只是根据 UI 灰显行为推断。Windows
窗口化 HDR 支持并不能自动改变游戏自身这一额外限制。

0.4.0 在 `FUN_141E9F4D0` 的已验证 14 字节入口安装 inline observer。默认和
`unlock_hdr_menu` 模式完整返回原值；`emulate_hdr_fullscreen_state` 只有在实时配置和后端
均请求 HDR、原值为 `false`，并且具体 DXGI 对象通过 10 位无边框 flip-model、PQ 输出及
`CheckColorSpaceSupport(PRESENT)` 检查时，才把**此内部结果**改为 `true`。它不 Hook 或
伪造全局 `IDXGISwapChain::GetFullscreenState`，因此窗口管理、Alt+Tab 与其他游戏逻辑仍能
看到真实无边框状态。

0.4.0 实测已经验证这段运行时模型。0.5.0 的新模式不在后端 Hook 内直接调用 DXGI，而是把
期望颜色空间写入按交换链隔离的状态机；`Present` / `Present1` Hook 在调用原函数前再次验证
交换链/输出，并执行一次受管 `SetColorSpace1`。这样内部实际状态先由 `FUN_1419E7780` 写入
`+0x13C`，颜色空间标签再和即将提交的帧对齐。进入实验前最后观察到的颜色空间被保存为
恢复目标；Resize 或全屏
状态变化会标记重应用，失败则锁存并停止重新启用。0.5.0 实机日志和画面已共同验证该基本
时序；第六轮实测又验证了无边框/普通窗口化启动恢复和一次普通窗口尺寸重配置。显示器
变化、Windows HDR 热切换、休眠恢复及 HDR 开启期间切换独占全屏仍待验证。

### AMD AGS HDR 路径

游戏附带 `amd_ags_x64.dll` 5.0.5，并导入：

- `agsInit`
- `agsDeInit`
- `agsSetDisplayMode`

`FUN_141EB0640` 的机器码行为已经人工核对：

1. 把传入的显示设备名转换并与 AGS 枚举出的设备/显示器比较；
2. 选择具有目标能力位的显示器，取得设备索引与显示器索引；
3. 清零栈上的 `0x68` 字节设置结构；
4. 对布尔参数执行 `neg` / `sbb` / `and 2`，即关闭时写 `mode = 0`，开启时写
   `mode = 2`；
5. 在 `0x141EB08F6` 调用 `agsSetDisplayMode(context, device, display, &settings)`；
6. AGS 成功时返回游戏状态 `0`，失败时映射为 `5`。

AGS 5.0.5 的官方头文件定义顺序为：

```text
Mode_SDR = 0
Mode_scRGB = 1
Mode_PQ = 2
Mode_DolbyVision = 3
```

同一头文件明确指出 `Mode_PQ` 要求 `1010102 UNORM` 交换链和 PQ 输出着色器，并要求在
每次全屏切换或交换链变化后重新调用 `agsSetDisplayMode`。这使“底层影响”的判断从推测
变成了直接证据：当前版本的原生 HDR 是 HDR10/PQ 路径，不是一个孤立的 UI 标志。

这个函数是 AMD 专用路径。它可能经接口虚表间接调用，静态扫描未找到直接 `call` 到
`0x141EB0640` 的站点。不能据此推断 NVIDIA 也依赖 AGS；跨厂商部分仍应以 DXGI 状态为准。

### 尚未确认的事项

- Windows HDR 热切换、休眠恢复和设备重建后的稳定性；
- 两台显示器 HDR 状态不一致时的跨屏/移动，以及显示器断连；
- AMD、Intel 以及其他 NVIDIA 驱动/显示器组合；
- ReShade、RTSS、Steam/NVIDIA Overlay 和其他 DXGI Hook 的共存；OBS P010/PQ 与
  UnlockTheFps 已有一次明确双版本组合通过，但不能外推到其他配置；
- HDR 已开启时切换独占全屏及再返回窗口态的恢复；
- **已排除：**只让 HDR 菜单回调在窗口态运行并不足以切换内部 PQ 输出；
- **已确认（当前 NVIDIA 环境）：**内部 HDR + Present 前 PQ 能消除 0.4.0 的发灰，并能在
  关闭时恢复正常 SDR；公共可用性覆盖能保留两种窗口态的游戏自身 HDR 状态。
- **已确认（当前限定组合）：**1.0.0 四次跨版本回归中的 Alt+Tab、OBS P010/PQ 预览与
  UnlockTheFps 同时加载未出现问题。

在 EXE 中没有找到 `IDXGISwapChain3/4`、`IDXGIOutput6` GUID 的直接字节常量，但这不能证明
接口没有被使用：接口可能经包装层、已有对象、动态表或其他模块获得。

## 为什么窗口化 HDR 在平台层面成立

微软的 Advanced Color 文档说明，开启 Advanced Color 后 DWM 使用 FP16 进行桌面合成，
窗口化应用不再局限于传统的 8 bit/channel 输出。Win32/DXGI 应用需要：

1. 确认目标输出的当前 Advanced Color 能力与亮度范围；
2. 使用 flip sequential 或 flip discard 交换链；
3. 为 scRGB 使用 FP16，或者为 HDR10/PQ 使用 10 位格式并显式设置 PQ/BT.2020 颜色空间；
4. 在窗口移动、显示器或系统 HDR 状态变化后重新查询能力和配置交换链。

`SetColorSpace1` 决定 Windows 如何解释交换链像素。`SetHDRMetaData` 只提供可选的显示元数据，
不会改变像素解释；微软也不保证元数据一定转发到显示器。因此后者不能被当成“开启 HDR”
的快捷开关。

这说明只要游戏的无边框交换链可转成兼容的 10 位 flip-model 链，并同时启用正确的 PQ
着色器输出，DWM 路径在技术上能够呈现原生 HDR。

## 推荐的 DLL 架构

### 1. ModEngine3 提前加载

使用 Rust `cdylib`，通过 `[[natives]]` 配置 `load_early = true`，并导出显式初始化函数。
初始化函数在游戏创建 DXGI 工厂前安装 Hook；`DllMain` 保持最小化。

参考项目 `UnlockTheFps` 已经实现了适合本项目复用的基础设施：

- EXE IAT 定位与安全替换；
- `CreateDXGIFactory` / `CreateDXGIFactory1` 捕获；
- 工厂、交换链的对象级影子虚表；
- `QueryInterface` / `Release` 生命周期维护；
- 与 Overlay、外部影子虚表和不同接口长度共存；
- 仅扫描可执行 PE 节、唯一签名和页保护恢复。

应复用或提炼这些已测试机制，不建议另写一个只覆盖固定虚表长度的简化 Hook。

### 2. 第一版观测器（已完成）

首次 DLL 不改变返回值或参数，只记录：

- 所有交换链创建描述；
- `GetDesc` / `GetDesc1`、窗口模式和输出设备；
- `ResizeBuffers`、`SetFullscreenState`、`Present`；
- `CheckColorSpaceSupport`、`SetColorSpace1`；
- `SetHDRMetaData` 的类型与结构；
- `IDXGIOutput6::GetDesc1` 的 `ColorSpace`、`BitsPerColor`、亮度信息；
- AMD 的 `agsSetDisplayMode` 参数及返回值；
- 内部 `HDRSetting`、窗口模式和交换链重建事件的时间顺序。

三条基线与额外复测已经采集。交换链格式不是差异点；全屏 HDR 开启当次也未调用已观测的
DXGI/AGS HDR API。0.3.0 已进一步采集正确谓词、配置 setter 与无边框视觉结果；0.4.0 又
增加内部请求状态、原生实际状态和最终有效状态的去重日志，并已完成三组实测。

### 3. 当前最小干预路径

0.5.0 已完成菜单、请求、内部实际状态和窗口化 PQ 的基本闭环，0.6.0 又完成公共可用性、
持久化与普通窗口化闭环。当前顺序为：

1. **已完成：**使用 `windowed_hdr` 在活动交换链上验证公共 HDR 可用性覆盖；
2. **已完成：**保持游戏自己的持久化数据为唯一事实来源，不解析或修改 `.sl2/.co2`；
3. **已完成：**无边框 HDR=true 与 false 均能跨启动恢复，且无需打开设置页或加载角色；
4. **已完成：**普通窗口化满足 10 位、flip-model、PQ 输出和 `PRESENT` 支持；
5. **已完成：**两种窗口态使用同一内部实际状态 + Present 同步 PQ/恢复状态机；
6. 扩展 Resize、HDR/SDR 混合显示器、Windows HDR 热切换、休眠恢复和 Overlay 共存；
7. 仅在 AMD 上、且游戏切换基线确有需要时复现 `agsSetDisplayMode(mode = 2)`；
8. 只在基线确实使用时复现 HDR10 metadata。

0.1.0 已证明“内部仍为 SDR 时过早设置 PQ”会产生异常；0.5.0 已证明当前严格时序正确。
0.6.0 不改变这一安全边界，只把同一候选判断前移到公共可用性层，以防游戏在窗口态把已
保存的请求归一化为关闭。最终 PQ 调用仍只发生在内部 HDR 已获准并即将 `Present` 时。

## 风险评估

| 风险 | 级别 | 说明与缓解 |
| --- | --- | --- |
| 只解锁 UI，管线仍为 SDR | 高 | 仅作为分阶段诊断；不据此宣称 HDR 成功。 |
| 内部仍为 SDR 时设置 PQ | 高 | 0.1.0 已复现异常；0.5.0 要求全部内部门控成立，并将切换同步到 `Present` 前。 |
| PQ 提交成功但最终像素仍非 PQ | 中高 | 当前 NVIDIA 实机已得到正常 HDR；其他硬件仍需逐一验证。 |
| 关闭 HDR 后颜色空间未恢复 | 高 | 保存最后观察到的颜色空间；对应 SDR 帧 `Present` 前恢复，失败锁存并记录。 |
| 内部状态模拟破坏窗口逻辑 | 高 | 只 Hook 游戏的 HDR 后端汇总函数，不改真实 `GetFullscreenState`；条件失败即透传。 |
| 内部着色器仍输出 SDR/gamma 2.2 | 高 | 对照全屏 HDR 的内部状态和最终图像。 |
| 公共可用性覆盖过早或过宽 | 高 | 只覆盖两个已确认调用者共用的函数；原生 true 透传，窗口态必须通过严格 DXGI 候选检查。 |
| 启动时已保存 HDR 仍被清零 | 高 | 当前 NVIDIA 实测已通过；候选尚不可用时仍安全失败并保留完整日志，不直接改存档。 |
| 重复维护 MOD 与游戏两份 HDR 状态 | 中高 | 0.6.0 不创建 MOD 布尔值，以游戏自身序列化状态为唯一来源。 |
| Overlay/MOD 重复改写 DXGI 虚表 | 高 | 使用对象级影子表、链式调用和 QI/Release 生命周期。 |
| Alt+Tab、Resize、显示器切换后失效 | 中高 | 1.0.0 回归中的 Alt+Tab 与 Resize 已通过；双 HDR 显示器移动已通过视觉测试，但输出能力变化不一定触发 revision，其他场景仍需实测。 |
| AMD AGS 与现代 DXGI 状态冲突 | 中高 | 只复现游戏自己的 AMD 时序；其他厂商不调用 AGS。 |
| Windows HDR 未开启 | 中 | 不擅自改全局设置，给出明确日志并回退。 |
| 游戏更新与 Arxan 影响签名 | 高 | 已知哈希交叉核对、可执行节唯一扫描、调用关系与 RTTI/虚表验证；歧义时安全失败。 |
| HDR 元数据在显示器间表现不一致 | 中 | 不把 metadata 当开关，优先正确色调映射与颜色空间。 |

## 实机验证矩阵

至少记录以下四种状态，并比较事件顺序和交换链描述：

| 窗口模式 | 游戏 HDR | 用途 |
| --- | --- | --- |
| 全屏 | 关 | SDR 基线 |
| 全屏 | 开 | 原生 HDR 参考基线 |
| 无边框 | 关 | 当前受限路径基线 |
| 无边框 | `unlock_hdr_menu` | 让游戏自己的回调运行并逐项比较 |
| 无边框 | `emulate_hdr_fullscreen_state` | 已验证解除第二层门控后画面发灰的诊断对照 |
| 无边框 | `emulate_hdr_and_set_pq` | 已验证 Present 同步 PQ 能得到正常 HDR，并能恢复 SDR |
| 无边框 | `windowed_hdr` | 已验证 HDR 开/关跨启动持久化及核心路径回归 |
| 普通窗口化 | `windowed_hdr` | 已验证相同候选、正常 HDR/SDR 和跨启动持久化 |
| 普通窗口化，双 HDR 显示器 | `windowed_hdr` | 已通过跨屏显示和快捷键移动的操作/视觉测试；无新增日志 |
| 任意窗口态，Windows HDR 关闭 | `windowed_hdr` | 已确认选项保持不可选择，不强制 PQ；无新增日志 |

每次测试记录：

- Windows 版本、Windows HDR 开关、GPU/驱动、显示器、接口和刷新率；
- EXE SHA-256、AGS DLL 版本、ModEngine3 版本和同时加载的 MOD/Overlay；
- 交换链格式、SwapEffect、Flags、颜色空间支持位与当前颜色空间；
- 输出 `BitsPerColor`、颜色空间及亮度范围；
- AGS 参数（AMD）和 HRESULT/返回码；
- Alt+Tab、分辨率切换、窗口移到另一显示器、休眠恢复、HDR 热切换结果；
- 视觉上是否出现灰雾、黑位抬升、高光裁切、过饱和或重复色调映射。

当前首发证据只覆盖 NVIDIA。扩大兼容性结论前至少还应在 AMD 上验证一次；静态分析无法
替代这一步。

## 继续或终止的判定

0.6.0 已满足当前单机 NVIDIA 环境的核心推进条件：

- **已满足：**观测器能稳定捕获全屏 HDR 切换当次的状态时序；
- **已满足：**`emulate_hdr_fullscreen_state` 得到 `effective_actual=true`，并在关闭时可靠回退；
- **已满足：**无边框交换链稳定保持 10 位 flip-model；
- **已满足：**`emulate_hdr_and_set_pq` 的 PQ 与恢复调用均成功；
- **已满足（当前 NVIDIA 环境）：**视觉结果没有发灰、黑位抬升、异常饱和或重复色调映射；
- **已满足（当前 NVIDIA 环境）：**HDR 开启与关闭能在无边框下跨启动准确恢复；
- **已满足（当前 NVIDIA 环境）：**普通窗口化通过同一候选检查并得到正常 HDR/SDR；
- **部分满足：**正常退出、已测 Resize、Windows HDR 关闭回退和双 HDR 显示器移动成立；
  1.0.0 限定组合中的 Alt+Tab 也成立，HDR/SDR 混合显示器及系统 HDR 热切换仍需扩展验证。

若只能让菜单可点，却无法得到 10 位交换链或正确 PQ 输出，应停止该路线，不发布“伪 HDR”
补丁。若必须重写大量 D3D12 资源、PSO 或最终色调映射着色器，项目仍可能实现，但工作量和
兼容风险将从“状态/交换链 Hook”上升到“渲染管线 MOD”，应重新评估范围。

## 最终判断

- 平台可行性：高。Windows 明确支持 DWM 合成的窗口化 HDR。
- 游戏静态可行性：中高。原生 PQ/AGS 路径和 HDR 菜单门控传递链已有机器码证据。
- 交换链前提：高。NVIDIA 首轮实测已确认无边框同样是 10 位 flip-model。
- 简单 UI 解锁可行性：只适合作为诊断，不能单独作为成品方案。
- Rust + ModEngine3 DLL 方案：高。0.1.0 至 1.0.0 已真实加载并逐层验证；当前 NVIDIA
  环境已在 1.16.2 与 1.17 得到正常无边框/普通窗口化 HDR、正常 SDR 恢复和跨启动持久化。
- 最终成品成功把握：高但仍有条件。核心路径、游戏自身持久化和两种窗口态已经成立；当前
  关键问题是更广泛的异常恢复和兼容矩阵。1.0.0 的公开说明必须限定为当前 NVIDIA 环境
  已验证；跨 GPU、驱动和 Overlay 验证完成前不能称为跨硬件通用方案。

## 主要资料

- [微软：在高/标准动态范围显示器上将 DirectX 与高级颜色配合使用](https://learn.microsoft.com/zh-cn/windows/win32/direct3darticles/high-dynamic-range)
- [微软：IDXGISwapChain3::CheckColorSpaceSupport](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-checkcolorspacesupport)
- [微软：IDXGISwapChain3::SetColorSpace1](https://learn.microsoft.com/zh-cn/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-setcolorspace1)
- [微软：IDXGISwapChain4::SetHDRMetaData](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgiswapchain4-sethdrmetadata)
- [微软 DirectX 12 HDR 示例](https://github.com/microsoft/DirectX-Graphics-Samples/blob/master/Samples/Desktop/D3D12HDR/src/D3D12HDR.cpp)
- [AMD AGS SDK 5.0.5 头文件](https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/v5.0.5/ags_lib/inc/amd_ags.h)
- [Eldenpedia：System（用于交叉核对“声音与显示设置”的 UI 顺序）](https://eldenring.wiki.gg/wiki/System)
