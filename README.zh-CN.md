![spektrafilm-rs](docs/card.jpg)

# spektrafilm-rs

[English](README.md) | **简体中文**

用同一张数码照片，你可以看看它印在彩色相纸上的样子，也可以换一款黑白胶片，试试黄镜和不同反差的相纸。spektrafilm-rs 模拟从胶片曝光、显影到印相或扫描的过程。它可以单独使用，也是摄影应用 FPW 使用的胶片引擎。下文介绍独立版的功能，FPW 有自己的版本和控件。

## 从一张照片开始

在桌面预览工具里打开一张 **sRGB 的 JPEG 或 PNG**。想试彩色，可以选 Kodak Portra 400 和 Kodak Portra Endura 相纸。想试黑白，可以选 Tri-X 400，从默认的 Ilford Multigrade IV RC 2 号纸开始。保持胶片和曝光不变，换成 1 号和 4 号纸，就能看到同一张负片如何印得更柔和或更硬朗。

独立工具也能读取线性 TIFF 和受支持相机的 RAW 文件。首次运行、保存预览、导出全尺寸照片和命令行用法见[使用指南](docs/USAGE.md)。

## 现在能做什么

- **控制曝光。** 可以手动调曝光，也可以让测光优先照顾亮部或暗部。遇到逆光照片，可用命令行的曝光宽容度提示，看看有多少画面接近所选胶片能分辨的明暗边界。
- **调整彩色印相。** 用放大机滤色调整偏色，用印相曝光调整明暗，也可以试预闪。看反转片时可以直接扫描胶片，跳过相纸。直扫负片会得到负像。
- **制作黑白照片。** Tri-X 400、T-Max 100、Acros II 和 HP5 Plus 都可搭配 Multigrade IV RC。黄镜和红镜能改变不同颜色变成灰色后的深浅，例如让蓝天变暗。可以选择补偿滤镜损失的曝光，再单独调纸号控制反差。
- **调整质感和片幅。** 可以调颗粒、镜头柔光和亮边光晕。用 Vulkan 渲染时，还可通过配方选择 35 mm 全格、半格、XPan 或 120 片幅，添加片边字印、日期和局部漏光。[片幅与片边用法](docs/135_FILM_BORDER.md)
- **把配色带到其他软件。** 可以将胶片和相纸的颜色效果导出为 CUBE 查找表。颗粒、光晕和片边要在渲染照片时生成。[CUBE 导出说明](docs/USAGE.md#spectral-cube-export)

## 哪些效果要手动开

彩色负片默认按光学方式印到相纸上。以下选项会改变成片观感，可以在同一张照片上分别试用。

- **负片或印相漂白旁路（留银效果）。** 两项默认都为零。调负片上的留银会影响这张负片后续的扫描和印相；调相纸上的留银只影响彩色印相。
- **从负片重建印相色相。** 默认关闭。可以用一张带蓝紫色织物的照片对比开关前后。这个选项适用于输出为 sRGB 的彩色印相，手动黄、品红校色均需为零。
- **胶片颜色响应。** 默认用 Hanatos 2025 计算照片颜色在胶片上的曝光，也可手动改用 Arctic beta04。使用 Hanatos 时，部分胶片还提供单独的逐胶片颜色校正，默认关闭。[颜色与印相设置](docs/USAGE.md)

## 暂时用不了什么

| 项目 | 目前的情况 |
| --- | --- |
| 黑白相纸半号 | 目前提供 00、0、1、2、3、4、5 号。已有测量还不足以可靠区分硬调端的 4½ 号，因此保留整号。 |
| 老款 Portra NC / VC | 160 和 400 的 NC、VC 四款胶片及配套印相数据，仍在等待明确的再分发许可。 |
| 去背涂层胶片的特殊红晕 | 做过原型，强光边缘的红色拖尾过重，暂未开放。常规胶片光晕可用。其他试验性的颜色和显影模型见[研究记录](docs/ENGINE_STUDIES.md)。 |
| 按嵌入的色彩配置转换照片 | 独立工具尚未实现内嵌 ICC 配置转换。带其他色彩配置的 JPEG 或 PNG 请先转为 sRGB。完整的广色域工作流仍待开发。 |

## 成片验证

我们用实拍照片和灰阶做了印相对照。在已验证的黑白和彩色印相配方下，CPU 和 Vulkan 的输出都与参考成片接近。[验证范围与样例配方](docs/ENGINE_EVOLUTION.md)

开发者可查阅[构建与后端支持](docs/USAGE.md#backends)、[宿主接入指南](docs/maintenance/HOST_GPU_INTEGRATION_CONTRACT.md)和[接入示例](crates/spektrafilm-core/examples/fpw_host_contract.rs)。

## 致谢与许可

胶片模型和光谱数据源自 [Andrea Volpato 的 spektrafilm](https://github.com/andreavolpato/spektrafilm)，最初的 Rust 移植由 [turbasvin](https://github.com/turbasvin/spektrafilm-rs) 完成。光谱重建基于 [Johannes Hanatos](https://github.com/hanatos) 的工作。数值计算参考了 NumPy、SciPy、scikit-image 和 colour-science。多层颗粒与滤波实现包含 SpektraLab 的贡献。[实现来源](docs/maintenance/GRAIN_MULTILAYER_MIGRATION.md)

代码采用 [GPL-3.0](LICENSE)。胶片与相纸数据请参阅[上游署名及 CC BY-SA 4.0 条款](SPEKTRAFILM_DATA_LICENSE.txt)、[黑白胶片与相纸的单独许可说明](data/SPEKTRALAB_BW_NOTICE.md)及[扫描校准数据来源](data/negative_scan_ap0_v1/PROVENANCE.md)。
