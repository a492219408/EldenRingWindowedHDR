Elden Ring Native Windowed HDR 1.0.0
=====================================

本 MOD 的作用
-------------

本 MOD 允许《艾尔登法环》在“无边框窗口化”和普通“窗口化”模式下使用游戏自身的
HDR 渲染链。它不是 ReShade 预设、后处理滤镜或 Auto HDR，而是让游戏原生的 HDR
渲染与窗口态交换链正确配合。

游戏内的 HDR 开关可以正常开启和关闭，状态仍由游戏自身保存。关闭 HDR 后，MOD 会恢复
正常 SDR 输出。

使用要求
--------

- 《艾尔登法环》；App Ver. 1.16.2 / eldenring.exe 2.6.2.0 与 App Ver. 1.17 /
  eldenring.exe 2.7.0.0 均已完成实机验证；
- 支持 HDR 的显示器；
- 已在 Windows 中为当前显示游戏画面的显示器开启 HDR；
- ModEngine3；
- 关闭 Easy Anti-Cheat，仅限离线游玩。

已完成静态审计与实机验证的版本：

- App Ver. 1.16.2 / eldenring.exe 2.6.2.0；
  大小 86,998,096 字节；
  SHA-256 34102B1C08BB5F769A724427A6F70FE29B3B732C31CF73693F861C48D3492DDB。
- App Ver. 1.17 / eldenring.exe 2.7.0.0；
  大小 87,024,720 字节；
  SHA-256 D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134。

以上两个版本均在当前 NVIDIA 测试机上通过同一运行时目标解析器与窗口态 HDR 回归。
未来游戏版本若相关代码结构没有变化，DLL 会在严格检查通过后尝试继续工作；检查存在任何
歧义时会安全保留游戏原生行为，并在日志中说明原因，而不是强行使用旧版本地址。

安装和使用
----------

1. 完整解压发布包，不要改变 .me3 文件与 natives 文件夹的相对位置。
2. 使用 ModEngine3 加载 EldenRingWindowedHDR.me3，并通过 ModEngine3 启动游戏。
3. 在 Windows 设置中，为支持 HDR 的显示器开启 HDR。
4. 在游戏中选择“无边框窗口化”或普通“窗口化”。
5. 打开游戏的 HDR 设置页，开启“高动态范围成像”。

发布包附带的 INI 已使用正常功能模式：

    [HDR]
    mode = windowed_hdr

如果 Windows HDR 没有开启，或者当前显示器/交换链不满足所需条件，游戏内 HDR 选项会
保持不可选择。这是预期的安全行为。MOD 不会、也不可能让 SDR 显示器凭空产生真正 HDR
效果。

卸载
----

在 ModEngine3 中停止使用 EldenRingWindowedHDR.me3，或删除解压后的 MOD 文件夹即可。
本 MOD 不修改 eldenring.exe、Windows HDR、显卡驱动设置、注册表或游戏存档。

问题排查
--------

- 确认显示游戏画面的显示器已在 Windows 中开启 HDR；
- 检查日志中的 COMPATIBILITY 行；若出现 COMPATIBILITY FAILURE，说明当前游戏版本或其他
  MOD 改动了关键代码，本 MOD 不会启用窗口态 HDR；
- 保持 EldenRingWindowedHDR.dll 与 EldenRingWindowedHDR.ini 同在 natives 文件夹中；
- 如果同时使用会修改 DXGI 色彩空间或全屏状态的 MOD，可先暂时停用它们；
- 反馈问题时请附上 natives/EldenRingWindowedHDR.log。该日志每次启动都会被覆盖，请在
  再次启动游戏前保存。

兼容性说明
----------

当前 NVIDIA 测试环境已在以上两个游戏版本确认 HDR/SDR 开关、状态跨启动恢复和普通
窗口化；最终跨版本回归还通过了 Alt+Tab 以及与 UnlockTheFps 同时加载。Windows HDR 关闭
回退、在两台均已开启 HDR 的显示器之间移动窗口和跨屏显示则在 1.17 上额外通过。AMD、
Intel、其他驱动/显示器、HDR/SDR 混合显示器、Windows HDR 运行时热切换、休眠恢复以及
更多 Overlay/MOD 组合仍未完成全面验证。

日志标记为“unknown executable accepted”的未来版本，在完成静态复核和对应实机回归前
只能视为实验性结构兼容。

本 MOD 仅用于 ModEngine3 离线运行。请勿在绕过反作弊后进入官方匹配。

许可证
------

本项目以 MIT License 开源，详见 LICENSE.txt 和 THIRD_PARTY_NOTICES.txt。
