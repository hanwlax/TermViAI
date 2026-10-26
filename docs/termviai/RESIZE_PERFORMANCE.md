# Windows 窗口缩放性能

本轮保留现有终端与 UI 渲染器，参考 winit 的 Windows 重绘事件处理及 Win32 更新区域规则修复调度；没有迁移到 GPUI，也没有把新增缓动当作卡顿修复。

## 确认的机制与改动

- 原 `wm_paint` 在限帧期间直接返回，未清理更新区域；同时解除限流依赖普通异步队列。拖动窗口的 Windows 模态消息循环与普通任务队列不是同一个调度路径。
- TermViAI 路径改为始终验证更新区域，使用按帧起始时间计算的预算。绘制本身消耗的时间计入帧间隔，不在绘制完成后再额外等一整个周期。
- 缩放消息只标记待更新；实际绘制前读取最新 client rect，合并中间尺寸。DPI、最小化、最大化等非 live resize 路径保留原有处理。
- 独立的 Windows 模态计时器推动有预算的普通任务队列，鼠标停住时仍有唤醒机会；它与重绘延期计时器分开，避免频繁刷新重绘期限导致普通任务一直得不到执行。计时器在结束拖动/销毁时清理，不作为精确垂直同步保证。
- `wm_size` 返回 `None` 表示继续默认消息处理。退出 size/move 时原来的 `?` 会跳过末尾的 `live_resizing=false` 通知；现在明确送达最终状态，完成延后的终端布局。
- 原生 UI 标签的 shaping 结果按 font ID / 文本缓存；字体回退使用 shape generation 失效，DPI/字号/字重使用不同 font ID。缓存不存纹理或屏幕位置，因此 resize 的裁切仍重新计算。缓存限制 512 项，超过 2048 字节的字符串不进入缓存。终端网格 shaping 保持原路径。

## 可复现的 CPU 基准

Windows Release：

```text
cargo test --locked --release -p wezterm-gui benchmark_resize_layout_and_text -- --ignored --nocapture --test-threads=1
```

基准使用相同的 Monitor 数据、尺寸变化序列、真实 UI 字体；每组预热 20 次、采样 120 次。比较不使用/使用 UI shaping 缓存；包含 Monitor 布局与可见文字 shaping，不包含完整 box model、字形纹理上传、GPU 绘制、呈现或真实鼠标输入。两种模式处理的 glyph 数必须相同；不能把这组耗时换算为应用 FPS。

首轮结果：

| 场景 | 无缓存 P50/P95 | 缓存 P50/P95 |
| --- | --- | --- |
| 8 张 NPU 卡片 | 0.267 / 0.293 ms | 0.025 / 0.025 ms |
| 含超长命令的详情抽屉 | 0.703 / 0.998 ms | 0.068 / 0.073 ms |

这些结果证明减少了重复 CPU 工作，但不足以单独解释用户录屏中的明显卡顿。帧调度与驱动等待仍需要实机轨迹确认。

## 实机轨迹

试用包内双击 `Resize-Diagnostics.cmd`，进入发生卡顿的界面，拖动窗口边缘约 5–10 秒后松开。每次结束拖动会异步追加到同目录 `resize-trace.csv`。普通双击 `TermViAI.exe` 不开启记录。

CSV 按每次拖动 session 分组，包含 count、sampled、P50/P95/max/total 毫秒；每阶段最多保存 8192 个样本用于分位数，count/total/max 覆盖全部。最多排队 8 份报告，记录与写盘不阻塞 UI。只记录阶段与耗时，不记录主机、命令、路径、密码或终端文本。

| stage | 含义 |
| --- | --- |
| resize_event | 收到的 live resize 请求数；duration 为零，用于与实际布局次数比较 |
| resize_layout | 应用尺寸/终端布局开销 |
| resize_to_repaint_return | 首个未处理尺寸请求到重绘回调返回，包括排队 |
| resize_to_present_return | 首个未处理尺寸请求到提交/交换接口返回；不是屏幕扫描呈现延迟 |
| frame_callback | 完整原生重绘回调 |
| ui_layout | TermViAI 页面与交互布局 |
| ui_elements | UI box model 计算及生成绘制元素 |
| ui_shape_miss | 未命中文字缓存时的 shaping |
| paint_prepare | 整帧 CPU 绘制准备（包含 UI 阶段） |
| draw_submit | 绘制调用提交（wgpu 路径也包含 acquire/present） |
| gl_present | OpenGL/ANGLE 交换缓冲的 CPU 等待 |
| wgpu_acquire / wgpu_present | WGPU 取得交换链纹理/提交呈现的 CPU 等待 |
| modal_task / modal_queue | 单个任务/一轮有预算的模态队列执行时间 |
| present_interval | 呈现 API 返回之间的间隔；包含按住鼠标不移动等空闲间隔，不能直接当作持续绘制 FPS |

阶段相互嵌套，不能把它们的 total 相加。应结合 `resize_to_present_return`、`frame_callback` 与各子阶段判断：排队延迟、CPU 准备或驱动等待。WM_TIMER 是低优先级消息且不是高精度帧时钟；不得据此承诺任意机器稳定 60/144 fps。

真实拖动的验证未自动执行：computer-use 技能禁止操作终端应用。原生单元测试、字体/布局基准与用户设备上的 GPU/鼠标体验分开验收。

## 参考

- [winit Windows WM_PAINT 实现](https://docs.rs/winit/latest/src/winit/platform_impl/windows/event_loop.rs.html)
- [Microsoft：更新区域合并与验证](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/gdi/invalidating-and-validating-the-update-region.md)
- [Microsoft：计时器机制](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-timers)
- [Zed Windows 渲染实现背景](https://zed.dev/blog/windows-progress-report)
