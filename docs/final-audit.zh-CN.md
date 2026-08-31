# EldenRingBorderlessHDR 0.6.1 历史审计与 1.0.0 发布补充

> 本文前半部分保留 0.6.1 审计时的文件名、哈希和打包结论，作为历史证据；文末另列
> `Elden Ring Native Windowed HDR` 1.0.0 的发布层变更，二者不可混用。

审计日期：2026-08-31  
目标游戏：App Ver. 1.17 / `eldenring.exe` 2.7.0.0  
已实机验证实现：0.6.0 `windowed_hdr`  
审计补丁版本：0.6.1

## 审计结论

在本项目定义的 0.6.0 验收范围内，没有发现阻止交付的问题。六次启动均通过目标 EXE 强
校验，所有必要 Hook 均安装成功；无边框和普通窗口化的严格候选条件成立，HDR/PQ 开启、
SDR 恢复以及游戏自身开关的开启/关闭持久化形成了完整闭环。维护者同时确认六次测试的
画面与操作结果均符合 `docs/testing.zh-CN.md` 的预期。

结论只适用于当前已测环境和固定 EXE 哈希。0.6.1 没有改变已验证的正常 HDR 状态机路径；
它只收紧失败路径、未知 Hook 冲突处理和发布脚本的数据保护。0.6.1 DLL 已完成本地静态检查
与构建，但没有再次启动真实游戏，因此运行时证据仍归属于 0.6.0。

## 测试证据完整性

收集脚本未传入 `-GameExePath`，所以六份 `system.txt` 没有由脚本追加的 EXE 路径、版本、
大小和哈希。这不影响本轮判定：DLL 在安装任何 Hook 前会自行读取实际运行中的主程序，
六份日志都记录了同一结果：

```text
target executable size: 87024720 bytes
target executable SHA-256: D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134
configuration: mode=windowed_hdr
```

该值也与只读备份重新计算的大小、文件版本和 SHA-256 一致。六份 `observations.txt` 仍是
空模板，因此日志只能证明程序状态；视觉判断依据维护者在测试完成后给出的“所有测试均
符合预期”，不能从空模板独立重建显示器型号、连接方式、Overlay 或 Alt+Tab 结果。

## 六组 0.6.0 实机结果

测试系统日志记录为 Windows 11 build 26200、GeForce RTX 4090 D、驱动
`32.0.16.1074`。目标输出为 `\\.\DISPLAY2`，报告 10 bpc、PQ、亮度范围
`0.010..420.000 nits`；六次均无 `SAFETY`、失败 HRESULT、`success=false`、候选拒绝或
Hook 缺失。

| 日志 | 已确认的程序闭环 |
| --- | --- |
| `20260831-054033-v060-borderless-toggle-regression` | 无边框候选通过；实时 HDR `0→1→0`；PQ 开启和 `G22/P709` 恢复均成功。 |
| `20260831-054124-v060-borderless-persist-on-write` | 无边框开启 HDR 后以开启状态退出；PQ 提交成功。 |
| `20260831-054302-v060-borderless-persist-on-reload` | 打开设置页前，游戏后端已以 `live_config_hdr=unknown`、`backend_requested_hdr=true` 恢复 HDR；随后关闭并恢复 SDR。 |
| `20260831-054516-v060-borderless-persist-off-reload` | 保存关闭后重启未误启 PQ；菜单仍通过公共可用性覆盖保持可选择。 |
| `20260831-054654-v060-windowed-persist-on-write` | 切换到 `2560x1440` 普通窗口化后候选通过，HDR/PQ 开启成功并以开启状态退出。 |
| `20260831-054834-v060-windowed-persist-on-reload` | 普通窗口化在设置页打开前自动恢复 HDR/PQ；随后关闭并成功恢复 SDR。 |

持久化没有由 MOD 自建布尔值完成。日志证明窗口态公共可用性覆盖阻止了游戏把已保存的 HDR
请求归一化为关闭，实际序列化仍由游戏拥有；MOD 没有打开、解析或修改 `.sl2/.co2`。

## 实现与安全边界审计

- ModEngine3 配置保持 `start_online = false`、`load_early = true` 和显式 initializer；
  `DllMain` 只保存模块、关闭线程通知并启动工作线程。
- 初始化先校验文件名、`87,024,720` 字节和完整 SHA-256，再接触版本专用 RVA。
- 菜单 Hook 校验 RTTI、相邻虚表项和原函数体；三个 inline Hook 均校验完整入口字节，含
  RIP-relative security cookie 的公共可用性函数使用专用重定位 trampoline。
- DXGI 工厂和交换链使用对象级影子虚表，保留 `QueryInterface` / `Release` 生命周期、不同
  接口长度、内存区域边界和外部 Hook 链处理；相关压力测试来自并扩展了 `UnlockTheFps`。
- `windowed_hdr` 只在 10 位、至少双缓冲、flip-model、真实窗口态、PQ/10 bpc 输出及
  `CheckColorSpaceSupport(PRESENT)` 同时成立时改变内部资格结果。
- PQ 只在内部 HDR 请求获准后、对应帧 `Present` 前提交；关闭时在 SDR 帧前恢复此前记录的
  颜色空间。失败按交换链锁存，外部冲突会放弃所有权。
- 不修改真实 `GetFullscreenState`、Windows HDR、注册表、显示器模式、驱动配置、游戏 EXE
  或存档；目标版本不符或前提不成立时安全回退。

## 0.6.1 审计修正

- `CheckColorSpaceSupport` 观察 Hook 只在 HRESULT 成功时读取输出参数，避免失败调用留下的
  未初始化值被诊断代码读取。
- `windowed_hdr` 现在也被视为行为修改模式；若真正 HDR 菜单槽已被未知 Hook 修改，整个
  行为路径会拒绝叠加，而不是按只读观察规则继续。
- 菜单日志把公共可用性覆盖后的结果改称 `upstream_eligible`，避免误写成原生资格。
- 打包脚本若发现待替换目录含 `test-results` 会直接停止，防止重打同版本时删除实机证据。
- 日志收集脚本在未提供 `-GameExePath` 时会向 `system.txt` 写入明确说明。

## 已执行的本地检查

Cargo 不由本机 mise 管理，因此按项目规则直接使用 PATH 中的 Cargo 1.96.0：

```text
cargo fmt --all -- --check                                      通过
cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
                                                                通过，0 warning
cargo test --locked --target x86_64-pc-windows-msvc             通过，41/41
cargo build --locked --release --target x86_64-pc-windows-msvc  通过
PowerShell AST 解析 build.ps1 / collect-logs.ps1 / package.ps1  通过
```

发布 DLL 为 COFF x86-64、64 位、`IMAGE_FILE_DLL`，保留 ASLR、High Entropy VA 和 NX；导出
`DllMain` 与 `elden_ring_borderless_hdr_init`。构建目录与打包目录中的 DLL SHA-256 均为：

```text
543753FE490FC57CF2F88C17E25079E575838B3DD36FF1D1A582C62982EDFA3A
```

ZIP 只含 `.me3`、DLL、INI、README、三份文档、许可证、第三方声明和日志收集脚本，没有
测试日志、游戏二进制、PDB、Ghidra 数据或存档。

## 仍未验证的范围

- AMD、Intel，以及其他 NVIDIA GPU、驱动、显示器和连接方式；
- HDR/SDR 混合显示器、Windows HDR 热切换、休眠/恢复和设备移除/重建；
- HDR 开启期间切换独占全屏、长时间运行和各种分辨率变化；
- ReShade、OBS、RTSS、Steam/NVIDIA Overlay、UnlockTheFps 及其他 DXGI MOD 共存；
- 游戏更新后的地址、字节、控制流和 ABI。

当前候选缓存以交换链对象和重配置 revision 为边界；只改变输出能力而没有触发交换链事件的
系统 HDR/显示器变化仍属于上述未验证范围。跨硬件和异常恢复矩阵完成前，本项目应描述为
“在固定 1.17 版本和当前 NVIDIA 环境已验证可用”，不能描述为通用 HDR MOD。

## 0.6.1 发布判定（历史）

0.6.1 可作为当前测试环境的受保护发布候选。发布 INI 继续默认 `observe`，避免在尚未覆盖的
硬件上自动改变行为；使用已验证功能时显式设置：

```ini
[HDR]
mode = windowed_hdr
```

真实游戏测试证据保留在本地 `EldenRingBorderlessHDR-0.6.0/test-results`，不纳入公开 ZIP。

## 1.0.0 发布准备补充

1.0.0 没有修改 `src/dxgi.rs`、`src/game_hdr.rs` 或 `src/windows.rs` 中已验证的 HDR/DXGI
状态机。发布层变更如下：

- 正式名称改为 `Elden Ring Native Windowed HDR`，避免 `Borderless` 让用户误以为普通
  “窗口化”不受支持；
- crate、DLL、INI、日志和 `.me3` 前缀统一为 `EldenRingWindowedHDR`；新的 ModEngine3
  initializer 为 `elden_ring_windowed_hdr_init`，同时保留旧 initializer 导出作为开发期配置
  兼容别名；
- 发布 INI 和缺省配置都改为 `mode = windowed_hdr`，因此首次运行缺少 INI 时也会创建并
  使用正式功能模式；`observe` 只保留为诊断选项；
- 玩家 ZIP 不再包含开发用 `docs` 与 `scripts`，只含 `.me3`、DLL、INI、双语 TXT README、
  `LICENSE.txt` 和 `THIRD_PARTY_NOTICES.txt`，并附独立 SHA-256 文件；
- 新增 GitHub Actions：普通 push/PR 构建并上传 ZIP artifact，`v<版本>` 标签在与
  `Cargo.toml` 版本完全一致时创建或更新 GitHub Release；
- MIT 主署名改为 `Luan (a492219408)`；第三方声明继续保留历史来源中的 `YmdElf` 与
  `Luca2040`。

维护者在 0.6.x 核心实现上追加确认了 Windows HDR 关闭时安全保持不可用，以及两台均开启
HDR 的显示器间跨屏显示和 `Win + Shift + Left/Right` 移动仍为正常 HDR；该轮没有新增 DLL
日志，属于操作与视觉证据。HDR/SDR 混合显示器、运行时热切换和断连仍未覆盖。

1.0.0 已完成的本地构建、静态检查、发布包清单与最终哈希记录在本节后续审计结果中：

```text
EldenRingWindowedHDR.dll SHA-256: CC550681B1D36221FE37183E5B0DABA21F5805F54A7A60B99123DABD788928BB
EldenRingWindowedHDR-1.0.0.zip SHA-256: A3FA5A664BEC28C38A325CB0E3083024618E2AF8D5643D4B3E3D7918BDACAD2B
```

Cargo 不由 mise 管理，本轮直接使用 PATH 中的 Cargo。`cargo fmt --all -- --check`、带
`-D warnings` 的 Clippy、41/41 单元测试和 x64 MSVC Release 构建均通过。DLL 为 COFF
x86-64，具备 ASLR、High Entropy VA、NX 与 CFG instrumentation，导出 `DllMain`、新
initializer 和旧兼容 initializer。构建目录与打包目录中的 DLL 哈希一致。三个 PowerShell
脚本均通过 AST 解析；ZIP 校验文件与复算值一致，归档恰含 7 个运行/用户文档文件，没有
`docs`、`scripts`、`test-results`、PDB、日志或转储。两个 GitHub Actions 工作流还通过了
官方 `actionlint` 1.7.12 的本地静态检查。

改名后的最终 DLL/INI/.me3 组合尚未由本环境启动真实游戏。发布标签前应按
`docs/testing.zh-CN.md` 的“1.0.0 发布候选快速复核”完成三次启动并保存日志；在此之前，
1.0.0 的结论是“静态与本地构建通过、核心状态机继承既有实测、最终发行物待实机复核”。
