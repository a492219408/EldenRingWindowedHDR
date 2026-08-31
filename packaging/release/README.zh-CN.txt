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

- 《艾尔登法环》App Ver. 1.17，eldenring.exe 文件版本 2.7.0.0；
- 支持 HDR 的显示器；
- 已在 Windows 中为当前显示游戏画面的显示器开启 HDR；
- ModEngine3；
- 关闭 Easy Anti-Cheat，仅限离线游玩。

支持的 eldenring.exe 大小为 87,024,720 字节，SHA-256 为：
D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134

DLL 会在安装任何 Hook 前校验该指纹，不支持的游戏版本会安全拒绝运行。

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
- 确认游戏 EXE 与上面的受支持指纹一致；
- 保持 EldenRingWindowedHDR.dll 与 EldenRingWindowedHDR.ini 同在 natives 文件夹中；
- 如果同时使用会修改 DXGI 色彩空间或全屏状态的 MOD，可先暂时停用它们；
- 反馈问题时请附上 natives/EldenRingWindowedHDR.log。该日志每次启动都会被覆盖，请在
  再次启动游戏前保存。

兼容性说明
----------

当前 NVIDIA 测试环境已确认 HDR/SDR 开关、状态跨启动恢复、普通窗口化，以及在两台均已
开启 HDR 的显示器之间移动窗口和跨屏显示。AMD、Intel、其他驱动/显示器、Windows HDR
运行时热切换、休眠恢复以及更多 Overlay/MOD 组合仍未完成全面验证。

本 MOD 仅用于 ModEngine3 离线运行。请勿在绕过反作弊后进入官方匹配。

许可证
------

本项目以 MIT License 开源，详见 LICENSE.txt 和 THIRD_PARTY_NOTICES.txt。
