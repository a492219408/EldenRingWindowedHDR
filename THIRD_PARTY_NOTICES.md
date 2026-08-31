# 第三方代码声明

`src/dxgi.rs`、`src/windows.rs` 与 `src/logger.rs` 的基础 Hook、Windows FFI 和日志实现改编自
本机参考项目 `UnlockTheFps`。该项目采用 MIT License：

```text
Copyright (c) 2026 YmdElf
Copyright (c) 2025 Luca2040
```

本仓库保留了其对象级影子虚表、Overlay 链式调用、PE 导入表检查和对应压力测试，并将业务
逻辑改为 HDR 状态观测与受保护的 PQ 实验。完整 MIT 条款见 `LICENSE`。
