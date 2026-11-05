# Fonts

| File | Font | Version | License | Source |
| --- | --- | --- | --- | --- |
| `Roboto-Regular.ttf` | Roboto Regular | 2.138 | Apache-2.0 (`LICENSE-Roboto`) | https://github.com/googlefonts/roboto-2/releases/tag/v2.138, `roboto-unhinted.zip` (sha256 `70f64c71…a728`); file sha256 `f3edb805…24c6` |
| `Roboto-Medium.ttf` | Roboto Medium | 2.138 | Apache-2.0 (`LICENSE-Roboto`) | same `roboto-unhinted.zip`; file sha256 `b398bb9c791ddb08f1063d9c874f98e8aadb99132043b2d272a9276cf90c465a` |
| `RobotoMono-Regular.ttf` | Roboto Mono Regular | 3.000 | Apache-2.0 (`LICENSE-RobotoMono`) | static instance (wght 400, fontTools 4.68 `varLib.instancer`, `updateFontNames`) of the variable `RobotoMono[wght].ttf` in github.com/google/fonts at commit `ac8f68d4ac7c8ada8a74502131e1a94d52d8137d` (`apache/robotomono`; later releases moved to OFL, so this is the last Apache one); file sha256 `6fab085ae0ee91bf2abdd5090bb2b7e6002595337c1ffa1868636339ed9e14d8` |
| `RobotoMono-Medium.ttf` | Roboto Mono Medium | 3.000 | Apache-2.0 (`LICENSE-RobotoMono`) | same, wght 500; file sha256 `1529264afcb636fd0fdaa4d56a749243a6db04f347315fd7f381818b1ad8e26f` |

The app uses this instead of egui's bundled fonts, which are OFL-1.1 / Ubuntu-font-1.0 (ADR-001).
