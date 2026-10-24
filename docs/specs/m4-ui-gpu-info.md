# m4-ui-gpu-info: Model Info names the graphics adapter

The Model Info window gets one more line saying which graphics adapter and backend draw the
viewport, so a slow machine can be diagnosed.

## `chrome::adapter_line(name, backend, device_type) -> String`

Returns `"Graphics: <name> (<backend>, <device_type>)"`, e.g.
`adapter_line("Quadro P2000", "Vulkan", "discrete GPU")` is
`"Graphics: Quadro P2000 (Vulkan, discrete GPU)"`.
- Each argument is trimmed. An empty name becomes `unknown adapter`.
- An empty backend or device type is left out with its comma: `("X", "", "CPU")` gives
  `"Graphics: X (CPU)"`; `("X", "Vulkan", "")` gives `"Graphics: X (Vulkan)"`; both empty gives
  `"Graphics: X"`.

## App wiring (no automated test; check in the window)

In `app.rs`, when the Model Info window is shown, append `adapter_line(...)` to the lines when
the wgpu render state is available. Take the name, backend and device type from the adapter info
(`wgpu_render_state().adapter.get_info()`); show the backend as `Vulkan`, `Metal`, `DX12`, `GL`
or `BrowserWebGpu`, and the device type as `discrete GPU`, `integrated GPU`, `virtual GPU`, `CPU`
or `other`. Without a render state add nothing. `model_info_lines` is unchanged.
