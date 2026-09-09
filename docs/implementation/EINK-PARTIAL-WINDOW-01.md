# EINK-PARTIAL-WINDOW-01

Follow-up, not implemented by the UI restyle.

The production panel path has evidence for fullscreen partial transfer only.
Before adding SSD1677 regional-window commands, collect one physical trace per
route transition with timestamps for: debounced button capture, state
transition, framebuffer render, panel-transfer start, transfer completion and
BUSY release. Repeat after wake and after the periodic global cleanup.

Acceptance requires no stale pixels outside the proposed window, correct
portrait byte alignment, preserved wake/base-frame safety, and a measured
improvement against the existing fullscreen partial transfer. Until then, the
single `PanelRefreshCoordinator` continues to select only global-base or the
proven fullscreen partial path.
