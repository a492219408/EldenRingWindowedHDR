# 实机测试与日志收集

## 已完成的关键基线

2026-08-30 已完成 0.3.0 至 0.5.0 的分层测试：

| 版本与日志 | 已确认结果 |
| --- | --- |
| 0.3.0 `20260830-200213-v030-unlock-borderless-hdr-transition` | 真正 HDR 行可解锁，实时配置 `+0x1B` 能 `0→1→0`，但画面仍为 SDR。 |
| 0.4.0 `20260830-210635-v040-emulate-borderless-hdr-transition` | 内部 `effective_actual=true`，画面发灰；关闭后内部状态和 SDR 画面恢复。 |
| 0.5.0 `20260830-215204-v050-borderless-hdr-pq-sync` | 开启时内部 HDR 与 Present 前 PQ 均成功，用户确认画面为正常 HDR；关闭时恢复 `G22/P709`，画面为正常 SDR。 |

0.5.0 日志中的关键闭环是：

```text
graphics-config apply ... destination+0x1B=0->1
HDR backend ... native_actual=false, effective_actual=true, override=true
managed SetColorSpace1 before Present ... transition=enable_hdr_pq ... success=true
graphics-config apply ... destination+0x1B=1->0
managed SetColorSpace1 before Present ... transition=restore_previous ... success=true
```

这已经确认无边框 HDR 的核心渲染与颜色空间路径在当前 NVIDIA 测试机上可行。0.6.0 的测试
目标不再是重复证明 PQ，而是验证：

1. 游戏自己的 HDR 开关能否在窗口态跨启动保留；
2. 新的公共可用性覆盖是否会阻止启动/设置页初始化把已载入值压回关闭；
3. 同一路径能否用于普通“窗口化”，而不只用于“无边框窗口化”。

上述 0.6.0 测试已于 2026-08-31 全部完成：

| 日志 | 结果 |
| --- | --- |
| `20260831-054033-v060-borderless-toggle-regression` | 无边框正常 HDR/SDR 开关，PQ 与恢复均成功。 |
| `20260831-054124-v060-borderless-persist-on-write` | 无边框以 HDR 开启状态正常退出。 |
| `20260831-054302-v060-borderless-persist-on-reload` | 打开设置页前自动恢复 HDR，随后关闭并恢复 SDR。 |
| `20260831-054516-v060-borderless-persist-off-reload` | 保存关闭状态后重启未误启 HDR，选项仍可操作。 |
| `20260831-054654-v060-windowed-persist-on-write` | 普通窗口化正常开启 HDR，并以开启状态退出。 |
| `20260831-054834-v060-windowed-persist-on-reload` | 普通窗口化自动恢复 HDR，随后关闭并恢复 SDR。 |

六份 DLL 日志均通过目标 EXE 哈希、Hook、候选条件和 HRESULT 检查，维护者确认视觉结果全部
符合预期。详细审计见 `final-audit.zh-CN.md`。以下矩阵保留作为复现和后续硬件回归流程。

## 0.6.0/0.6.1 的持久化方案

使用：

```ini
[HDR]
mode = windowed_hdr
```

该模式不会打开、解析、复制或修改 `ER0000.sl2/.co2`。游戏仍负责读写自己的 HDR 设置；MOD
只覆盖 `FUN_140953A10` 的窗口态可用性结果。定向 Ghidra 分析确认该函数只有两个直接调用者：

- 真正 HDR 行的灰显谓词；
- 设置页初始化函数 `FUN_14093D730`。后者在 HDR 不可用时会把已载入的 HDR 值同步为当前
  “实际状态”，窗口态下通常就是关闭。

原生全屏结果始终透传。原生结果为假时，活动交换链必须同时满足以下条件才会被判定可用：

- `R10G10B10A2_UNORM`；
- 至少 2 个缓冲；
- `FLIP_SEQUENTIAL` 或 `FLIP_DISCARD`；
- 真实窗口态，未进入独占全屏；
- 输出连接到桌面、至少 10 bpc，Windows 输出色彩空间为 PQ；
- `CheckColorSpaceSupport(PQ)` 含 `PRESENT`。

因此无边框与普通窗口化使用同一受保护路径。启动时配置复制观察器可能还没有看到菜单值；
此时只有游戏后端自己的已确认“请求 HDR”位为真，才允许进入内部 HDR。用户操作菜单后，
仍要求菜单配置与后端请求一致。

## 1.0.0 发布候选快速复核

1.0.0 只改变公开名称、文件名、initializer 名称、缺省配置和打包方式，HDR 状态机未改；但
正式发布前仍应对**最终 ZIP 中的实际 DLL/INI/.me3 组合**做一次短回归。测试时不要同时加载
旧 `EldenRingBorderlessHDR.dll`，也不要手工修改发布 INI。

### 第 1 次：新默认值与无边框开关

1. Windows HDR 开启，以“无边框窗口化 + HDR 关闭”启动；
2. 打开 HDR 设置页，确认选项可选择；
3. 开启 HDR，确认画面为正常 HDR；再关闭，确认恢复正常 SDR；
4. 再次开启 HDR 并正常退出；
5. 收集为 `v100-rc-borderless-default-toggle`。

日志应以 `EldenRingWindowedHDR 1.0.0` 开头，并包含：

```text
configuration: mode=windowed_hdr
initialization completed successfully
managed SetColorSpace1 before Present ... transition=enable_hdr_pq ... success=true
managed SetColorSpace1 before Present ... transition=restore_previous ... success=true
```

### 第 2 次：持久化与普通窗口化

1. 再次启动，在打开设置页或加载角色前等待标题界面稳定；
2. 确认 HDR 已自动恢复，且画面正常；
3. 切换到普通“窗口化”，确认 HDR 仍正常；若方便，可在两台均开启 HDR 的显示器间移动；
4. 打开设置页，关闭 HDR，确认恢复正常 SDR，然后正常退出；
5. 收集为 `v100-rc-persist-windowed`。

### 第 3 次：Windows HDR 关闭时的安全回退

1. 退出游戏后，在 Windows 中关闭目标显示器的 HDR；
2. 启动游戏并打开 HDR 设置页；
3. 确认 HDR 选项不可选择，画面保持正常 SDR，没有异常色彩；
4. 正常退出，收集为 `v100-rc-windows-hdr-off`；
5. 测试完成后按需恢复 Windows HDR。

该快速复核完成前，只能把 1.0.0 称为“本地构建通过、等待最终实机确认”的发布候选。

## 0.6.x 历史矩阵测试前准备

- 复核历史证据时使用相应 0.6.x 包，并确认日志首行为 `EldenRingBorderlessHDR 0.6.1` 或
  `EldenRingBorderlessHDR 0.6.0`；
- Windows“使用 HDR”保持开启；
- INI 设置为 `mode = windowed_hdr`；
- 首轮关闭 OBS、ReShade、RTSS、Steam/NVIDIA Overlay 和其他 MOD；
- 保持 `start_online = false`，只做离线测试；
- 不要编辑、重命名或替换 `.sl2/.co2`，也不需要加载角色；
- 每次启动会截断日志，无需手动删除；必须在下一次启动前收集；
- 若原生全屏 HDR 在本次开机中出现游戏已知的偶发发灰，先重启并确认原生基线正常；
- 出现黑屏、持续闪烁、严重过曝或明显异常颜色时，关闭 HDR 并退出，不要反复切换。

## 第 1 组：无边框开启/关闭回归

一次启动内完成：

1. 以“无边框窗口化 + HDR 关闭”启动；
2. 打开 HDR 设置页，确认 HDR 可选择；
3. 开启 HDR，等待 10 秒，确认仍是 0.5.0 已验证的正常 HDR；
4. 关闭 HDR，等待 5 秒，确认恢复正常 SDR；
5. 退出并收集为 `v060-borderless-toggle-regression`。

预期日志包含：

```text
configuration: mode=windowed_hdr
HDR common-availability observer installed ... passthrough=true
EXPERIMENT: strict windowed HDR availability override enabled ...
HDR common availability ... native_eligible=false ... windowed_candidate=true, effective_eligible=true, mode=windowed_hdr
HDR menu gate ... upstream_eligible=true, original_grayed=false, effective_grayed=false, mode=windowed_hdr
managed SetColorSpace1 before Present ... transition=enable_hdr_pq ... success=true
managed SetColorSpace1 before Present ... transition=restore_previous ... success=true
```

候选检查日志可能由 `HDR common availability` 或 `HDR backend` 首先触发；两者都可以。
历史 0.6.0 日志中的同一菜单字段名为 `native_eligible`；0.6.1 改名是为了说明它已经包含
公共可用性 Hook 的上游结果，不是行为变化。

## 第 2 组：无边框 HDR 开启状态跨启动

### 第 2A 次启动：写入开启状态

1. 保持无边框，打开设置页并开启 HDR；
2. 确认画面为正常 HDR；
3. **不要关闭 HDR**，直接从游戏菜单正常退出；
4. 收集为 `v060-borderless-persist-on-write`。

### 第 2B 次启动：读取开启状态

1. 再次启动后先不要打开设置页，也不要加载角色，等待标题界面稳定 10 秒；
2. 打开 HDR 设置页，记录开关是否已经为“开启”且可选择；
3. 若已自动开启，确认画面/显示器状态为正常 HDR；
4. 将 HDR 关闭，等待 5 秒确认正常 SDR，再正常退出；
5. 收集为 `v060-borderless-persist-on-reload`。

成功的启动恢复应在用户打开设置页或手动切换前出现近似日志：

```text
HDR backend actual query ... live_config_hdr=unknown, backend_requested_hdr=true ... effective_actual=true, override=true, mode=windowed_hdr
HDR color-space synchronization request ... desired=HDR/PQ
managed SetColorSpace1 before Present ... transition=enable_hdr_pq ... success=true
```

同时，不应在用户操作前出现把 HDR 从 `1` 写回 `0` 的 `graphics-config apply`。如果启动仍为
关闭，请保留首次 `HDR common availability` 的时间、`active_swap_chain`、`revision`、候选结果
和全部 `SAFETY` 行；不要为了重试而修改存档。

## 第 3 组：无边框 HDR 关闭状态跨启动

第 2B 次启动退出时已经保存关闭状态。再启动一次：

1. 不打开设置页、不加载角色，等待 10 秒；
2. 确认没有自动进入 HDR/PQ；
3. 打开设置页，确认 HDR 为“关闭”且仍可选择；
4. 不改设置，正常退出；
5. 收集为 `v060-borderless-persist-off-reload`。

预期后端为 `backend_requested_hdr=false`、`effective_actual=false`，且用户操作前没有
`desired=HDR/PQ` 或 `transition=enable_hdr_pq`。

## 第 4 组：普通窗口化与持久化

前 3 组全部正常后再做本组。

### 第 4A 次启动：窗口化开启

1. 切换到普通“窗口化”，确认 HDR 初始为关闭；
2. 打开 HDR 设置页，确认选项可选择；
3. 开启 HDR，等待 10 秒并检查高光、暗部、黑位和色彩；
4. 保持 HDR 开启并正常退出；
5. 收集为 `v060-windowed-persist-on-write`。

### 第 4B 次启动：窗口化恢复

1. 再次启动后先等待 10 秒，不打开设置页、不加载角色；
2. 确认仍为普通窗口化；
3. 打开设置页，确认 HDR 自动恢复为开启且画面正常；
4. 关闭 HDR，确认恢复正常 SDR，再退出；
5. 收集为 `v060-windowed-persist-on-reload`。

窗口化成功日志的候选详情应为 `windowed=true`、`exclusive_fullscreen=false`、10 位、flip-model、
PQ 输出及 `PRESENT` 支持。窗口尺寸与无边框不同是正常现象；若格式、SwapEffect 或输出条件
不同，MOD 应拒绝启用并记录具体原因。

## 日志收集命令

每次游戏退出后、下次启动前运行：

```powershell
.\scripts\collect-logs.ps1 `
  -LogPath 'D:\你的Mod目录\natives\EldenRingWindowedHDR.log' `
  -Label 'v100-rc-borderless-default-toggle' `
  -GameExePath 'D:\你的Steam库\steamapps\common\ELDEN RING\Game\eldenring.exe'
```

每次替换 `-Label`。请填写生成目录中的 `observations.txt`，尤其记录：启动前预期保存状态、
打开设置页前后的状态、退出时状态、是否加载角色，以及视觉异常。原始日志可能包含本机路径，
不要提交到公开仓库。

`-GameExePath` 可以省略，但建议始终提供。省略时只会让 `system.txt` 缺少脚本追加的 EXE
元数据；DLL 日志仍会在安装 Hook 前记录并强校验实际运行 EXE 的大小和 SHA-256。当前
收集脚本会在 `system.txt` 中明确标记这一情况。

## 通过与停止条件

通过下限：

- 无边框开启/关闭仍分别是正常 HDR/SDR；
- HDR 开启和关闭都能跨启动准确恢复；
- 普通窗口化能通过相同候选检查，并得到正常 HDR/SDR；
- 所有受管 `SetColorSpace1` 都成功，且没有 `SAFETY`、设备移除或崩溃；
- 启动恢复不要求打开设置页或加载角色。

出现以下任一情况就停止对应组并回传日志：

- 公共可用性 Hook、候选检查或受管颜色空间切换失败；
- HDR 仍在启动时被改回关闭；
- 开启后发灰、过曝、黑位抬升、颜色异常、闪屏或黑屏；
- 关闭后未完整恢复 SDR；
- 普通窗口化交换链不满足候选条件。

0.6.0 已通过这些测试；维护者随后还通过了 Windows HDR 关闭回退和双 HDR 显示器移动/跨屏
的视觉测试。但这些结果只代表当前 NVIDIA/显示器环境；HDR/SDR 混合显示器、Windows HDR
热切换、休眠恢复、AMD/Intel、Overlay 与其他 MOD 共存仍需后续验证。
